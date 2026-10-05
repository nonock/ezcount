//! Talking to the relay: uploading a document's updates and downloading the others'.

use super::*;

/// Every request to the relay gives up after this long (set per request: in the browser,
/// reqwest has no client-wide timeout).
pub(super) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub fn http_client() -> Res<reqwest::Client> {
    let builder = reqwest::Client::builder();
    // The relay never redirects. Following one could resend tokens elsewhere, or over plain
    // HTTP. In the browser, fetch follows redirects itself, but drops the Authorization header
    // across origins and refuses plain HTTP from an HTTPS page.
    #[cfg(not(target_family = "wasm"))]
    let builder = builder
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none());
    // Tests swap relays on one address; a pooled connection would still reach the old one.
    #[cfg(test)]
    let builder = builder.pool_max_idle_per_host(0);
    builder
        .build()
        .map_err(|e| format!("Could not set up networking: {e}"))
}

#[derive(Deserialize)]
pub(super) struct UpdatesPage {
    pub(super) updates: Vec<RemoteUpdate>,
    pub(super) has_more: bool,
    // Missing from relays older than this field.
    #[serde(default)]
    pub(super) relay_id: Option<String>,
}

/// One decrypted page of updates.
pub(super) struct Page {
    pub(super) updates: Vec<(i64, Vec<u8>)>,
    pub(super) has_more: bool,
    pub(super) relay_id: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RemoteUpdate {
    pub(super) seq: i64,
    pub(super) data: String,
}

pub(super) fn updates_url(server_url: &str, group_id: &str) -> String {
    format!("{server_url}/v1/groups/{group_id}/updates")
}

pub(super) fn request_err(e: reqwest::Error) -> String {
    // In the browser, a request that can't reach the server is a plain request error.
    #[cfg(target_family = "wasm")]
    let unreachable = e.is_request() || e.is_timeout();
    #[cfg(not(target_family = "wasm"))]
    let unreachable = e.is_connect() || e.is_timeout();
    if unreachable {
        "Could not reach the sync server".to_string()
    } else {
        format!("Sync request failed: {e}")
    }
}

pub(super) async fn check_status(response: reqwest::Response) -> Res<reqwest::Response> {
    match response.status().as_u16() {
        200..=299 => Ok(response),
        300..=399 => {
            let target = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("another address")
                .to_string();
            Err(format!(
                "The sync server redirects to {target}, which ezcount doesn't follow. Check the \
                 server address."
            ))
        }
        401 | 403 => Err("The sync server rejected this group's key".to_string()),
        404 => Err(NOT_ON_SERVER.to_string()),
        413 => Err("This group has reached the sync server's size limit".to_string()),
        429 => Err(
            "The sync server is getting too many requests from your network. Try again in a while."
                .to_string(),
        ),
        507 => Err("The sync server is full. Try again later.".to_string()),
        status => {
            let body = response.text().await.unwrap_or_default();
            Err(format!("The sync server answered {status}: {body}"))
        }
    }
}

pub(super) const NOT_ON_SERVER: &str = "The sync server does not know this group";

/// A relay from before accounts answers 404 on the account endpoints.
pub(super) const NO_ACCOUNTS: &str =
    "This server doesn't support accounts. Update the ezcount relay.";

/// Encrypts and uploads one update.
pub(super) async fn push(
    http: &reqwest::Client,
    server_url: &str,
    keys: &GroupKeys,
    group_id: &str,
    update: &[u8],
) -> Res<()> {
    let response = http
        .post(updates_url(server_url, group_id))
        .bearer_auth(&keys.auth_token)
        .header("content-type", "application/octet-stream")
        .body(keys.seal(group_id, update)?)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    check_status(response).await?;
    Ok(())
}

/// Fetches and decrypts one page of updates after `after`, in order.
pub(super) async fn pull_page(
    http: &reqwest::Client,
    server_url: &str,
    keys: &GroupKeys,
    group_id: &str,
    after: i64,
) -> Res<Page> {
    let response = http
        .get(updates_url(server_url, group_id))
        .query(&[("after", after)])
        .bearer_auth(&keys.auth_token)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    let page: UpdatesPage = check_status(response)
        .await?
        .json()
        .await
        .map_err(|e| format!("The sync server sent an unexpected response: {e}"))?;
    let updates = page
        .updates
        .into_iter()
        .map(|u| {
            let sealed = STANDARD
                .decode(u.data)
                .map_err(|e| format!("The sync server sent corrupt data: {e}"))?;
            Ok((u.seq, keys.open(group_id, &sealed)?))
        })
        .collect::<Res<_>>()?;
    Ok(Page {
        updates,
        has_more: page.has_more,
        relay_id: page.relay_id,
    })
}

/// Most pages read from the relay in one go. The loops below follow the relay's `has_more`,
/// and a broken or hostile relay could otherwise keep one running forever, holding
/// `sync_lock`. A longer backlog carries on at the next sync, from the saved cursor.
pub(super) const MAX_PAGES_PER_SYNC: usize = 50;

/// Runs blocking storage work (SQLite commits wait for the disk) without stalling the
/// other tasks on this async worker thread.
#[cfg(not(target_family = "wasm"))]
pub(super) fn blocking<T>(work: impl FnOnce() -> T) -> T {
    use tokio::runtime::{Handle, RuntimeFlavor};
    match Handle::try_current().map(|h| h.runtime_flavor()) {
        Ok(RuntimeFlavor::MultiThread) => tokio::task::block_in_place(work),
        _ => work(),
    }
}

/// The browser's one thread is the Web Worker running the core: nothing else to keep going.
#[cfg(target_family = "wasm")]
pub(super) fn blocking<T>(work: impl FnOnce() -> T) -> T {
    work()
}

/// Downloads a document, or its first `MAX_PAGES_PER_SYNC` pages: syncing it once stored
/// fetches the rest. Returns it with the matching sync state.
pub(super) async fn download(
    http: &reqwest::Client,
    server_url: &str,
    secret: &Secret,
    id: &str,
) -> Res<(LoroDoc, SyncMeta)> {
    let keys = GroupKeys::derive(secret)?;
    let doc = LoroDoc::new();
    let mut meta = SyncMeta::new(server_url.to_string(), secret.clone());
    for _ in 0..MAX_PAGES_PER_SYNC {
        let page = pull_page(http, server_url, &keys, id, meta.cursor).await?;
        meta.relay_id = page.relay_id;
        let done = !page.has_more || page.updates.is_empty();
        for (seq, bytes) in page.updates {
            import_remote_update(&doc, &bytes, &mut meta.server_vv)?;
            meta.cursor = seq;
        }
        if done {
            break;
        }
    }
    meta.last_synced_at = Some(chrono::Utc::now());
    Ok((doc, meta))
}
