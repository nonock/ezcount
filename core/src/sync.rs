//! Client side of accounts and group sync.
//!
//! The server is a dumb relay: it stores opaque Loro updates per group, in order, and hands
//! them back by sequence number. Each device pushes the operations the server doesn't have
//! yet and pulls everything after the last sequence number it imported. Loro merges the rest.
//!
//! Anyone holding a group's invite code (server URL, group ID and secret key) can read and
//! edit that group. The relay never sees the secret: updates are end-to-end encrypted and the
//! relay only receives a derived auth token (see `crypto`).
//!
//! An account is one more document synced the same way (see `account`). It lists the groups
//! of one person with their invite details, so every device logged into the account ends up
//! with the same groups: `reconcile` downloads groups added on another device and drops the
//! ones left there. Every group of a logged-in device is shared through the relay.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use loro::{ExportMode, LoroDoc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use url::Url;

use crate::account;
use crate::crypto::{new_recovery_key, CredentialKeys, GroupKeys, LinkKeys, Secret};
use crate::csv_file;
use crate::doc;
use crate::models::{Group, LoginLink, PasswordStrength};
use crate::storage::{import_remote_update, Session, Store, SyncMeta};
use crate::AppState;

type Res<T> = Result<T, String>;

const MIN_PASSWORD_LEN: usize = 8;

/// Every request to the relay gives up after this long (set per request: in the browser,
/// reqwest has no client-wide timeout).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

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

// ---------------------------------------------------------------------------
// Invite codes
// ---------------------------------------------------------------------------

pub struct Invite {
    pub server_url: String,
    pub group_id: String,
    pub secret: Secret,
}

/// Invite format version. Version 2 means end-to-end encrypted updates.
const INVITE_VERSION: &str = "2";

/// An invite link: `<relay>/join#v=2&g=<group id>&k=<secret>`.
///
/// It is a web link, so chat apps make it clickable. The relay's `/join` page opens the app
/// (or tells how to join by hand). The secret is in the fragment, which browsers never send
/// to the server.
pub fn invite_code(server_url: &str, group_id: &str, secret: &Secret) -> String {
    let fragment = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("v", INVITE_VERSION)
        .append_pair("g", group_id)
        .append_pair("k", secret.expose())
        .finish();
    format!("{server_url}/join#{fragment}")
}

/// Accepts invite links (see [`invite_code`]) and the `ezcount://join?server=…&group=…&key=…&v=2`
/// form, which the join page and older versions of the app use.
pub fn parse_invite(code: &str) -> Res<Invite> {
    let invalid = || "This is not a valid ezcount invite".to_string();
    let url = Url::parse(code.trim()).map_err(|_| invalid())?;
    let (server, params): (String, Vec<(String, String)>) = match url.scheme() {
        "ezcount" if url.host_str() == Some("join") => {
            let params: Vec<_> = url.query_pairs().into_owned().collect();
            let server = params
                .iter()
                .find(|(k, _)| k == "server")
                .map(|(_, v)| v.clone())
                .ok_or_else(invalid)?;
            let renamed = params.into_iter().map(|(k, v)| {
                let short = match k.as_str() {
                    "group" => "g",
                    "key" => "k",
                    other => other,
                };
                (short.to_string(), v)
            });
            (server, renamed.collect())
        }
        "http" | "https" => {
            let prefix = url.path().trim_end_matches('/').strip_suffix("/join");
            let (Some(prefix), Some(fragment)) = (prefix, url.fragment()) else {
                return Err(invalid());
            };
            let params = url::form_urlencoded::parse(fragment.as_bytes())
                .into_owned()
                .collect();
            (
                format!("{}{prefix}", url.origin().ascii_serialization()),
                params,
            )
        }
        _ => return Err(invalid()),
    };
    let param = |name: &str| {
        params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
            .ok_or_else(invalid)
    };
    if param("v").ok().as_deref() != Some(INVITE_VERSION) {
        return Err(
            "This invite is for a different version of ezcount. Ask for a new one.".to_string(),
        );
    }
    let secret = Secret::new(param("k")?);
    GroupKeys::derive(&secret).map_err(|_| invalid())?;
    Ok(Invite {
        server_url: normalize_server_url(&server)?,
        group_id: param("g")?,
        secret,
    })
}

pub fn normalize_server_url(raw: &str) -> Res<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    let url = Url::parse(trimmed).map_err(|_| format!("'{trimmed}' is not a valid server URL"))?;
    match url.scheme() {
        "https" => {}
        // Plain HTTP would send login and group tokens readable to anyone on the way.
        "http" if is_local(&url) => {}
        "http" => {
            return Err(
                "Use an https:// address. Plain http:// only works for a server on \
                        this device or your local network: over the internet it would send \
                        your login unencrypted."
                    .to_string(),
            )
        }
        _ => return Err("The server URL must start with https://".to_string()),
    }
    Ok(trimmed.to_string())
}

/// This device or a private network (home network, emulator, Tailscale): where plain HTTP to
/// a relay you run yourself is acceptable.
fn is_local(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(ip)) => {
            let [a, b, ..] = ip.octets();
            // 100.64.0.0/10: carrier-grade NAT, which Tailscale uses for its private network.
            ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || (a == 100 && b & 0xc0 == 64)
        }
        Some(url::Host::Ipv6(ip)) => {
            let first = ip.segments()[0];
            // fc00::/7 unique local, fe80::/10 link-local.
            ip.is_loopback() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
        }
        Some(url::Host::Domain(name)) => {
            name == "localhost" || name.ends_with(".localhost") || name.ends_with(".local")
        }
        None => false,
    }
}

pub fn new_secret() -> Res<Secret> {
    Secret::generate()
}

// ---------------------------------------------------------------------------
// Account credentials
// ---------------------------------------------------------------------------

/// Lowercases and checks a username: 3 to 32 of `a-z 0-9 . _ -`, starting with a letter or
/// digit. The relay applies the same rule.
pub fn normalize_username(raw: &str) -> Res<String> {
    let username = raw.trim().to_lowercase();
    let bytes = username.as_bytes();
    let valid = (3..=32).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes.iter().all(|&b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        });
    if valid {
        Ok(username)
    } else {
        Err("Usernames are 3 to 32 letters, digits, dots, dashes or underscores".to_string())
    }
}

/// Lowest zxcvbn score accepted at sign-up. 3 is "safely unguessable": over 10^8 guesses,
/// which matters because a stolen relay database lets an attacker guess offline.
const MIN_PASSWORD_SCORE: u8 = 3;

/// How hard `password` is to guess, with the username and the app's name counted as known
/// to an attacker. zxcvbn looks at the first 100 characters only, so this stays fast.
pub fn password_strength(password: &str, username: &str) -> PasswordStrength {
    let username = username.trim().to_lowercase();
    let entropy = zxcvbn::zxcvbn(password, &[&username, "ezcount"]);
    let score = u8::from(entropy.score());
    let feedback = entropy.feedback();
    PasswordStrength {
        score,
        acceptable: password.chars().count() >= MIN_PASSWORD_LEN && score >= MIN_PASSWORD_SCORE,
        warning: feedback.and_then(|f| f.warning()).map(|w| w.to_string()),
        suggestions: feedback
            .map(|f| f.suggestions().iter().map(ToString::to_string).collect())
            .unwrap_or_default(),
    }
}

fn check_password(password: &str, username: &str) -> Res<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!(
            "Use a password of at least {MIN_PASSWORD_LEN} characters"
        ));
    }
    let strength = password_strength(password, username);
    if !strength.acceptable {
        let why = strength
            .warning
            .map(|w| format!(" {w}"))
            .unwrap_or_default();
        return Err(format!(
            "This password is too easy to guess.{why} Try a few unrelated words."
        ));
    }
    Ok(())
}

/// Argon2 takes a noticeable fraction of a second, so it runs on a blocking thread.
#[cfg(not(target_family = "wasm"))]
async fn password_keys(username: &str, password: &str) -> Res<CredentialKeys> {
    let (username, password) = (username.to_string(), password.to_string());
    tokio::task::spawn_blocking(move || CredentialKeys::from_password(&username, &password))
        .await
        .map_err(|e| format!("Could not derive keys from the password: {e}"))?
}

/// The browser has no blocking threads, but the core runs in a Web Worker there, so hashing
/// doesn't freeze the page.
#[cfg(target_family = "wasm")]
async fn password_keys(username: &str, password: &str) -> Res<CredentialKeys> {
    CredentialKeys::from_password(username, password)
}

// ---------------------------------------------------------------------------
// HTTP protocol
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct UpdatesPage {
    updates: Vec<RemoteUpdate>,
    has_more: bool,
    // Missing from relays older than this field.
    #[serde(default)]
    relay_id: Option<String>,
}

/// One decrypted page of updates.
struct Page {
    updates: Vec<(i64, Vec<u8>)>,
    has_more: bool,
    relay_id: Option<String>,
}

#[derive(Deserialize)]
struct RemoteUpdate {
    seq: i64,
    data: String,
}

#[derive(Serialize)]
struct SignUpRequest<'a> {
    username: &'a str,
    login_token: &'a str,
    wrapped_key: String,
    account_id: &'a str,
    doc_token: &'a str,
    recovery_token: &'a str,
    recovery_wrapped_key: String,
}

#[derive(Deserialize, Default)]
struct SignUpResponse {
    /// Relays before recovery keys answer with an empty body, and ignore the key.
    #[serde(default)]
    recovery: bool,
}

#[derive(Serialize)]
struct LogInRequest<'a> {
    username: &'a str,
    login_token: &'a str,
}

#[derive(Deserialize)]
struct LogInResponse {
    account_id: String,
    wrapped_key: String,
}

#[derive(Serialize)]
struct RecoverRequest<'a> {
    username: &'a str,
    recovery_token: &'a str,
}

/// See `POST /v1/accounts/credentials` in the relay: one proof, one or two replacements.
#[derive(Serialize, Default)]
struct CredentialsRequest<'a> {
    username: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    login_token: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_token: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_login_token: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_wrapped_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_recovery_token: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_recovery_wrapped_key: Option<String>,
}

fn updates_url(server_url: &str, group_id: &str) -> String {
    format!("{server_url}/v1/groups/{group_id}/updates")
}

fn request_err(e: reqwest::Error) -> String {
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

async fn check_status(response: reqwest::Response) -> Res<reqwest::Response> {
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

const NOT_ON_SERVER: &str = "The sync server does not know this group";
/// A relay from before accounts answers 404 on the account endpoints.
const NO_ACCOUNTS: &str = "This server doesn't support accounts. Update the ezcount relay.";

/// Encrypts and uploads one update.
async fn push(
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
async fn pull_page(
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
const MAX_PAGES_PER_SYNC: usize = 50;

/// Runs blocking storage work (SQLite commits wait for the disk) without stalling the
/// other tasks on this async worker thread.
#[cfg(not(target_family = "wasm"))]
fn blocking<T>(work: impl FnOnce() -> T) -> T {
    use tokio::runtime::{Handle, RuntimeFlavor};
    match Handle::try_current().map(|h| h.runtime_flavor()) {
        Ok(RuntimeFlavor::MultiThread) => tokio::task::block_in_place(work),
        _ => work(),
    }
}

/// The browser's one thread is the Web Worker running the core: nothing else to keep going.
#[cfg(target_family = "wasm")]
fn blocking<T>(work: impl FnOnce() -> T) -> T {
    work()
}

/// Downloads a document, or its first `MAX_PAGES_PER_SYNC` pages: syncing it once stored
/// fetches the rest. Returns it with the matching sync state.
async fn download(
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

// ---------------------------------------------------------------------------
// Group sync
// ---------------------------------------------------------------------------

/// Pushes local changes and pulls remote ones for a shared group (or the account document).
/// Returns whether remote changes were applied.
pub async fn sync_group(state: &AppState, group_id: &str) -> Res<bool> {
    // One sync at a time, so the background loop and "Sync now" never push the same changes twice.
    let _guard = state.sync_lock.lock().await;
    let result = sync_group_inner(state, group_id).await;
    state.store().record_sync_result(group_id, &result);
    result
}

async fn sync_group_inner(state: &AppState, group_id: &str) -> Res<bool> {
    let not_shared = || "This group is not shared".to_string();
    let meta = state
        .store()
        .sync_meta(group_id)
        .ok_or_else(not_shared)?
        .clone();
    let keys = GroupKeys::derive(&meta.secret)?;
    let mut changed = false;

    // Before uploading, check the relay still holds what this device thinks it does. After the
    // relay lost its data, or was replaced by an empty one, this device uploads everything
    // again, so the group survives as long as one device still has it.
    match pull_page(&state.http, &meta.server_url, &keys, group_id, meta.cursor).await {
        Ok(page) => {
            let restarted = state
                .store()
                .check_relay(group_id, page.relay_id.as_deref())?;
            // After a restart the page's sequence numbers mean nothing; read again below.
            if !restarted && !page.updates.is_empty() {
                changed |= blocking(|| state.store().import_remote(group_id, &page.updates))?;
            }
        }
        // Normal for a group never uploaded yet: the first upload registers it.
        Err(e) if e == NOT_ON_SERVER => {
            if meta.used_relay() {
                state.store().restart_sync(group_id)?;
            }
        }
        Err(e) => return Err(e),
    }

    let (meta, outgoing) = {
        let store = state.store();
        let meta = store.sync_meta(group_id).ok_or_else(not_shared)?.clone();
        let doc = store.doc(group_id)?;
        let local = doc.oplog_vv();
        let outgoing = if meta.server_vv.includes_vv(&local) {
            None
        } else {
            let bytes = doc
                .export(ExportMode::updates(&meta.server_vv))
                .map_err(|e| format!("Could not encode local changes: {e}"))?;
            Some((bytes, local))
        };
        (meta, outgoing)
    };

    if let Some((bytes, pushed)) = outgoing {
        push(&state.http, &meta.server_url, &keys, group_id, &bytes).await?;
        blocking(|| state.store().mark_pushed(group_id, &pushed))?;
    }

    for _ in 0..MAX_PAGES_PER_SYNC {
        let after = state
            .store()
            .sync_meta(group_id)
            .map(|m| m.cursor)
            .ok_or_else(not_shared)?;
        let page = pull_page(&state.http, &meta.server_url, &keys, group_id, after).await?;
        blocking(|| {
            state
                .store()
                .check_relay(group_id, page.relay_id.as_deref())
        })?;
        if page.updates.is_empty() {
            break;
        }
        changed |= blocking(|| state.store().import_remote(group_id, &page.updates))?;
        if !page.has_more {
            break;
        }
    }
    Ok(changed)
}

/// Syncs the account document. Returns whether another device changed it.
pub async fn sync_account(state: &AppState) -> Res<bool> {
    let Some(account_id) = state.store().session().map(|s| s.account_id.clone()) else {
        return Ok(false);
    };
    sync_group(state, &account_id).await
}

/// Makes this device's groups match the account: downloads groups added on other devices and
/// removes the ones left there. Groups that can't be downloaded yet are retried next time.
/// Returns whether the list of groups changed.
pub async fn reconcile(state: &AppState) -> Res<bool> {
    let (listed, local) = {
        let store = state.store();
        if store.session().is_none() {
            return Ok(false);
        }
        (account::groups(store.account_doc()?)?, store.group_ids())
    };
    let mut changed = false;

    for id in local
        .iter()
        .filter(|id| !listed.iter().any(|g| &g.group_id == *id))
    {
        // Left on another device. Upload what this device still had first, for the others,
        // and keep the group until that worked: deleting it earlier would lose those edits.
        let uploaded = sync_group(state, id).await;
        let mut store = state.store();
        if store.has_unpushed(id) {
            let reason = uploaded.err().unwrap_or_default();
            eprintln!("[sync] keeping left group {id} until its changes are uploaded: {reason}");
            continue;
        }
        store.delete(id)?;
        changed = true;
    }

    for entry in listed.iter().filter(|g| !local.contains(&g.group_id)) {
        match download(
            &state.http,
            &entry.server_url,
            &entry.secret,
            &entry.group_id,
        )
        .await
        {
            Ok((doc, meta)) => {
                if doc::read_group(&doc).map(|g| g.id).as_deref() != Ok(entry.group_id.as_str()) {
                    eprintln!("[sync] group {} has no usable data yet", entry.group_id);
                    continue;
                }
                let mut store = state.store();
                if !store.contains(&entry.group_id) {
                    store.insert(doc, Some(meta))?;
                    changed = true;
                }
            }
            // Created on another device that hasn't uploaded it yet.
            Err(e) if e == NOT_ON_SERVER => {}
            Err(e) => eprintln!("[sync] could not download group {}: {e}", entry.group_id),
        }
    }
    Ok(changed)
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

/// Creates an account on `server_url` and logs this device into it. Groups already on the
/// device become part of the account.
pub async fn sign_up(
    state: &AppState,
    server_url: &str,
    username: &str,
    password: &str,
) -> Res<Option<String>> {
    let server_url = normalize_server_url(server_url)?;
    let username = normalize_username(username)?;
    check_password(password, &username)?;
    ensure_logged_out(state)?;

    let keys = password_keys(&username, password).await?;
    let account_id = uuid::Uuid::new_v4().to_string();
    let account_key = new_secret()?;
    let doc_keys = GroupKeys::derive(&account_key)?;
    let recovery_key = new_recovery_key()?;
    let recovery = CredentialKeys::from_recovery_key(&recovery_key)?;

    let response = state
        .http
        .post(format!("{server_url}/v1/accounts"))
        .json(&SignUpRequest {
            username: &username,
            login_token: &keys.token,
            wrapped_key: STANDARD.encode(keys.wrap_account_key(&account_id, &account_key)?),
            account_id: &account_id,
            doc_token: &doc_keys.auth_token,
            recovery_token: &recovery.token,
            recovery_wrapped_key: STANDARD
                .encode(recovery.wrap_account_key(&account_id, &account_key)?),
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    let created: SignUpResponse = match response.status().as_u16() {
        409 => return Err(format!("The username \"{username}\" is already taken")),
        404 => return Err(NO_ACCOUNTS.to_string()),
        _ => {
            let body = check_status(response)
                .await?
                .text()
                .await
                .unwrap_or_default();
            serde_json::from_str(&body).unwrap_or_default()
        }
    };

    let session = Session {
        server_url: server_url.clone(),
        username,
        account_id,
    };
    state.store().set_session(
        session,
        LoroDoc::new(),
        SyncMeta::new(server_url, account_key),
    )?;
    adopt_local_groups(state)?;
    // Upload the new account right away; the background loop retries if this fails.
    let _ = sync_account(state).await;
    // Only a relay that stored it makes the recovery key worth showing.
    Ok(created.recovery.then_some(recovery_key))
}

/// Sends a `POST /v1/accounts/credentials`. `wrong_proof` is the message for a rejected one.
async fn update_credentials(
    state: &AppState,
    server_url: &str,
    request: &CredentialsRequest<'_>,
    wrong_proof: &str,
) -> Res<()> {
    let response = state
        .http
        .post(format!("{server_url}/v1/accounts/credentials"))
        .json(request)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    match response.status().as_u16() {
        401 => Err(wrong_proof.to_string()),
        404 => Err(OLD_RELAY.to_string()),
        429 => Err(TOO_MANY_ATTEMPTS.to_string()),
        _ => check_status(response).await.map(|_| ()),
    }
}

const OLD_RELAY: &str =
    "This server can't change passwords or recovery keys yet. Update the ezcount relay.";
const TOO_MANY_ATTEMPTS: &str = "Too many failed attempts. Wait a few minutes and try again.";

/// The logged-in account's username, relay, id and key.
fn account_credentials(state: &AppState) -> Res<(String, String, String, Secret)> {
    let store = state.store();
    let session = store
        .session()
        .ok_or_else(|| "You are not logged in".to_string())?;
    let key = store
        .sync_meta(&session.account_id)
        .map(|meta| meta.secret.clone())
        .ok_or_else(|| "This device has no key for your account".to_string())?;
    Ok((
        session.username.clone(),
        session.server_url.clone(),
        session.account_id.clone(),
        key,
    ))
}

/// Changes the password. Other devices stay logged in: the account key doesn't change, only
/// the copy of it the password unlocks.
pub async fn change_password(state: &AppState, current: &str, new: &str) -> Res<()> {
    let (username, server_url, account_id, account_key) = account_credentials(state)?;
    check_password(new, &username)?;
    let old = password_keys(&username, current).await?;
    let keys = password_keys(&username, new).await?;
    let request = CredentialsRequest {
        username: &username,
        login_token: Some(&old.token),
        new_login_token: Some(&keys.token),
        new_wrapped_key: Some(STANDARD.encode(keys.wrap_account_key(&account_id, &account_key)?)),
        ..Default::default()
    };
    update_credentials(
        state,
        &server_url,
        &request,
        "Your current password is wrong",
    )
    .await
}

/// Replaces the recovery key, or gives an account created before recovery keys its first one.
/// The old key stops working. Returns the new one, to show once.
pub async fn replace_recovery_key(state: &AppState, password: &str) -> Res<String> {
    let (username, server_url, account_id, account_key) = account_credentials(state)?;
    let keys = password_keys(&username, password).await?;
    let recovery_key = new_recovery_key()?;
    let recovery = CredentialKeys::from_recovery_key(&recovery_key)?;
    let request = CredentialsRequest {
        username: &username,
        login_token: Some(&keys.token),
        new_recovery_token: Some(&recovery.token),
        new_recovery_wrapped_key: Some(
            STANDARD.encode(recovery.wrap_account_key(&account_id, &account_key)?),
        ),
        ..Default::default()
    };
    update_credentials(state, &server_url, &request, "Wrong password").await?;
    Ok(recovery_key)
}

/// Sets a new password with the recovery key, for a forgotten password, and logs this device
/// in. A recovery key works once: returns its replacement, to show once.
pub async fn recover_account(
    state: &AppState,
    server_url: &str,
    username: &str,
    recovery_key: &str,
    new_password: &str,
) -> Res<String> {
    let wrong = "Wrong username or recovery key";
    let server_url = normalize_server_url(server_url)?;
    let username = normalize_username(username).map_err(|_| wrong.to_string())?;
    let recovery = CredentialKeys::from_recovery_key(recovery_key)?;
    check_password(new_password, &username)?;
    ensure_logged_out(state)?;

    let response = state
        .http
        .post(format!("{server_url}/v1/accounts/recover"))
        .json(&RecoverRequest {
            username: &username,
            recovery_token: &recovery.token,
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    let found: LogInResponse = match response.status().as_u16() {
        401 => return Err(wrong.to_string()),
        404 => return Err(OLD_RELAY.to_string()),
        429 => return Err(TOO_MANY_ATTEMPTS.to_string()),
        _ => check_status(response)
            .await?
            .json()
            .await
            .map_err(|e| format!("The sync server sent an unexpected response: {e}"))?,
    };
    let wrapped = STANDARD
        .decode(&found.wrapped_key)
        .map_err(|_| "The sync server sent corrupt account data".to_string())?;
    let account_key = recovery.unwrap_account_key(&found.account_id, &wrapped)?;

    let keys = password_keys(&username, new_password).await?;
    let next_key = new_recovery_key()?;
    let next = CredentialKeys::from_recovery_key(&next_key)?;
    let request = CredentialsRequest {
        username: &username,
        recovery_token: Some(&recovery.token),
        new_login_token: Some(&keys.token),
        new_wrapped_key: Some(
            STANDARD.encode(keys.wrap_account_key(&found.account_id, &account_key)?),
        ),
        new_recovery_token: Some(&next.token),
        new_recovery_wrapped_key: Some(
            STANDARD.encode(next.wrap_account_key(&found.account_id, &account_key)?),
        ),
        ..Default::default()
    };
    update_credentials(state, &server_url, &request, wrong).await?;

    // The new password is set; if this download fails, logging in with it finishes the job.
    let (doc, meta) = download(&state.http, &server_url, &account_key, &found.account_id).await?;
    let session = Session {
        server_url,
        username,
        account_id: found.account_id,
    };
    state.store().set_session(session, doc, meta)?;
    adopt_local_groups(state)?;
    state.sync_wakeup.notify_one();
    Ok(next_key)
}

/// Logs this device into an existing account. Its groups download in the background.
pub async fn log_in(state: &AppState, server_url: &str, username: &str, password: &str) -> Res<()> {
    let server_url = normalize_server_url(server_url)?;
    let username =
        normalize_username(username).map_err(|_| "Wrong username or password".to_string())?;
    ensure_logged_out(state)?;

    let keys = password_keys(&username, password).await?;
    let response = state
        .http
        .post(format!("{server_url}/v1/accounts/login"))
        .json(&LogInRequest {
            username: &username,
            login_token: &keys.token,
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    let found: LogInResponse = match response.status().as_u16() {
        401 => return Err("Wrong username or password".to_string()),
        404 => return Err(NO_ACCOUNTS.to_string()),
        429 => return Err(TOO_MANY_ATTEMPTS.to_string()),
        _ => check_status(response)
            .await?
            .json()
            .await
            .map_err(|e| format!("The sync server sent an unexpected response: {e}"))?,
    };
    let wrapped = STANDARD
        .decode(&found.wrapped_key)
        .map_err(|_| "The sync server sent corrupt account data".to_string())?;
    let account_key = keys.unwrap_account_key(&found.account_id, &wrapped)?;

    // Download the account before saving anything, so a failure leaves the device as it was.
    let (doc, meta) = download(&state.http, &server_url, &account_key, &found.account_id).await?;
    let session = Session {
        server_url,
        username,
        account_id: found.account_id,
    };
    state.store().set_session(session, doc, meta)?;
    adopt_local_groups(state)?;
    state.sync_wakeup.notify_one();
    Ok(())
}

// ---------------------------------------------------------------------------
// Login links
// ---------------------------------------------------------------------------

/// What a login link hands to the other device, encrypted with the link's code.
#[derive(Serialize, Deserialize)]
struct LinkedAccount {
    username: String,
    account_id: String,
    account_key: String,
}

#[derive(Serialize)]
struct CreateLinkRequest<'a> {
    username: &'a str,
    login_token: &'a str,
    ticket: &'a str,
    data: String,
}

#[derive(Deserialize)]
struct CreateLinkResponse {
    expires_in: u32,
}

#[derive(Serialize)]
struct ClaimLinkRequest<'a> {
    ticket: &'a str,
}

#[derive(Deserialize)]
struct ClaimLinkResponse {
    data: String,
}

/// A relay from before login links has no such endpoints.
const NO_LINKS: &str =
    "This server can't connect devices with a code yet. Update the ezcount relay.";

/// A login link: `ezcount://login?server=<relay>&code=<code>`. The code never reaches the
/// relay, only the device that scans the link.
fn login_link(server_url: &str, code: &Secret) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("server", server_url)
        .append_pair("code", code.expose())
        .finish();
    format!("ezcount://login?{query}")
}

/// The relay and the code of a login link.
fn parse_login_link(link: &str) -> Res<(String, Secret)> {
    let invalid = || "This is not an ezcount login code".to_string();
    let url = Url::parse(link.trim()).map_err(|_| invalid())?;
    if url.scheme() != "ezcount" || url.host_str() != Some("login") {
        return Err(invalid());
    }
    let param = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
            .ok_or_else(invalid)
    };
    Ok((
        normalize_server_url(&param("server")?)?,
        Secret::new(param("code")?),
    ))
}

/// Makes a link that logs another device into this account, once and for a short time (the
/// relay says how long). It asks for the password, so an app left open isn't enough to take
/// the account elsewhere.
///
/// The account's key waits on the relay encrypted with the link's code, which only the link
/// carries: the relay can't read it, and forgets it when it is fetched or expires.
pub async fn create_login_link(state: &AppState, password: &str) -> Res<LoginLink> {
    let (username, server_url, account_id, account_key) = account_credentials(state)?;
    let keys = password_keys(&username, password).await?;
    let code = new_secret()?;
    let link_keys = LinkKeys::derive(&code)?;
    let account = serde_json::to_vec(&LinkedAccount {
        username: username.clone(),
        account_id,
        account_key: account_key.expose().to_string(),
    })
    .map_err(|e| format!("Could not encode the account: {e}"))?;

    let response = state
        .http
        .post(format!("{server_url}/v1/accounts/links"))
        .json(&CreateLinkRequest {
            username: &username,
            login_token: &keys.token,
            ticket: &link_keys.ticket,
            data: STANDARD.encode(link_keys.seal(&account)?),
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    let created: CreateLinkResponse = match response.status().as_u16() {
        401 => return Err("Wrong password".to_string()),
        404 | 405 => return Err(NO_LINKS.to_string()),
        429 => return Err(TOO_MANY_ATTEMPTS.to_string()),
        _ => check_status(response)
            .await?
            .json()
            .await
            .map_err(|e| format!("The sync server sent an unexpected response: {e}"))?,
    };
    Ok(LoginLink {
        link: login_link(&server_url, &code),
        expires_in: created.expires_in,
    })
}

/// Logs this device into the account a login link is for (see [`create_login_link`]). Its
/// groups download in the background.
pub async fn log_in_with_link(state: &AppState, link: &str) -> Res<()> {
    let (server_url, code) = parse_login_link(link)?;
    let link_keys = LinkKeys::derive(&code)?;
    ensure_logged_out(state)?;

    let response = state
        .http
        .post(format!("{server_url}/v1/accounts/links/claim"))
        .json(&ClaimLinkRequest {
            ticket: &link_keys.ticket,
        })
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(request_err)?;
    let claimed: ClaimLinkResponse = match response.status().as_u16() {
        410 => {
            return Err(
                "This code has expired or was already used. Show a new one and scan it."
                    .to_string(),
            )
        }
        404 | 405 => return Err(NO_LINKS.to_string()),
        _ => check_status(response)
            .await?
            .json()
            .await
            .map_err(|e| format!("The sync server sent an unexpected response: {e}"))?,
    };
    let corrupt = || "The sync server sent corrupt account data".to_string();
    let sealed = STANDARD.decode(&claimed.data).map_err(|_| corrupt())?;
    let account: LinkedAccount =
        serde_json::from_slice(&link_keys.open(&sealed)?).map_err(|_| corrupt())?;
    let account_key = Secret::new(account.account_key);
    GroupKeys::derive(&account_key).map_err(|_| corrupt())?;

    // Download the account before saving anything, so a failure leaves the device as it was.
    let (doc, meta) = download(&state.http, &server_url, &account_key, &account.account_id).await?;
    let session = Session {
        server_url,
        username: account.username,
        account_id: account.account_id,
    };
    state.store().set_session(session, doc, meta)?;
    adopt_local_groups(state)?;
    state.sync_wakeup.notify_one();
    Ok(())
}

/// Logs out and removes the account's data from this device. Refuses while edits are not
/// uploaded yet, unless `force` is set.
pub async fn log_out(state: &AppState, force: bool) -> Res<()> {
    if !force {
        let _ = sync_account(state).await;
        let ids = state.store().synced_ids();
        for id in ids {
            let _ = sync_group(state, &id).await;
        }
        if state.store().has_unpushed_changes() {
            return Err(
                "Some changes on this device are not uploaded yet. Connect to the internet and \
                 try again, or log out anyway and lose them."
                    .to_string(),
            );
        }
    }
    let _guard = state.sync_lock.lock().await;
    state.store().wipe()
}

fn ensure_logged_out(state: &AppState) -> Res<()> {
    if state.store().session().is_some() {
        return Err("This device is already logged in".to_string());
    }
    Ok(())
}

fn require_session(state: &AppState) -> Res<Session> {
    state
        .store()
        .session()
        .cloned()
        .ok_or_else(|| "Log in first".to_string())
}

/// Adds the groups on this device to the account, sharing the ones that weren't yet.
fn adopt_local_groups(state: &AppState) -> Res<()> {
    let mut store = state.store();
    let server_url = store
        .session()
        .map(|s| s.server_url.clone())
        .ok_or_else(|| "Log in first".to_string())?;
    let listed: Vec<String> = account::groups(store.account_doc()?)?
        .into_iter()
        .map(|g| g.group_id)
        .collect();
    for id in store.group_ids() {
        if listed.contains(&id) {
            continue;
        }
        if store.sync_meta(&id).is_none() {
            store.set_sync(&id, SyncMeta::new(server_url.clone(), new_secret()?))?;
        }
        let meta = store.sync_meta(&id).cloned().expect("just set");
        store.update_account(|d| account::add_group(d, &id, &meta.server_url, &meta.secret))?;
    }
    state.sync_wakeup.notify_one();
    Ok(())
}

// ---------------------------------------------------------------------------
// Group membership
// ---------------------------------------------------------------------------

/// Creates a group shared through the account's relay. The first participant is the user.
pub fn create_group(
    state: &AppState,
    name: &str,
    currency: &str,
    participants: &[String],
) -> Res<Group> {
    require_session(state)?;
    let doc = doc::new_group_doc(name, currency, participants)?;
    let me = doc::read_group(&doc)?
        .participants
        .first()
        .ok_or_else(|| "Add yourself to the group".to_string())?
        .id
        .clone();
    add_new_group(state, doc, Some(&me))
}

/// Creates a group from a CSV file (see `csv_file`). Who the user is in it isn't known yet.
pub fn import_group(state: &AppState, name: &str, csv: &str) -> Res<Group> {
    require_session(state)?;
    let imported = csv_file::import(csv)?;
    let doc = doc::imported_group_doc(
        name,
        &imported.currency,
        &imported.participants,
        &imported.expenses,
    )?;
    add_new_group(state, doc, None)
}

/// Adds a group made on this device to the account, with `me` as the user in it.
fn add_new_group(state: &AppState, doc: LoroDoc, me: Option<&str>) -> Res<Group> {
    let session = require_session(state)?;
    let group = doc::read_group(&doc)?;
    let meta = SyncMeta::new(session.server_url, new_secret()?);

    // Account first: if the app stops in between, the group is fetched again from the
    // account instead of being dropped as "left on another device".
    let mut store = state.store();
    store.update_account(|d| {
        account::add_group(d, &group.id, &meta.server_url, &meta.secret)?;
        match me {
            Some(me) => account::set_identity(d, &group.id, me),
            None => Ok(()),
        }
    })?;
    let mut inserted = store.insert(doc, Some(meta));
    if inserted.is_err() {
        let _ = store.update_account(|d| account::remove_group(d, &group.id));
    } else if let Some(me) = me {
        let profile = account::profile(store.account_doc()?);
        // The name typed for the group is kept: only the picture comes from the profile.
        let picture = profile.avatar.as_deref();
        if let Ok(with_picture) =
            store.update(&group.id, |d| doc::set_participant_avatar(d, me, picture))
        {
            inserted = Ok(with_picture);
        }
    }
    drop(store);
    state.sync_wakeup.notify_one();
    inserted
}

#[derive(Deserialize)]
struct RateResponse {
    rate: String,
}

/// The exchange rate the account's relay suggests from one currency to another on a day
/// (`YYYY-MM-DD`): how much of `to` one unit of `from` is worth. `None` when it has no
/// suggestion, as on a relay from before them.
pub async fn suggested_rate(
    state: &AppState,
    from: &str,
    to: &str,
    date: Option<&str>,
) -> Res<Option<String>> {
    let session = require_session(state)?;
    // They go into the address.
    let code = |c: &str| c.len() == 3 && c.bytes().all(|b| b.is_ascii_alphabetic());
    if !code(from) || !code(to) {
        return Ok(None);
    }
    let mut url = format!("{}/v1/rates/{from}/{to}", session.server_url);
    if let Some(date) = date.filter(|d| d.bytes().all(|b| b.is_ascii_digit() || b == b'-')) {
        url.push_str(&format!("?date={date}"));
    }
    let response = state.http.get(url).send().await.map_err(request_err)?;
    match response.status().as_u16() {
        200 => {}
        404 => return Ok(None),
        status => return Err(format!("The sync server answered {status} for the rate")),
    }
    let answer: RateResponse = response.json().await.map_err(request_err)?;
    // Only what an expense accepts.
    Ok(doc::exchange_rate(&answer.rate).ok())
}

/// Downloads a shared group from its invite code and adds it to the account.
pub async fn join_group(state: &AppState, code: &str) -> Res<Group> {
    let invite = parse_invite(code)?;
    require_session(state)?;
    let already = || "This group is already in your account".to_string();
    if state.store().contains(&invite.group_id) {
        return Err(already());
    }

    let (doc, meta) = download(
        &state.http,
        &invite.server_url,
        &invite.secret,
        &invite.group_id,
    )
    .await?;
    let group = doc::read_group(&doc)
        .map_err(|_| "The sync server has no usable data for this group".to_string())?;
    if group.deleted {
        return Err("This group was deleted".to_string());
    }
    if group.id != invite.group_id {
        return Err("The invite code does not match the group on the server".to_string());
    }

    let mut store = state.store();
    if store.contains(&invite.group_id) {
        return Err(already());
    }
    store.update_account(|d| {
        account::add_group(d, &invite.group_id, &invite.server_url, &invite.secret)
    })?;
    let group = store.insert(doc, Some(meta))?;
    drop(store);
    state.sync_wakeup.notify_one();
    Ok(group)
}

/// Removes a group from the account, on every device. Other members keep it.
pub async fn leave_group(state: &AppState, group_id: &str) -> Res<()> {
    // Hand over this device's last edits to the other members first, if possible.
    if state.store().sync_meta(group_id).is_some() {
        let _ = sync_group(state, group_id).await;
    }
    let mut store = state.store();
    if store.session().is_some() {
        store.update_account(|d| account::remove_group(d, group_id))?;
    }
    store.delete(group_id)?;
    drop(store);
    state.sync_wakeup.notify_one();
    Ok(())
}

/// Deletes a group for everyone, or asks to: with balances that aren't settled it takes every
/// member's agreement, and this gives the user's. Returns the group while it waits for the
/// others, and nothing once it is deleted.
pub fn delete_group(state: &AppState, group_id: &str) -> Res<Option<Group>> {
    require_session(state)?;
    let mut store = state.store();
    let me = account::identities(store.account_doc()?)?.remove(group_id);
    let mut deleted = false;
    let group = store.update(group_id, |d| {
        deleted = doc::delete_or_vote(d, me.as_deref())?;
        Ok(())
    })?;
    drop(store);
    // The next sync tells the others, then drops the group here (`drop_deleted`).
    state.sync_wakeup.notify_one();
    Ok((!deleted).then_some(group))
}

/// Puts a group away for the user, on all their devices, or back among the others.
pub fn set_group_archived(state: &AppState, group_id: &str, archived: bool) -> Res<()> {
    let mut store = state.store();
    store.doc(group_id)?;
    store.update_account(|d| account::set_archived(d, group_id, archived))?;
    drop(store);
    state.sync_wakeup.notify_one();
    Ok(())
}

/// Drops a group that was deleted for everyone, once this device has nothing left to upload
/// for it (its own deletion included). Returns whether it did.
fn drop_deleted(state: &AppState, group_id: &str) -> bool {
    let mut store = state.store();
    if !store.group(group_id).is_ok_and(|g| g.deleted) || store.has_unpushed(group_id) {
        return false;
    }
    if store.session().is_some() {
        let _ = store.update_account(|d| account::remove_group(d, group_id));
    }
    store.delete(group_id).is_ok()
}

/// Records which participant the user is in a group, for all their devices.
pub fn set_identity(state: &AppState, group_id: &str, participant_id: &str) -> Res<()> {
    let mut store = state.store();
    let group = store.group(group_id)?;
    if !group
        .participants
        .iter()
        .any(|p| p.id == participant_id && !p.removed)
    {
        return Err("This person is not a member of the group".to_string());
    }
    let previous = account::identities(store.account_doc()?)?.remove(group_id);
    store.update_account(|d| account::set_identity(d, group_id, participant_id))?;
    // The profile follows the user: off the member they said they were before, onto this one.
    if let Some(previous) = previous.filter(|previous| previous != participant_id) {
        let _ = store.update(group_id, |d| {
            doc::set_participant_avatar(d, &previous, None)
        });
    }
    let profile = account::profile(store.account_doc()?);
    let _ = show_profile(&mut store, group_id, participant_id, &profile);
    drop(store);
    state.sync_wakeup.notify_one();
    Ok(())
}

/// Gives a member of a group the profile's name and picture. Without a name in the profile,
/// the member keeps theirs.
fn show_profile(
    store: &mut Store,
    group_id: &str,
    participant_id: &str,
    profile: &account::Profile,
) -> Res<()> {
    store
        .update(group_id, |d| {
            if let Some(name) = &profile.name {
                doc::rename_participant(d, participant_id, name)?;
            }
            doc::set_participant_avatar(d, participant_id, profile.avatar.as_deref())
        })
        .map(|_| ())
}

/// Sets the name and picture the user shows, and gives them to the member they are in each
/// of their groups, for the other members to see.
pub fn update_profile(state: &AppState, name: &str, avatar: Option<&str>) -> Res<()> {
    require_session(state)?;
    let mut store = state.store();
    store.update_account(|d| account::set_profile(d, name, avatar))?;
    let profile = account::profile(store.account_doc()?);
    for (group_id, participant_id) in account::identities(store.account_doc()?)? {
        // A group that is gone, or a member that was removed, doesn't keep the others from
        // getting it.
        let _ = show_profile(&mut store, &group_id, &participant_id, &profile);
    }
    drop(store);
    state.sync_wakeup.notify_one();
    Ok(())
}

/// Adds the user to a group as a new participant.
pub fn add_self(state: &AppState, group_id: &str, name: &str) -> Res<Group> {
    require_session(state)?;
    let mut new_id = String::new();
    state.mutate(group_id, |d| {
        new_id = doc::add_participant(d, name, doc::AddedBy::Themselves)?;
        Ok(())
    })?;
    set_identity(state, group_id, &new_id)?;
    state.store().group(group_id)
}

// ---------------------------------------------------------------------------
// Background sync
// ---------------------------------------------------------------------------

/// What a `sync_all` pass found, reported as it goes so the interface can refresh early.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    /// The account's groups or identities changed on another device.
    Account,
    /// One shared group was synced; `changed` if it received changes.
    Group { group_id: String, changed: bool },
}

/// One background pass: the account, then every shared group. The app runs it right after
/// local edits (`AppState::sync_wakeup`) and on a timer. Failures are kept in each group's
/// sync state for the interface, and the next pass retries.
pub async fn sync_all(state: &AppState, mut report: impl FnMut(SyncEvent)) {
    let account_changed = matches!(sync_account(state).await, Ok(true));
    let groups_changed = match reconcile(state).await {
        Ok(changed) => changed,
        Err(e) => {
            eprintln!("[sync] could not update groups from the account: {e}");
            false
        }
    };
    if account_changed || groups_changed {
        report(SyncEvent::Account);
    }

    let ids = state.store().synced_ids();
    for group_id in ids {
        let changed = matches!(sync_group(state, &group_id).await, Ok(true));
        if drop_deleted(state, &group_id) {
            state.sync_wakeup.notify_one();
            report(SyncEvent::Account);
            continue;
        }
        report(SyncEvent::Group { group_id, changed });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_code_round_trips() {
        let secret = new_secret().unwrap();
        for server in [
            "https://sync.example.com",
            "http://192.168.1.10:8787",
            "https://example.com/relay",
        ] {
            let code = invite_code(server, "g-1", &secret);
            assert!(code.starts_with(&format!("{server}/join#")), "{code}");
            let invite = parse_invite(&format!("  {code}\n")).unwrap();
            assert_eq!(invite.server_url, server);
            assert_eq!(invite.group_id, "g-1");
            assert_eq!(invite.secret, secret);
        }
    }

    #[test]
    fn accepts_app_links_from_the_join_page() {
        let secret = new_secret().unwrap();
        let mut url = Url::parse("ezcount://join").unwrap();
        url.query_pairs_mut()
            .append_pair("server", "https://sync.example.com")
            .append_pair("group", "g-1")
            .append_pair("key", secret.expose())
            .append_pair("v", "2");
        let invite = parse_invite(url.as_str()).unwrap();
        assert_eq!(invite.server_url, "https://sync.example.com");
        assert_eq!(invite.group_id, "g-1");
        assert_eq!(invite.secret, secret);
    }

    #[test]
    fn rejects_bad_invites_and_urls() {
        let secret = new_secret().unwrap();
        let without_version = invite_code("http://a", "g", &secret).replace("v=2&", "");
        let err = parse_invite(&without_version).err().unwrap();
        assert!(err.contains("different version"), "{err}");
        assert!(parse_invite(&invite_code(
            "http://a",
            "g",
            &Secret::new("too-short".into())
        ))
        .is_err());
        assert!(parse_invite("https://example.com/join?group=x").is_err());
        let elsewhere = invite_code("http://a", "g", &secret).replace("/join#", "/other#");
        assert!(parse_invite(&elsewhere).is_err());
        assert!(parse_invite("ezcount://join?server=http%3A%2F%2Fa&group=g").is_err());
        assert!(normalize_server_url("ftp://example.com").is_err());
        assert_eq!(
            normalize_server_url(" http://192.168.1.10:8787/ ").unwrap(),
            "http://192.168.1.10:8787"
        );
    }

    #[test]
    fn plain_http_only_on_this_device_or_a_private_network() {
        for local in [
            "http://localhost:8787",
            "http://127.0.0.1:8787",
            "http://192.168.1.10:8787",
            "http://10.0.2.2:8787",
            "http://172.20.0.5",
            "http://100.101.1.2:8787",
            "http://[::1]:8787",
            "http://[fd12::1]",
            "http://my-laptop.local:8787",
            "https://ezcount-relay.fly.dev",
            "https://203.0.113.9",
        ] {
            assert!(normalize_server_url(local).is_ok(), "{local}");
        }
        for public in [
            "http://ezcount-relay.fly.dev",
            "http://203.0.113.9:8787",
            "http://8.8.8.8",
            "http://[2001:db8::1]",
            "http://localhost.example.com",
        ] {
            let err = normalize_server_url(public).unwrap_err();
            assert!(err.contains("https://"), "{public}: {err}");
        }
        // Invites to such a server are refused too.
        let invite = invite_code("http://example.com", "g", &new_secret().unwrap());
        assert!(parse_invite(&invite).err().unwrap().contains("https://"));
    }

    #[test]
    fn secrets_stay_out_of_debug_output() {
        let secret = new_secret().unwrap();
        let meta = SyncMeta::new("https://relay".into(), secret.clone());
        let printed = format!("{secret:?} {meta:?}");
        assert!(!printed.contains(secret.expose()), "{printed}");
        assert!(printed.contains("Secret(…)"));
    }

    #[test]
    fn secrets_are_unique() {
        assert_ne!(new_secret().unwrap(), new_secret().unwrap());
    }

    #[test]
    fn usernames_are_normalized_and_checked() {
        assert_eq!(normalize_username("  Alice.B ").unwrap(), "alice.b");
        assert_eq!(normalize_username("bob_42").unwrap(), "bob_42");
        for bad in ["ab", "-alice", "al ice", "élodie", &"x".repeat(33)] {
            assert!(normalize_username(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn weak_passwords_are_refused() {
        for weak in [
            "password",
            "password123",
            "qwertyuiop",
            "alice2024",
            "ezcount2024",
        ] {
            let strength = password_strength(weak, "alice");
            assert!(!strength.acceptable, "{weak}: score {}", strength.score);
            assert!(check_password(weak, "alice").is_err(), "{weak}");
        }
        let common = password_strength("password", "alice");
        assert_eq!(common.score, 0);
        assert_eq!(
            common.warning.as_deref(),
            Some("This is a top-10 common password.")
        );
        let err = check_password("password", "alice").unwrap_err();
        assert!(
            err.contains("too easy to guess") && err.contains("top-10"),
            "{err}"
        );
        assert!(check_password("short", "alice")
            .unwrap_err()
            .contains("at least"));

        for strong in [
            "correct horse battery",
            "tangerine kayak mosaic",
            "v8#Lq2!mZr9@wT",
        ] {
            let strength = password_strength(strong, "alice");
            assert!(strength.acceptable, "{strong}: score {}", strength.score);
            assert!(check_password(strong, "alice").is_ok(), "{strong}");
        }
        // A strong password is still refused when it's mostly the username.
        assert!(!password_strength("maximilianmaximilian", "maximilian").acceptable);
    }
}

/// Devices syncing through a real relay on a local port.
#[cfg(test)]
mod end_to_end {
    use super::*;
    use crate::models::ExpenseSplit;
    use crate::storage::Store;
    use std::path::PathBuf;

    const PASSWORD: &str = "correct horse battery";

    struct Device {
        state: AppState,
        dir: PathBuf,
    }

    impl Device {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("ezcount-e2e-{}", uuid::Uuid::new_v4()));
            let (store, _) =
                Store::open(&dir.join("db.sqlite3"), &dir.join("ezcount_data.json")).unwrap();
            Self {
                state: AppState::new(store, Vec::new()).unwrap(),
                dir,
            }
        }

        async fn signed_up(url: &str, username: &str) -> Self {
            let device = Self::new();
            sign_up(&device.state, url, username, PASSWORD)
                .await
                .unwrap();
            device
        }

        async fn logged_in(url: &str, username: &str) -> Self {
            let device = Self::new();
            log_in(&device.state, url, username, PASSWORD)
                .await
                .unwrap();
            reconcile(&device.state).await.unwrap();
            device
        }

        fn group(&self, id: &str) -> Group {
            self.state.store().group(id).unwrap()
        }

        fn group_ids(&self) -> Vec<String> {
            self.state.store().group_ids()
        }

        fn identity(&self, group_id: &str) -> Option<String> {
            let store = self.state.store();
            account::identities(store.account_doc().unwrap())
                .unwrap()
                .remove(group_id)
        }

        fn create(&self, name: &str, people: &[&str]) -> Group {
            let people: Vec<String> = people.iter().map(|p| p.to_string()).collect();
            create_group(&self.state, name, "EUR", &people).unwrap()
        }

        fn invite(&self, id: &str) -> String {
            self.state.sync_info(id).unwrap().invite_code.unwrap()
        }

        fn edit(&self, id: &str, change: impl FnOnce(&LoroDoc) -> Res<()>) {
            self.state.mutate(id, change).unwrap();
        }

        /// One full round, like the background loop.
        async fn sync(&self) {
            sync_account(&self.state).await.unwrap();
            reconcile(&self.state).await.unwrap();
            let ids = self.state.store().synced_ids();
            let mut dropped = false;
            for id in ids {
                sync_group(&self.state, &id).await.unwrap();
                dropped |= drop_deleted(&self.state, &id);
            }
            // As the next pass of `sync_all` would: the account no longer lists them.
            if dropped {
                sync_account(&self.state).await.unwrap();
            }
        }
    }

    impl Drop for Device {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    async fn start_relay() -> (String, PathBuf) {
        let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let relay = ezcount_sync_server::Relay::open(&dir.join("relay.sqlite3")).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(ezcount_sync_server::serve(listener, relay));
        (url, dir)
    }

    /// A stand-in for the rate service: it knows USD to EUR, on any day but in 2031, and
    /// counts what it is asked.
    fn start_rate_service() -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::io::{Read, Write};
        use std::sync::atomic::Ordering;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/rate", listener.local_addr().unwrap());
        let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = std::sync::Arc::clone(&asked);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut request = [0u8; 2048];
                let n = stream.read(&mut request).unwrap_or(0);
                let request = String::from_utf8_lossy(&request[..n]).to_string();
                let path = request.split_whitespace().nth(1).unwrap_or_default();
                count.fetch_add(1, Ordering::SeqCst);
                let (status, body) = if !path.starts_with("/rate/USD/EUR") {
                    ("422 Unprocessable", r#"{"status":422}"#.to_string())
                } else if path.contains("date=2031") {
                    ("404 Not Found", r#"{"status":404}"#.to_string())
                } else {
                    let date = path.split("date=").nth(1).unwrap_or("2026-10-03");
                    (
                        "200 OK",
                        format!(r#"{{"date":"{date}","base":"USD","quote":"EUR","rate":0.85856}}"#),
                    )
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\n\
                     content-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (url, asked)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_relay_suggests_exchange_rates() {
        let (rates_url, asked) = start_rate_service();
        let asked = || asked.load(std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let relay = ezcount_sync_server::Relay::open_with(
            &dir.join("relay.sqlite3"),
            ezcount_sync_server::Settings {
                rates_url: Some(rates_url),
                ..Default::default()
            },
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(ezcount_sync_server::serve(listener, relay));

        let device = Device::signed_up(&url, "alice").await;
        let rate = |from: &'static str, date: Option<&'static str>| {
            let state = &device.state;
            async move { suggested_rate(state, from, "EUR", date).await.unwrap() }
        };
        let found = Some("0.85856".to_string());
        assert_eq!(rate("USD", Some("2026-08-29")).await, found);
        // The relay remembers the answer.
        assert_eq!(rate("usd", Some("2026-08-29")).await, found);
        assert_eq!(asked(), 1);
        // A day without a rate gets the latest one.
        assert_eq!(rate("USD", Some("2031-01-01")).await, found);
        assert_eq!(asked(), 3);
        assert_eq!(rate("USD", None).await, found);
        // A currency the service doesn't know, and something that isn't one.
        assert_eq!(rate("XXX", Some("2026-08-29")).await, None);
        assert_eq!(rate("../x", None).await, None);

        // A relay without a rate service, like one from before them, suggests nothing.
        let (plain, plain_dir) = start_relay().await;
        let other = Device::signed_up(&plain, "bob").await;
        assert_eq!(
            suggested_rate(&other.state, "USD", "EUR", None)
                .await
                .unwrap(),
            None
        );
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(plain_dir);
    }

    /// A relay that can be swapped for an empty one on the same address, as if its database
    /// had been lost.
    struct ReplaceableRelay {
        addr: std::net::SocketAddr,
        url: String,
        dir: PathBuf,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl ReplaceableRelay {
        async fn start() -> Self {
            let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            Self::spawn(dir, "127.0.0.1:0".parse().unwrap(), "first").await
        }

        async fn spawn(dir: PathBuf, addr: std::net::SocketAddr, db: &str) -> Self {
            let relay =
                ezcount_sync_server::Relay::open(&dir.join(format!("{db}.sqlite3"))).unwrap();
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            let addr = listener.local_addr().unwrap();
            let task = tokio::spawn(ezcount_sync_server::serve(listener, relay));
            Self {
                addr,
                url: format!("http://{addr}"),
                dir,
                task,
            }
        }

        async fn replace_with_empty(self) -> Self {
            self.task.abort();
            let _ = self.task.await;
            Self::spawn(self.dir, self.addr, "second").await
        }
    }

    fn split(id: &str) -> ExpenseSplit {
        ExpenseSplit {
            participant_id: id.to_string(),
            shares: 1,
            fixed_cents: None,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn one_account_on_two_devices() {
        let (url, relay_dir) = start_relay().await;
        let phone = Device::signed_up(&url, "Alice").await;
        let trip = phone.create("Trip", &["Alice", "Bob"]);
        let alice = trip.participants[0].id.clone();
        assert_eq!(
            phone.identity(&trip.id),
            Some(alice.clone()),
            "creator is the first person"
        );
        phone.sync().await;

        // Logging in on a laptop brings the same groups and the same identity.
        let laptop = Device::logged_in(&url, "alice").await;
        assert_eq!(laptop.group_ids(), vec![trip.id.clone()]);
        assert_eq!(laptop.group(&trip.id), phone.group(&trip.id));
        assert_eq!(laptop.identity(&trip.id), Some(alice.clone()));

        // A group created on the laptop shows up on the phone.
        let flat = laptop.create("Flat", &["Alice", "Chris"]);
        laptop.edit(&trip.id, |d| {
            doc::add_expense(
                d,
                "Taxi",
                3000,
                alice.clone(),
                vec![split(&alice)],
                None,
                None,
            )
        });
        laptop.sync().await;
        phone.sync().await;
        let mut ids = phone.group_ids();
        ids.sort();
        let mut expected = vec![trip.id.clone(), flat.id.clone()];
        expected.sort();
        assert_eq!(ids, expected);
        assert_eq!(phone.group(&trip.id).expenses.len(), 1);

        // Leaving on the phone removes the group from the laptop too.
        leave_group(&phone.state, &flat.id).await.unwrap();
        phone.sync().await;
        laptop.sync().await;
        assert_eq!(laptop.group_ids(), vec![trip.id.clone()]);

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_group_is_archived_for_one_and_deleted_for_all() {
        let (url, relay_dir) = start_relay().await;
        let alice = Device::signed_up(&url, "alice").await;
        let trip = alice.create("Trip", &["Alice", "Bob"]);
        let flat = alice.create("Flat", &["Alice", "Bob"]);
        alice.sync().await;
        let bob = Device::signed_up(&url, "bob").await;
        for group in [&trip, &flat] {
            let joined = join_group(&bob.state, &alice.invite(&group.id))
                .await
                .unwrap();
            set_identity(&bob.state, &group.id, &joined.participants[1].id).unwrap();
        }
        let archived = |d: &Device| d.state.require_account_info().unwrap().archived;

        // Archiving is the user's own: their other devices see it, the other members don't.
        set_group_archived(&alice.state, &trip.id, true).unwrap();
        alice.sync().await;
        let laptop = Device::logged_in(&url, "alice").await;
        assert_eq!(archived(&laptop), vec![trip.id.clone()]);
        bob.sync().await;
        assert!(archived(&bob).is_empty());
        set_group_archived(&laptop.state, &trip.id, false).unwrap();
        laptop.sync().await;
        alice.sync().await;
        assert!(archived(&alice).is_empty());

        // Nobody owes anything in Flat: deleting it removes it for everyone.
        let flat_invite = alice.invite(&flat.id);
        assert_eq!(delete_group(&alice.state, &flat.id).unwrap(), None);
        assert!(!alice.state.store().groups().iter().any(|g| g.id == flat.id));
        alice.sync().await;
        assert_eq!(alice.group_ids(), vec![trip.id.clone()]);
        bob.sync().await;
        assert_eq!(bob.group_ids(), vec![trip.id.clone()]);
        laptop.sync().await;
        assert_eq!(laptop.group_ids(), vec![trip.id.clone()]);
        let carol = Device::signed_up(&url, "carol").await;
        assert_eq!(
            join_group(&carol.state, &flat_invite).await.unwrap_err(),
            "This group was deleted"
        );

        // Bob owes Alice in Trip: it takes both of them.
        let (a, b) = (&trip.participants[0].id, &trip.participants[1].id);
        alice.edit(&trip.id, |d| {
            doc::add_expense(
                d,
                "Taxi",
                3000,
                a.clone(),
                vec![split(a), split(b)],
                None,
                None,
            )
        });
        let waiting = delete_group(&alice.state, &trip.id).unwrap().unwrap();
        assert_eq!(waiting.deletion_votes, vec![a.clone()]);
        alice.sync().await;
        bob.sync().await;
        assert_eq!(bob.group(&trip.id).deletion_votes, vec![a.clone()]);

        // Bob refuses, then Alice asks again and he agrees.
        bob.edit(&trip.id, doc::refuse_deletion);
        bob.sync().await;
        alice.sync().await;
        assert!(alice.group(&trip.id).deletion_votes.is_empty());
        delete_group(&alice.state, &trip.id).unwrap();
        alice.sync().await;
        bob.sync().await;
        assert_eq!(delete_group(&bob.state, &trip.id).unwrap(), None);
        bob.sync().await;
        assert!(bob.group_ids().is_empty());
        alice.sync().await;
        assert!(alice.group_ids().is_empty());

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_profile_shows_in_every_group() {
        let (url, relay_dir) = start_relay().await;
        let picture = "data:image/webp;base64,UklGRg==";
        let alice = Device::signed_up(&url, "alice").await;
        let trip = alice.create("Trip", &["Alice", "Bob"]);
        let flat = alice.create("Flat", &["Al", "Chris"]);
        let me = |group: &Group| group.participants[0].clone();

        update_profile(&alice.state, " Alice M. ", Some(picture)).unwrap();
        for id in [&trip.id, &flat.id] {
            let group = alice.group(id);
            assert_eq!(me(&group).name, "Alice M.");
            assert_eq!(me(&group).avatar.as_deref(), Some(picture));
            assert_eq!(group.participants[1].avatar, None);
        }
        let info = alice.state.require_account_info().unwrap();
        assert_eq!(info.display_name.as_deref(), Some("Alice M."));
        assert_eq!(info.avatar.as_deref(), Some(picture));

        // A new group gets the picture, and the other members see it.
        let dinner = alice.create("Dinner", &["Alice", "Bob"]);
        assert_eq!(me(&dinner).avatar.as_deref(), Some(picture));
        alice.sync().await;
        let bob = Device::signed_up(&url, "bob").await;
        let joined = join_group(&bob.state, &alice.invite(&dinner.id))
            .await
            .unwrap();
        assert_eq!(me(&joined).avatar.as_deref(), Some(picture));

        // The profile is on the account's other devices.
        let laptop = Device::logged_in(&url, "alice").await;
        let info = laptop.state.require_account_info().unwrap();
        assert_eq!(info.display_name.as_deref(), Some("Alice M."));

        // Saying to be someone else moves the picture and the name there.
        let other = dinner.participants[1].id.clone();
        set_identity(&alice.state, &dinner.id, &other).unwrap();
        let group = alice.group(&dinner.id);
        assert_eq!(me(&group).avatar, None);
        assert_eq!(group.participants[1].name, "Alice M.");
        assert_eq!(group.participants[1].avatar.as_deref(), Some(picture));

        // Without a name or a picture, groups keep their names and lose the picture.
        update_profile(&alice.state, "", None).unwrap();
        let group = alice.group(&trip.id);
        assert_eq!(me(&group).name, "Alice M.");
        assert_eq!(me(&group).avatar, None);
        assert!(update_profile(&alice.state, "Alice", Some("not a picture")).is_err());

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn identity_chosen_on_one_device_applies_everywhere() {
        let (url, relay_dir) = start_relay().await;
        let alice = Device::signed_up(&url, "alice").await;
        let group = alice.create("Dinner", &["Alice", "Bob"]);
        alice.sync().await;

        let bob_phone = Device::signed_up(&url, "bob").await;
        let joined = join_group(&bob_phone.state, &alice.invite(&group.id))
            .await
            .unwrap();
        assert_eq!(
            bob_phone.identity(&group.id),
            None,
            "joiners pick who they are"
        );
        let bob = joined.participants[1].id.clone();
        set_identity(&bob_phone.state, &group.id, &bob).unwrap();
        assert!(set_identity(&bob_phone.state, &group.id, "nobody").is_err());
        bob_phone.sync().await;

        let bob_laptop = Device::logged_in(&url, "bob").await;
        assert_eq!(bob_laptop.identity(&group.id), Some(bob));

        // Someone new adds themselves instead of picking an existing name.
        let carol = Device::signed_up(&url, "carol").await;
        join_group(&carol.state, &alice.invite(&group.id))
            .await
            .unwrap();
        let updated = add_self(&carol.state, &group.id, "Carol").unwrap();
        let me = carol.identity(&group.id).unwrap();
        assert_eq!(
            updated
                .participants
                .iter()
                .find(|p| p.id == me)
                .unwrap()
                .name,
            "Carol"
        );

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn devices_upload_again_when_the_relay_loses_its_data() {
        let relay = ReplaceableRelay::start().await;
        let alice = Device::signed_up(&relay.url, "alice").await;
        let group = alice.create("Trip", &["Alice", "Bob"]);
        let (gid, a, b) = (
            group.id.clone(),
            group.participants[0].id.clone(),
            group.participants[1].id.clone(),
        );
        alice.sync().await;
        let bob = Device::signed_up(&relay.url, "bob").await;
        join_group(&bob.state, &alice.invite(&gid)).await.unwrap();
        bob.sync().await;

        let relay = relay.replace_with_empty().await;

        // Bob syncs first: the relay doesn't know the group any more, so he uploads all of it.
        bob.edit(&gid, |d| {
            doc::add_expense(
                d,
                "Museum",
                3000,
                b.clone(),
                vec![split(&a), split(&b)],
                None,
                None,
            )
        });
        bob.sync().await;
        // Alice's position refers to the old database. The new relay id tells her to start
        // over: she uploads everything too, and reads Bob's upload from the beginning.
        alice.edit(&gid, |d| {
            doc::add_expense(
                d,
                "Dinner",
                6000,
                a.clone(),
                vec![split(&a), split(&b)],
                None,
                None,
            )
        });
        alice.sync().await;
        bob.sync().await;

        let (ga, gb) = (alice.group(&gid), bob.group(&gid));
        assert_eq!(ga, gb, "both devices converge again");
        let mut titles: Vec<&str> = ga.expenses.iter().map(|e| e.title.as_str()).collect();
        titles.sort();
        assert_eq!(titles, vec!["Dinner", "Museum"]);

        // The accounts' documents were uploaded again as well, so their other devices can sync.
        let db = rusqlite::Connection::open(relay.dir.join("second.sqlite3")).unwrap();
        let alice_account = alice.state.store().session().unwrap().account_id.clone();
        let known: bool = db
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM groups WHERE id = ?1)",
                [&alice_account],
                |r| r.get(0),
            )
            .unwrap();
        assert!(known);
        // Another round moves nothing.
        assert!(!sync_group(&alice.state, &gid).await.unwrap());

        relay.task.abort();
        let _ = std::fs::remove_dir_all(&relay.dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_relay_serves_the_join_page_and_app_links() {
        let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let http = reqwest::Client::new();
        let mut urls = Vec::new();
        for android_app in [
            None,
            Some(ezcount_sync_server::AndroidApp {
                package: "com.example.app".into(),
                cert_sha256: vec!["AB:CD".into()],
            }),
        ] {
            let db = dir.join(format!("{}.sqlite3", urls.len()));
            let settings = ezcount_sync_server::Settings {
                android_app,
                ..Default::default()
            };
            let relay = ezcount_sync_server::Relay::open_with(&db, settings).unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            urls.push(format!("http://{}", listener.local_addr().unwrap()));
            tokio::spawn(ezcount_sync_server::serve(listener, relay));
        }

        // The page an invite link opens builds the app link from the fragment.
        let page = http.get(format!("{}/join", urls[0])).send().await.unwrap();
        assert_eq!(page.status(), 200);
        assert!(page.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("default-src 'none'"));
        let html = page.text().await.unwrap();
        assert!(html.contains(r#"new URL("ezcount://join")"#));
        // No web version here: nothing at the root, and the page doesn't offer one.
        assert!(!html.contains("<body data-web>"));
        assert_eq!(http.get(&urls[0]).send().await.unwrap().status(), 404);

        let links = format!("{}/.well-known/assetlinks.json", urls[0]);
        assert_eq!(http.get(links).send().await.unwrap().status(), 404);
        let links = format!("{}/.well-known/assetlinks.json", urls[1]);
        let links: serde_json::Value = http.get(links).send().await.unwrap().json().await.unwrap();
        assert_eq!(links[0]["target"]["package_name"], "com.example.app");
        assert_eq!(links[0]["target"]["sha256_cert_fingerprints"][0], "AB:CD");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_relay_serves_the_web_version() {
        let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
        let web = dir.join("web");
        std::fs::create_dir_all(web.join("assets")).unwrap();
        std::fs::write(
            web.join("index.html"),
            "<!doctype html><title>ezcount</title>",
        )
        .unwrap();
        std::fs::write(web.join("assets").join("index-abc.js"), "start()").unwrap();
        std::fs::write(dir.join("secret.txt"), "not for the web").unwrap();
        let settings = ezcount_sync_server::Settings {
            web_dir: Some(web),
            ..Default::default()
        };
        let relay =
            ezcount_sync_server::Relay::open_with(&dir.join("db.sqlite3"), settings).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(ezcount_sync_server::serve(listener, relay));
        let http = reqwest::Client::new();
        let header =
            |r: &reqwest::Response, name: &str| r.headers()[name].to_str().unwrap().to_string();

        let page = http.get(format!("{url}/")).send().await.unwrap();
        assert_eq!(page.status(), 200);
        assert!(header(&page, "content-type").starts_with("text/html"));
        let csp = header(&page, "content-security-policy");
        assert!(
            csp.contains("script-src 'self' 'wasm-unsafe-eval';"),
            "{csp}"
        );
        assert!(csp.contains("connect-src 'self';"), "{csp}");
        assert_eq!(header(&page, "cache-control"), "no-cache");
        assert_eq!(header(&page, "x-content-type-options"), "nosniff");

        let asset = http
            .get(format!("{url}/assets/index-abc.js"))
            .send()
            .await
            .unwrap();
        assert_eq!(asset.status(), 200);
        assert!(header(&asset, "cache-control").contains("immutable"));
        assert_eq!(header(&asset, "content-security-policy"), csp);
        assert_eq!(asset.text().await.unwrap(), "start()");

        // Nothing outside the web folder, and the API still answers.
        for path in [
            "/../secret.txt",
            "/%2e%2e/secret.txt",
            "/assets/..%2f..%2fsecret.txt",
        ] {
            let response = http.get(format!("{url}{path}")).send().await.unwrap();
            assert_ne!(response.status(), 200, "{path}");
        }
        let health = http.get(format!("{url}/health")).send().await.unwrap();
        assert_eq!(health.text().await.unwrap(), "ok");
        let join = http.get(format!("{url}/join")).send().await.unwrap();
        assert!(join.text().await.unwrap().contains("<body data-web>"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_invite_opens_one_group_only() {
        let (url, relay_dir) = start_relay().await;
        let alice = Device::signed_up(&url, "alice").await;
        let trip = alice.create("Trip", &["Alice", "Bob"]);
        let flat = alice.create("Flat", &["Alice", "Chris"]);
        alice.sync().await;

        let bob = Device::signed_up(&url, "bob").await;
        join_group(&bob.state, &alice.invite(&trip.id))
            .await
            .unwrap();
        bob.sync().await;
        assert_eq!(bob.group_ids(), vec![trip.id.clone()]);

        // Alice's later groups don't reach Bob either, on any of his devices.
        let _party = alice.create("Party", &["Alice"]);
        alice.sync().await;
        bob.sync().await;
        assert_eq!(bob.group_ids(), vec![trip.id.clone()]);
        let bob_laptop = Device::logged_in(&url, "bob").await;
        assert_eq!(bob_laptop.group_ids(), vec![trip.id.clone()]);

        // The Trip key can't be used to open the Flat: each group has its own key.
        let trip_secret = bob
            .state
            .store()
            .sync_meta(&trip.id)
            .unwrap()
            .secret
            .clone();
        let borrowed = invite_code(&url, &flat.id, &trip_secret);
        let err = join_group(&bob.state, &borrowed).await.unwrap_err();
        assert!(err.contains("rejected"), "{err}");
        assert!(!bob.state.store().contains(&flat.id));

        // Nor can Alice's account be opened with a group key.
        let alice_account = alice.state.store().session().unwrap().account_id.clone();
        let err = join_group(&bob.state, &invite_code(&url, &alice_account, &trip_secret))
            .await
            .unwrap_err();
        assert!(err.contains("rejected"), "{err}");

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    /// A relay with the given limits, taking the client's address from `x-test-client`.
    async fn start_limited_relay(limits: ezcount_sync_server::Limits) -> (String, PathBuf) {
        let dir = std::env::temp_dir().join(format!("ezcount-relay-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let settings = ezcount_sync_server::Settings {
            client_ip_header: Some("x-test-client".parse().unwrap()),
            limits,
            ..Default::default()
        };
        let relay =
            ezcount_sync_server::Relay::open_with(&dir.join("relay.sqlite3"), settings).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(ezcount_sync_server::serve(listener, relay));
        (url, dir)
    }

    /// Uploads `size` bytes to a document as `client`; returns the HTTP status.
    async fn raw_push(url: &str, client: &str, doc: &str, token: &str, size: usize) -> u16 {
        reqwest::Client::new()
            .post(updates_url(url, doc))
            .header("x-test-client", client)
            .bearer_auth(token)
            .body(vec![7u8; size])
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_relay_caps_sizes_and_upload_rates() {
        let token = "t".repeat(43);
        let limits = |f: fn(&mut ezcount_sync_server::Limits)| {
            let mut limits = ezcount_sync_server::Limits::default();
            f(&mut limits);
            limits
        };

        // Per document, then in all: 413, then 507. Existing data stays readable.
        let (url, dir) = start_limited_relay(limits(|l| {
            l.max_document_bytes = 1000;
            l.max_total_bytes = 1500;
        }))
        .await;
        assert_eq!(raw_push(&url, "10.0.0.1", "g1", &token, 600).await, 200);
        assert_eq!(raw_push(&url, "10.0.0.1", "g1", &token, 600).await, 413);
        assert_eq!(raw_push(&url, "10.0.0.1", "g2", &token, 600).await, 200);
        assert_eq!(raw_push(&url, "10.0.0.2", "g3", &token, 600).await, 507);
        let _ = std::fs::remove_dir_all(&dir);

        // Per client: new documents and uploaded bytes per hour. Other clients are unaffected.
        let (url, dir) = start_limited_relay(limits(|l| {
            l.new_documents_per_hour = 2;
            l.upload_bytes_per_hour = 1000;
        }))
        .await;
        assert_eq!(raw_push(&url, "10.0.0.1", "g1", &token, 100).await, 200);
        assert_eq!(raw_push(&url, "10.0.0.1", "g2", &token, 100).await, 200);
        assert_eq!(raw_push(&url, "10.0.0.1", "g3", &token, 100).await, 429);
        assert_eq!(raw_push(&url, "10.0.0.1", "g1", &token, 900).await, 429);
        assert_eq!(raw_push(&url, "10.0.0.2", "g3", &token, 900).await, 200);
        // IPv6 clients count per /64: another address in it shares the quota.
        assert_eq!(
            raw_push(&url, "2001:db8:1:2::1", "g4", &token, 600).await,
            200
        );
        assert_eq!(
            raw_push(&url, "2001:db8:1:2::99", "g4", &token, 600).await,
            429
        );
        assert_eq!(
            raw_push(&url, "2001:db8:1:3::1", "g4", &token, 600).await,
            200
        );
        let _ = std::fs::remove_dir_all(&dir);

        // Sign-ups per client; the app reports the limit.
        let (url, dir) = start_limited_relay(limits(|l| l.sign_ups_per_hour = 1)).await;
        let _alice = Device::signed_up(&url, "alice").await;
        let err = sign_up(&Device::new().state, &url, "bob", PASSWORD)
            .await
            .unwrap_err();
        assert!(err.contains("too many requests"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn failed_logins_lock_one_network_not_the_account() {
        let (url, dir) = start_limited_relay(ezcount_sync_server::Limits {
            login_failures_per_client: 3,
            login_failures_per_username: 5,
            ..Default::default()
        })
        .await;
        // The app sends no x-test-client header: it counts as another network.
        let _alice = Device::signed_up(&url, "alice").await;
        let wrong_login = |client: &'static str| {
            let url = url.clone();
            async move {
                reqwest::Client::new()
                    .post(format!("{url}/v1/accounts/login"))
                    .header("x-test-client", client)
                    .json(
                        &serde_json::json!({ "username": "alice", "login_token": "w".repeat(43) }),
                    )
                    .send()
                    .await
                    .unwrap()
                    .status()
                    .as_u16()
            }
        };
        for _ in 0..3 {
            assert_eq!(wrong_login("203.0.113.7").await, 401);
        }
        assert_eq!(
            wrong_login("203.0.113.7").await,
            429,
            "that network is blocked"
        );
        let phone = Device::new();
        log_in(&phone.state, &url, "alice", PASSWORD).await.unwrap();

        // Past the per-username ceiling, spread over networks, everyone waits.
        for _ in 0..2 {
            assert_eq!(wrong_login("198.51.100.1").await, 401);
        }
        let err = log_in(&Device::new().state, &url, "alice", PASSWORD)
            .await
            .unwrap_err();
        assert!(err.contains("Too many failed attempts"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    const NEW_PASSWORD: &str = "juniper walrus lantern";

    #[tokio::test(flavor = "multi_thread")]
    async fn a_forgotten_password_is_reset_with_the_recovery_key() {
        let (url, relay_dir) = start_relay().await;
        let laptop = Device::new();
        let key = sign_up(&laptop.state, &url, "alice", PASSWORD)
            .await
            .unwrap()
            .expect("the relay stores recovery keys");
        let trip = laptop.create("Trip", &["Alice", "Bob"]).id;
        laptop.sync().await;

        let phone = Device::new();
        let err = recover_account(
            &phone.state,
            &url,
            "alice",
            &new_recovery_key().unwrap(),
            NEW_PASSWORD,
        )
        .await
        .unwrap_err();
        assert_eq!(err, "Wrong username or recovery key");
        let err = recover_account(&phone.state, &url, "alice", &key, "password123")
            .await
            .unwrap_err();
        assert!(err.contains("too easy to guess"), "{err}");

        // Typed as people do: lowercase, spaces instead of dashes.
        let typed = key.to_lowercase().replace('-', " ");
        let next = recover_account(&phone.state, &url, "alice", &typed, NEW_PASSWORD)
            .await
            .unwrap();
        assert_ne!(next, key, "a recovery key works once");
        reconcile(&phone.state).await.unwrap();
        assert_eq!(phone.group_ids(), vec![trip.clone()]);

        let other = Device::new();
        let err = log_in(&other.state, &url, "alice", PASSWORD)
            .await
            .unwrap_err();
        assert_eq!(err, "Wrong username or password");
        let err = recover_account(&other.state, &url, "alice", &key, NEW_PASSWORD)
            .await
            .unwrap_err();
        assert_eq!(
            err, "Wrong username or recovery key",
            "the used key is gone"
        );
        log_in(&other.state, &url, "alice", NEW_PASSWORD)
            .await
            .unwrap();
        // The laptop never needed the password again.
        laptop.sync().await;

        let _ = std::fs::remove_dir_all(&relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn changing_the_password_keeps_other_devices_in() {
        let (url, relay_dir) = start_relay().await;
        let laptop = Device::signed_up(&url, "alice").await;
        let trip = laptop.create("Trip", &["Alice", "Bob"]);
        laptop.sync().await;
        let phone = Device::logged_in(&url, "alice").await;

        let err = change_password(&laptop.state, "not my password", NEW_PASSWORD)
            .await
            .unwrap_err();
        assert_eq!(err, "Your current password is wrong");
        let err = change_password(&laptop.state, PASSWORD, "password123")
            .await
            .unwrap_err();
        assert!(err.contains("too easy to guess"), "{err}");
        change_password(&laptop.state, PASSWORD, NEW_PASSWORD)
            .await
            .unwrap();

        let other = Device::new();
        assert!(log_in(&other.state, &url, "alice", PASSWORD).await.is_err());
        log_in(&other.state, &url, "alice", NEW_PASSWORD)
            .await
            .unwrap();

        // The phone still syncs, without the new password.
        phone.sync().await;
        let (a, b) = (
            trip.participants[0].id.clone(),
            trip.participants[1].id.clone(),
        );
        phone.edit(&trip.id, |d| {
            doc::add_expense(
                d,
                "Taxi",
                1200,
                a.clone(),
                vec![split(&a), split(&b)],
                None,
                None,
            )
        });
        phone.sync().await;
        laptop.sync().await;
        assert_eq!(laptop.group(&trip.id).expenses.len(), 1);

        let _ = std::fs::remove_dir_all(&relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_new_recovery_key_replaces_the_old_one() {
        let (url, relay_dir) = start_relay().await;
        let laptop = Device::signed_up(&url, "alice").await;
        // As for an account from before recovery keys: the relay has none for it.
        let db = rusqlite::Connection::open(relay_dir.join("relay.sqlite3")).unwrap();
        db.execute(
            "UPDATE accounts SET recovery_hash = NULL, recovery_wrapped_key = NULL",
            [],
        )
        .unwrap();

        let err = replace_recovery_key(&laptop.state, "not my password")
            .await
            .unwrap_err();
        assert_eq!(err, "Wrong password");
        let first = replace_recovery_key(&laptop.state, PASSWORD).await.unwrap();
        let second = replace_recovery_key(&laptop.state, PASSWORD).await.unwrap();

        let phone = Device::new();
        let err = recover_account(&phone.state, &url, "alice", &first, NEW_PASSWORD)
            .await
            .unwrap_err();
        assert_eq!(err, "Wrong username or recovery key", "replaced");
        recover_account(&phone.state, &url, "alice", &second, NEW_PASSWORD)
            .await
            .unwrap();

        let _ = std::fs::remove_dir_all(&relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_login_link_logs_another_device_in_once() {
        let (url, relay_dir) = start_relay().await;
        let laptop = Device::signed_up(&url, "alice").await;
        let trip = laptop.create("Trip", &["Alice", "Bob"]);
        laptop.sync().await;

        let err = create_login_link(&laptop.state, "not my password")
            .await
            .unwrap_err();
        assert_eq!(err, "Wrong password");
        let link = create_login_link(&laptop.state, PASSWORD).await.unwrap();
        assert_eq!(link.expires_in, 120);
        assert!(link.link.starts_with("ezcount://login?server=http"));

        // The relay is given the ticket, which is neither the code nor in the link.
        let (_, code) = parse_login_link(&link.link).unwrap();
        let ticket = LinkKeys::derive(&code).unwrap().ticket;
        assert!(ticket != code.expose() && !link.link.contains(&ticket));

        let phone = Device::new();
        log_in_with_link(&phone.state, &link.link).await.unwrap();
        reconcile(&phone.state).await.unwrap();
        assert_eq!(phone.group_ids(), vec![trip.id.clone()]);
        assert_eq!(phone.state.store().session().unwrap().username, "alice");
        // It is a full login: the phone's edits reach the laptop.
        phone.edit(&trip.id, |doc| {
            doc::add_participant(doc, "Carol", doc::AddedBy::Member(None)).map(|_| ())
        });
        phone.sync().await;
        laptop.sync().await;
        assert_eq!(laptop.group(&trip.id).participants.len(), 3);

        // A link works once.
        let tablet = Device::new();
        let err = log_in_with_link(&tablet.state, &link.link)
            .await
            .unwrap_err();
        assert!(err.starts_with("This code has expired"), "{err}");
        assert!(tablet.state.store().session().is_none());

        for bad in [
            "hello",
            "ezcount://join?server=x",
            "https://example.com/login?code=x",
        ] {
            let err = log_in_with_link(&tablet.state, bad).await.unwrap_err();
            assert_eq!(err, "This is not an ezcount login code", "{bad}");
        }
        let err = log_in_with_link(&phone.state, &link.link)
            .await
            .unwrap_err();
        assert_eq!(err, "This device is already logged in");

        let _ = std::fs::remove_dir_all(&relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_login_link_expires() {
        let (url, relay_dir) = start_limited_relay(ezcount_sync_server::Limits {
            link_lifetime: Duration::from_millis(50),
            ..Default::default()
        })
        .await;
        let laptop = Device::signed_up(&url, "alice").await;
        let link = create_login_link(&laptop.state, PASSWORD).await.unwrap();
        tokio::time::sleep(Duration::from_millis(120)).await;

        let phone = Device::new();
        let err = log_in_with_link(&phone.state, &link.link)
            .await
            .unwrap_err();
        assert!(err.starts_with("This code has expired"), "{err}");

        let _ = std::fs::remove_dir_all(&relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sign_up_and_log_in_errors() {
        let (url, relay_dir) = start_relay().await;
        let _alice = Device::signed_up(&url, "alice").await;

        let other = Device::new();
        let err = sign_up(&other.state, &url, "ALICE", PASSWORD)
            .await
            .unwrap_err();
        assert!(err.contains("already taken"), "{err}");
        let err = sign_up(&other.state, &url, "dave", "short")
            .await
            .unwrap_err();
        assert!(err.contains("at least"), "{err}");
        let err = sign_up(&other.state, &url, "dave", "password123")
            .await
            .unwrap_err();
        assert!(err.contains("too easy to guess"), "{err}");

        let err = log_in(&other.state, &url, "alice", "wrong password")
            .await
            .unwrap_err();
        assert_eq!(err, "Wrong username or password");
        let err = log_in(&other.state, &url, "nobody", PASSWORD)
            .await
            .unwrap_err();
        assert_eq!(
            err, "Wrong username or password",
            "unknown user looks the same"
        );
        assert!(other.state.store().session().is_none());

        // After repeated failures the relay refuses even the right password for a while.
        for _ in 0..4 {
            let _ = log_in(&other.state, &url, "alice", "wrong password").await;
        }
        let err = log_in(&other.state, &url, "alice", PASSWORD)
            .await
            .unwrap_err();
        assert!(err.contains("Too many"), "{err}");

        let offline = Device::new();
        let err = sign_up(&offline.state, "http://127.0.0.1:9", "erin", PASSWORD)
            .await
            .unwrap_err();
        assert!(err.contains("reach"), "{err}");
        assert!(offline.state.store().session().is_none());

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn local_groups_join_the_account_and_log_out_clears_the_device() {
        let (url, relay_dir) = start_relay().await;
        let device = Device::new();
        let doc = doc::new_group_doc("Before accounts", "EUR", &["Ann".into()]).unwrap();
        let old = device.state.store().insert(doc, None).unwrap();

        sign_up(&device.state, &url, "ann", PASSWORD).await.unwrap();
        assert!(device.state.sync_info(&old.id).unwrap().enabled);
        device.sync().await;

        log_out(&device.state, false).await.unwrap();
        assert!(device.state.store().session().is_none());
        assert!(device.group_ids().is_empty());

        log_in(&device.state, &url, "ann", PASSWORD).await.unwrap();
        reconcile(&device.state).await.unwrap();
        assert_eq!(device.group(&old.id).name, "Before accounts");

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn log_out_keeps_unuploaded_changes_unless_forced() {
        let device = Device::new();
        let (url, relay_dir) = start_relay().await;
        sign_up(&device.state, &url, "ann", PASSWORD).await.unwrap();
        // Point the account at a dead relay so nothing can be uploaded.
        device
            .state
            .store()
            .insert(
                doc::new_group_doc("Offline", "EUR", &["Ann".into()]).unwrap(),
                Some(SyncMeta::new(
                    "http://127.0.0.1:9".into(),
                    new_secret().unwrap(),
                )),
            )
            .unwrap();

        let err = log_out(&device.state, false).await.unwrap_err();
        assert!(err.contains("not uploaded"), "{err}");
        assert!(device.state.store().session().is_some());
        log_out(&device.state, true).await.unwrap();
        assert!(device.state.store().session().is_none());

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn members_share_and_converge() {
        let (url, relay_dir) = start_relay().await;
        let (a, b) = (
            Device::signed_up(&url, "alice").await,
            Device::signed_up(&url, "bob").await,
        );

        // A creates a group with one expense and uploads it.
        let group = a.create("Trip", &["Alice", "Bob"]);
        let (gid, alice, bob) = (
            group.id.clone(),
            group.participants[0].id.clone(),
            group.participants[1].id.clone(),
        );
        a.edit(&gid, |d| {
            doc::add_expense(
                d,
                "Hotel",
                20000,
                alice.clone(),
                vec![split(&alice), split(&bob)],
                None,
                None,
            )
        });
        sync_group(&a.state, &gid).await.unwrap();

        // B joins with the invite code and sees the same group.
        let joined = join_group(&b.state, &a.invite(&gid)).await.unwrap();
        assert_eq!(joined, a.group(&gid));
        assert!(
            join_group(&b.state, &a.invite(&gid)).await.is_err(),
            "joining twice is refused"
        );

        // Both edit while "offline", then sync in any order.
        let hotel = joined.expenses[0].id.clone();
        a.edit(&gid, |d| {
            doc::add_participant(d, "Charlie", doc::AddedBy::Member(None)).map(|_| ())
        });
        a.edit(&gid, |d| {
            doc::update_expense(
                d,
                &hotel,
                "Hotel",
                24000,
                alice.clone(),
                vec![split(&alice), split(&bob)],
                None,
                None,
            )
        });
        b.edit(&gid, |d| {
            doc::update_expense(
                d,
                &hotel,
                "Hotel Roma",
                20000,
                alice.clone(),
                vec![split(&alice), split(&bob)],
                None,
                None,
            )
        });
        b.edit(&gid, |d| {
            doc::add_expense(
                d,
                "Pizza",
                3000,
                bob.clone(),
                vec![split(&alice), split(&bob)],
                None,
                None,
            )
        });
        b.edit(&gid, |d| doc::remove_participant(d, &bob, None));

        assert!(
            !sync_group(&a.state, &gid).await.unwrap(),
            "nothing new from B yet"
        );
        assert!(
            sync_group(&b.state, &gid).await.unwrap(),
            "B receives A's edits"
        );
        assert!(
            sync_group(&a.state, &gid).await.unwrap(),
            "A receives B's edits"
        );

        let (ga, gb) = (a.group(&gid), b.group(&gid));
        assert_eq!(ga, gb, "devices converge");
        assert_eq!(ga.participants.len(), 3);
        assert!(ga.participants.iter().any(|p| p.id == bob && p.removed));
        let hotel = ga.expenses.iter().find(|e| e.id == hotel).unwrap();
        assert_eq!(
            (hotel.title.as_str(), hotel.amount_cents),
            ("Hotel Roma", 24000)
        );
        assert_eq!(ga.expenses.len(), 2);

        // Bob was removed but still owes money, so he still appears in the balances.
        let balances = crate::engine::calculate_balances(&ga);
        assert!(balances
            .iter()
            .any(|b| b.participant_id == bob && b.removed));
        assert_eq!(balances.iter().map(|b| b.net_cents).sum::<i64>(), 0);

        // Another sync round moves nothing.
        assert!(!sync_group(&a.state, &gid).await.unwrap());
        assert!(!sync_group(&b.state, &gid).await.unwrap());

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn relay_sees_neither_data_nor_secrets() {
        use sha2::{Digest, Sha256};

        let (url, relay_dir) = start_relay().await;
        let a = Device::signed_up(&url, "alice").await;
        let marker = "Confidential-Hotel-Name-7391";
        let gid = a.create(marker, &["Zoe-Marker"]).id;
        // Sanity check: an unencrypted Loro update does contain the text.
        let plain = a
            .state
            .store()
            .doc(&gid)
            .unwrap()
            .export(ExportMode::Snapshot)
            .unwrap();
        assert!(plain.windows(marker.len()).any(|w| w == marker.as_bytes()));
        a.sync().await;
        let secret = a.state.store().sync_meta(&gid).unwrap().secret.clone();

        let relay = rusqlite::Connection::open(relay_dir.join("relay.sqlite3")).unwrap();
        let blobs: Vec<Vec<u8>> = relay
            .prepare("SELECT data FROM updates UNION ALL SELECT wrapped_key FROM accounts")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(blobs.len() >= 3, "group, account and wrapped key");
        // Neither the data, nor the group key kept in the account, nor the password.
        for blob in &blobs {
            for needle in [
                marker.as_bytes(),
                b"Zoe-Marker",
                secret.expose().as_bytes(),
                PASSWORD.as_bytes(),
            ] {
                assert!(
                    !blob.windows(needle.len()).any(|w| w == needle),
                    "plaintext leaked"
                );
            }
        }

        // The relay stores the hash of the derived token, never of the secret itself.
        let stored: Vec<u8> = relay
            .query_row("SELECT key_hash FROM groups WHERE id = ?1", [&gid], |r| {
                r.get(0)
            })
            .unwrap();
        let token = GroupKeys::derive(&secret).unwrap().auth_token;
        assert_eq!(stored, Sha256::digest(token.as_bytes()).to_vec());
        assert_ne!(stored, Sha256::digest(secret.expose().as_bytes()).to_vec());
        let login_hash: Vec<u8> = relay
            .query_row("SELECT login_hash FROM accounts", [], |r| r.get(0))
            .unwrap();
        assert_ne!(login_hash, Sha256::digest(PASSWORD.as_bytes()).to_vec());

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tampered_update_is_refused() {
        let (url, relay_dir) = start_relay().await;
        let (a, b) = (
            Device::signed_up(&url, "alice").await,
            Device::signed_up(&url, "bob").await,
        );
        let gid = a.create("Flat", &["Ann"]).id;
        sync_group(&a.state, &gid).await.unwrap();

        // A malicious or faulty relay flips one byte of the stored update.
        let relay = rusqlite::Connection::open(relay_dir.join("relay.sqlite3")).unwrap();
        let mut blob: Vec<u8> = relay
            .query_row(
                "SELECT data FROM updates WHERE group_id = ?1",
                [&gid],
                |r| r.get(0),
            )
            .unwrap();
        *blob.last_mut().unwrap() ^= 1;
        relay
            .execute(
                "UPDATE updates SET data = ?1 WHERE group_id = ?2",
                rusqlite::params![blob, gid],
            )
            .unwrap();

        let err = join_group(&b.state, &a.invite(&gid)).await.unwrap_err();
        assert!(err.contains("decrypted"), "{err}");
        assert!(!b.state.store().contains(&gid));

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn wrong_key_is_rejected() {
        let (url, relay_dir) = start_relay().await;
        let (a, b) = (
            Device::signed_up(&url, "alice").await,
            Device::signed_up(&url, "bob").await,
        );
        let gid = a.create("Flat", &["Ann"]).id;
        sync_group(&a.state, &gid).await.unwrap();

        let forged = invite_code(&url, &gid, &new_secret().unwrap());
        let err = join_group(&b.state, &forged).await.unwrap_err();
        assert!(err.contains("rejected"), "{err}");

        let missing = invite_code(&url, "no-such-group", &new_secret().unwrap());
        assert!(join_group(&b.state, &missing).await.is_err());

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    /// A relay that never stops paging: every request gets the same real update under a new
    /// sequence number, with `has_more`. Returns its URL and how many requests it answered.
    fn start_endless_relay(
        group_id: &str,
        secret: &Secret,
        update: &[u8],
    ) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};

        let keys = GroupKeys::derive(secret).unwrap();
        let sealed = STANDARD.encode(keys.seal(group_id, update).unwrap());
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = std::sync::Arc::new(AtomicUsize::new(0));
        let answered = requests.clone();
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                // Read the whole request, body included, before answering.
                let mut request = Vec::new();
                let mut chunk = [0; 4096];
                while let Ok(n @ 1..) = stream.read(&mut chunk) {
                    request.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&request).to_lowercase();
                    if let Some(end) = text.find("\r\n\r\n") {
                        let body_len = text
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length:"))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        if request.len() >= end + 4 + body_len {
                            break;
                        }
                    }
                }
                let seq = answered.fetch_add(1, Ordering::SeqCst) + 1;
                let body = format!(
                    r#"{{"updates":[{{"seq":{seq},"data":"{sealed}"}}],"has_more":true,"relay_id":"endless"}}"#
                );
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                     content-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (url, requests)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_relay_that_never_stops_paging_cannot_hold_up_sync() {
        let device = Device::new();
        let doc = doc::new_group_doc("Trip", "EUR", &["Alice".into()]).unwrap();
        let update = doc.export(ExportMode::Snapshot).unwrap();
        let secret = new_secret().unwrap();
        let group = device.state.store().insert(doc, None).unwrap();
        let (url, requests) = start_endless_relay(&group.id, &secret, &update);
        let answered = || requests.load(std::sync::atomic::Ordering::SeqCst);

        let (copy, meta) = download(&device.state.http, &url, &secret, &group.id)
            .await
            .unwrap();
        assert_eq!(answered(), MAX_PAGES_PER_SYNC);
        assert_eq!(meta.cursor, MAX_PAGES_PER_SYNC as i64);
        assert_eq!(doc::read_group(&copy).unwrap(), group);

        device
            .state
            .store()
            .set_sync(&group.id, SyncMeta::new(url, secret))
            .unwrap();
        let before = answered();
        sync_group(&device.state, &group.id).await.unwrap();
        let used = answered() - before;
        // A first check and possibly an upload, then at most one budget of pages.
        assert!(used <= MAX_PAGES_PER_SYNC + 2, "{used} requests");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_group_left_elsewhere_stays_until_its_edits_are_uploaded() {
        let (url, relay_dir) = start_relay().await;
        let phone = Device::signed_up(&url, "alice").await;
        let trip = phone.create("Trip", &["Alice"]);
        let alice = trip.participants[0].id.clone();
        let invite = phone.invite(&trip.id);
        phone.sync().await;
        let laptop = Device::logged_in(&url, "alice").await;

        // The phone can't reach the relay for this group when the laptop leaves it.
        let mut meta = phone.state.store().sync_meta(&trip.id).unwrap().clone();
        let relay_url = std::mem::replace(&mut meta.server_url, "http://127.0.0.1:9".into());
        phone
            .state
            .store()
            .set_sync(&trip.id, meta.clone())
            .unwrap();
        phone.edit(&trip.id, |d| {
            doc::add_expense(
                d,
                "Taxi",
                3000,
                alice.clone(),
                vec![split(&alice)],
                None,
                None,
            )
        });
        leave_group(&laptop.state, &trip.id).await.unwrap();
        laptop.sync().await;

        sync_account(&phone.state).await.unwrap();
        reconcile(&phone.state).await.unwrap();
        assert!(
            phone.state.store().contains(&trip.id),
            "the unuploaded edit is kept"
        );

        // Back online: the edit goes up first, then the group goes.
        meta.server_url = relay_url;
        phone.state.store().set_sync(&trip.id, meta).unwrap();
        reconcile(&phone.state).await.unwrap();
        assert!(!phone.state.store().contains(&trip.id));

        let bob = Device::signed_up(&url, "bob").await;
        let joined = join_group(&bob.state, &invite).await.unwrap();
        assert_eq!(
            joined.expenses.len(),
            1,
            "the edit reached the other members"
        );

        let _ = std::fs::remove_dir_all(relay_dir);
    }
}
