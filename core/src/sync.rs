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
use crate::models::{Group, LoginLink, PasswordStrength, Received};
use crate::notices::{self, Notice};
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

mod links;
pub use links::*;

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
        // Best effort: the error reported is the insert's, and a group left in the account
        // is only fetched again.
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

/// Longest message the feedback form sends, in characters.
pub const MAX_FEEDBACK_CHARS: usize = 2000;

/// Sends an idea or a problem to whoever runs the account's relay, with how to answer
/// (`contact`) when the user wants an answer, and which app it comes from.
pub async fn send_feedback(
    state: &AppState,
    message: &str,
    contact: Option<&str>,
    app: Option<&str>,
) -> Res<()> {
    let session = require_session(state)?;
    let message = message.trim();
    if message.is_empty() {
        return Err("Write a message first".to_string());
    }
    if message.chars().count() > MAX_FEEDBACK_CHARS {
        return Err(format!(
            "This message is too long ({MAX_FEEDBACK_CHARS} characters at most)"
        ));
    }
    let short = |text: Option<&str>| {
        text.map(|t| t.trim().chars().take(200).collect::<String>())
            .filter(|t| !t.is_empty())
    };
    let body = serde_json::json!({
        "message": message,
        "contact": short(contact),
        "app": short(app),
    });
    let response = state
        .http
        .post(format!("{}/v1/feedback", session.server_url))
        .json(&body)
        .send()
        .await
        .map_err(request_err)?;
    match response.status().as_u16() {
        200..=299 => Ok(()),
        // A relay from before the form.
        404 | 405 => Err("This sync server doesn't take messages yet".to_string()),
        429 => Err("Too many messages were sent from your network: try again later".to_string()),
        status => Err(format!("The sync server answered {status} to the message")),
    }
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
    // Out of the account first: left in it, the group would be downloaded again. A failure
    // leaves both as they are for the next pass.
    if store.session().is_some()
        && store
            .update_account(|d| account::remove_group(d, group_id))
            .is_err()
    {
        return false;
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
            doc::set_participant_avatar(d, &previous, None)?;
            doc::set_participant_iban(d, &previous, None)
        });
    }
    let profile = account::profile(store.account_doc()?);
    let _ = show_profile(&mut store, group_id, participant_id, &profile);
    drop(store);
    state.sync_wakeup.notify_one();
    Ok(())
}

/// Gives a member of a group the profile's name, picture and bank account. Without a name in
/// the profile, the member keeps theirs.
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
            doc::set_participant_avatar(d, participant_id, profile.avatar.as_deref())?;
            doc::set_participant_iban(d, participant_id, profile.iban.as_deref())
        })
        .map(|_| ())
}

/// Sets the name, picture and bank account the user shows, and gives them to the member they
/// are in each of their groups, for the other members to see.
pub fn update_profile(
    state: &AppState,
    name: &str,
    avatar: Option<&str>,
    iban: Option<&str>,
) -> Res<()> {
    require_session(state)?;
    let mut store = state.store();
    store.update_account(|d| account::set_profile(d, name, avatar, iban))?;
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
pub async fn sync_all(state: &AppState, report: impl FnMut(SyncEvent)) {
    sync_all_noticing(state, report).await;
}

/// `sync_all`, which also returns what the other members did in the groups that received
/// changes, for the notifications a phone shows.
pub async fn sync_all_noticing(state: &AppState, mut report: impl FnMut(SyncEvent)) -> Vec<Notice> {
    let mut notices = Vec::new();
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
        let before = state.store().group(&group_id).ok();
        let changed = matches!(sync_group(state, &group_id).await, Ok(true));
        // Read apart: the store stays locked for as long as what it returned is matched on.
        let after = state.store().group(&group_id);
        if let (true, Some(before)) = (changed, before) {
            if let Ok(after) = after {
                let me = state
                    .account_info()
                    .ok()
                    .flatten()
                    .and_then(|account| account.identities.get(&group_id).cloned());
                notices.extend(notices::news(&before, &after, me.as_deref()));
            }
        }
        if drop_deleted(state, &group_id) {
            state.sync_wakeup.notify_one();
            report(SyncEvent::Account);
            continue;
        }
        report(SyncEvent::Group { group_id, changed });
    }
    notices
}

#[cfg(test)]
mod tests;

/// Devices syncing through a real relay on a local port.
#[cfg(test)]
mod end_to_end;
