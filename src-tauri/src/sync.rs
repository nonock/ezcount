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

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use loro::{ExportMode, LoroDoc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

use crate::account;
use crate::crypto::{GroupKeys, PasswordKeys};
use crate::doc;
use crate::models::Group;
use crate::storage::{import_remote_update, Session, SyncMeta};
use crate::AppState;

type Res<T> = Result<T, String>;

/// How often shared groups are synced in the background, besides right after local edits.
const POLL_INTERVAL: Duration = Duration::from_secs(20);
/// Short pause after a wake-up so a burst of edits goes out as one push.
const DEBOUNCE: Duration = Duration::from_millis(500);
const MIN_PASSWORD_LEN: usize = 8;

/// Frontend event emitted after each background sync attempt of a group.
pub const SYNC_EVENT: &str = "sync-updated";
/// Frontend event emitted when the account's groups or identities changed on another device.
pub const ACCOUNT_EVENT: &str = "account-updated";

#[derive(Clone, Serialize)]
struct SyncEvent {
    group_id: String,
    changed: bool,
}

pub fn http_client() -> Res<reqwest::Client> {
    let builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30));
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
    pub secret: String,
}

/// Invite format version. Version 2 means end-to-end encrypted updates.
const INVITE_VERSION: &str = "2";

pub fn invite_code(server_url: &str, group_id: &str, secret: &str) -> String {
    let mut url = Url::parse("ezcount://join").expect("static URL is valid");
    url.query_pairs_mut()
        .append_pair("server", server_url)
        .append_pair("group", group_id)
        .append_pair("key", secret)
        .append_pair("v", INVITE_VERSION);
    url.to_string()
}

pub fn parse_invite(code: &str) -> Res<Invite> {
    let invalid = || "This is not a valid ezcount invite code".to_string();
    let url = Url::parse(code.trim()).map_err(|_| invalid())?;
    if url.scheme() != "ezcount" || url.host_str() != Some("join") {
        return Err(invalid());
    }
    let param = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
            .filter(|v| !v.is_empty())
            .ok_or_else(invalid)
    };
    if param("v").ok().as_deref() != Some(INVITE_VERSION) {
        return Err(
            "This invite code is for a different version of ezcount. Ask for a new code."
                .to_string(),
        );
    }
    let secret = param("key")?;
    GroupKeys::derive(&secret).map_err(|_| invalid())?;
    Ok(Invite {
        server_url: normalize_server_url(&param("server")?)?,
        group_id: param("group")?,
        secret,
    })
}

pub fn normalize_server_url(raw: &str) -> Res<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    let url = Url::parse(trimmed).map_err(|_| format!("'{trimmed}' is not a valid server URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("The server URL must start with http:// or https://".to_string());
    }
    Ok(trimmed.to_string())
}

pub fn new_secret() -> Res<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("Could not generate a key: {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
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

fn check_password(password: &str) -> Res<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!(
            "Use a password of at least {MIN_PASSWORD_LEN} characters"
        ));
    }
    Ok(())
}

/// Argon2 takes a noticeable fraction of a second, so it runs on a blocking thread.
async fn password_keys(username: &str, password: &str) -> Res<PasswordKeys> {
    let (username, password) = (username.to_string(), password.to_string());
    tauri::async_runtime::spawn_blocking(move || PasswordKeys::derive(&username, &password))
        .await
        .map_err(|e| format!("Could not derive keys from the password: {e}"))?
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

fn updates_url(server_url: &str, group_id: &str) -> String {
    format!("{server_url}/v1/groups/{group_id}/updates")
}

fn request_err(e: reqwest::Error) -> String {
    if e.is_connect() || e.is_timeout() {
        "Could not reach the sync server".to_string()
    } else {
        format!("Sync request failed: {e}")
    }
}

async fn check_status(response: reqwest::Response) -> Res<reqwest::Response> {
    match response.status().as_u16() {
        200..=299 => Ok(response),
        401 | 403 => Err("The sync server rejected this group's key".to_string()),
        404 => Err(NOT_ON_SERVER.to_string()),
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

/// Downloads a whole document. Returns it with the matching sync state.
async fn download(
    http: &reqwest::Client,
    server_url: &str,
    secret: &str,
    id: &str,
) -> Res<(LoroDoc, SyncMeta)> {
    let keys = GroupKeys::derive(secret)?;
    let doc = LoroDoc::new();
    let mut meta = SyncMeta::new(server_url.to_string(), secret.to_string());
    loop {
        let page = pull_page(http, server_url, &keys, id, meta.cursor).await?;
        meta.relay_id = page.relay_id;
        for (seq, bytes) in page.updates {
            import_remote_update(&doc, &bytes, &mut meta.server_vv)?;
            meta.cursor = seq;
        }
        if !page.has_more {
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
                changed |= state.store().import_remote(group_id, &page.updates)?;
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
        state.store().mark_pushed(group_id, &pushed)?;
    }

    loop {
        let after = state
            .store()
            .sync_meta(group_id)
            .map(|m| m.cursor)
            .ok_or_else(not_shared)?;
        let page = pull_page(&state.http, &meta.server_url, &keys, group_id, after).await?;
        state
            .store()
            .check_relay(group_id, page.relay_id.as_deref())?;
        if !page.updates.is_empty() {
            changed |= state.store().import_remote(group_id, &page.updates)?;
        }
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
        // Left on another device. Upload what this device still had first, for the others.
        let _ = sync_group(state, id).await;
        state.store().delete(id)?;
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
) -> Res<()> {
    let server_url = normalize_server_url(server_url)?;
    let username = normalize_username(username)?;
    check_password(password)?;
    ensure_logged_out(state)?;

    let keys = password_keys(&username, password).await?;
    let account_id = uuid::Uuid::new_v4().to_string();
    let account_key = new_secret()?;
    let doc_keys = GroupKeys::derive(&account_key)?;

    let response = state
        .http
        .post(format!("{server_url}/v1/accounts"))
        .json(&SignUpRequest {
            username: &username,
            login_token: &keys.login_token,
            wrapped_key: STANDARD.encode(keys.wrap_account_key(&account_id, &account_key)?),
            account_id: &account_id,
            doc_token: &doc_keys.auth_token,
        })
        .send()
        .await
        .map_err(request_err)?;
    match response.status().as_u16() {
        409 => return Err(format!("The username \"{username}\" is already taken")),
        404 => return Err(NO_ACCOUNTS.to_string()),
        _ => check_status(response).await?,
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
    Ok(())
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
            login_token: &keys.login_token,
        })
        .send()
        .await
        .map_err(request_err)?;
    let found: LogInResponse = match response.status().as_u16() {
        401 => return Err("Wrong username or password".to_string()),
        404 => return Err(NO_ACCOUNTS.to_string()),
        429 => {
            return Err("Too many failed attempts. Wait a few minutes and try again.".to_string())
        }
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
    let session = require_session(state)?;
    let doc = doc::new_group_doc(name, currency, participants)?;
    let group = doc::read_group(&doc)?;
    let me = group
        .participants
        .first()
        .ok_or_else(|| "Add yourself to the group".to_string())?
        .id
        .clone();
    let meta = SyncMeta::new(session.server_url, new_secret()?);

    // Account first: if the app stops in between, the group is fetched again from the
    // account instead of being dropped as "left on another device".
    let mut store = state.store();
    store.update_account(|d| {
        account::add_group(d, &group.id, &meta.server_url, &meta.secret)?;
        account::set_identity(d, &group.id, &me)
    })?;
    let inserted = store.insert(doc, Some(meta));
    if inserted.is_err() {
        let _ = store.update_account(|d| account::remove_group(d, &group.id));
    }
    drop(store);
    state.sync_wakeup.notify_one();
    inserted
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
    store.update_account(|d| account::set_identity(d, group_id, participant_id))?;
    drop(store);
    state.sync_wakeup.notify_one();
    Ok(())
}

/// Adds the user to a group as a new participant.
pub fn add_self(state: &AppState, group_id: &str, name: &str) -> Res<Group> {
    require_session(state)?;
    let mut new_id = String::new();
    state.mutate(group_id, |d| {
        new_id = doc::add_participant(d, name)?;
        Ok(())
    })?;
    set_identity(state, group_id, &new_id)?;
    state.store().group(group_id)
}

// ---------------------------------------------------------------------------
// Background loop
// ---------------------------------------------------------------------------

/// Syncs the account and every shared group in the background: right after local edits and
/// on a timer.
pub fn spawn_background_sync(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let state = app.state::<AppState>();
            tokio::select! {
                _ = state.sync_wakeup.notified() => {}
                _ = tokio::time::sleep(POLL_INTERVAL) => {}
            }
            tokio::time::sleep(DEBOUNCE).await;

            let account_changed = matches!(sync_account(&state).await, Ok(true));
            let groups_changed = match reconcile(&state).await {
                Ok(changed) => changed,
                Err(e) => {
                    eprintln!("[sync] could not update groups from the account: {e}");
                    false
                }
            };
            if account_changed || groups_changed {
                let _ = app.emit(ACCOUNT_EVENT, ());
            }

            let ids = state.store().synced_ids();
            for group_id in ids {
                let changed = matches!(sync_group(&state, &group_id).await, Ok(true));
                let _ = app.emit(SYNC_EVENT, SyncEvent { group_id, changed });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_code_round_trips() {
        let secret = new_secret().unwrap();
        let code = invite_code("https://sync.example.com", "g-1", &secret);
        let invite = parse_invite(&format!("  {code}\n")).unwrap();
        assert_eq!(invite.server_url, "https://sync.example.com");
        assert_eq!(invite.group_id, "g-1");
        assert_eq!(invite.secret, secret);
    }

    #[test]
    fn rejects_bad_invites_and_urls() {
        let secret = new_secret().unwrap();
        let without_version = invite_code("http://a", "g", &secret).replace("&v=2", "");
        let err = parse_invite(&without_version).err().unwrap();
        assert!(err.contains("different version"), "{err}");
        assert!(parse_invite(&invite_code("http://a", "g", "too-short")).is_err());
        assert!(parse_invite("https://example.com/join?group=x").is_err());
        assert!(parse_invite("ezcount://join?server=http%3A%2F%2Fa&group=g").is_err());
        assert!(normalize_server_url("ftp://example.com").is_err());
        assert_eq!(
            normalize_server_url(" http://192.168.1.10:8787/ ").unwrap(),
            "http://192.168.1.10:8787"
        );
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
        assert!(check_password("short").is_err());
        assert!(check_password("long enough").is_ok());
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
            for id in ids {
                sync_group(&self.state, &id).await.unwrap();
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
            doc::add_expense(d, "Taxi", 3000, alice.clone(), vec![split(&alice)], None)
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
        a.edit(&gid, |d| doc::add_participant(d, "Charlie").map(|_| ()));
        a.edit(&gid, |d| {
            doc::update_expense(
                d,
                &hotel,
                "Hotel",
                24000,
                alice.clone(),
                vec![split(&alice), split(&bob)],
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
            )
        });
        b.edit(&gid, |d| doc::remove_participant(d, &bob));

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
                secret.as_bytes(),
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
        assert_ne!(stored, Sha256::digest(secret.as_bytes()).to_vec());
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
}
