//! Client side of group sync.
//!
//! The server is a dumb relay: it stores opaque Loro updates per group, in order, and hands
//! them back by sequence number. Each device pushes the operations the server doesn't have
//! yet and pulls everything after the last sequence number it imported. Loro merges the rest.
//!
//! Anyone holding a group's invite code (server URL, group ID and secret key) can read and
//! edit that group. The relay never sees the secret: updates are end-to-end encrypted and the
//! relay only receives a derived auth token (see `crypto`).

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use loro::{ExportMode, LoroDoc, VersionVector};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

use crate::crypto::GroupKeys;
use crate::doc;
use crate::models::Group;
use crate::storage::{import_remote_update, SyncMeta};
use crate::AppState;

type Res<T> = Result<T, String>;

/// How often shared groups are synced in the background, besides right after local edits.
const POLL_INTERVAL: Duration = Duration::from_secs(20);
/// Short pause after a wake-up so a burst of edits goes out as one push.
const DEBOUNCE: Duration = Duration::from_millis(500);

/// Frontend event emitted after each background sync attempt of a group.
pub const SYNC_EVENT: &str = "sync-updated";

#[derive(Clone, Serialize)]
struct SyncEvent {
    group_id: String,
    changed: bool,
}

pub fn http_client() -> Res<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
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
// HTTP protocol
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct UpdatesPage {
    updates: Vec<RemoteUpdate>,
    has_more: bool,
}

#[derive(Deserialize)]
struct RemoteUpdate {
    seq: i64,
    data: String,
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
        404 => Err("The sync server does not know this group".to_string()),
        status => {
            let body = response.text().await.unwrap_or_default();
            Err(format!("The sync server answered {status}: {body}"))
        }
    }
}

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

/// Fetches and decrypts one page of updates after `after`.
/// Returns them in order plus whether more remain.
async fn pull_page(
    http: &reqwest::Client,
    server_url: &str,
    keys: &GroupKeys,
    group_id: &str,
    after: i64,
) -> Res<(Vec<(i64, Vec<u8>)>, bool)> {
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
    Ok((updates, page.has_more))
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// Pushes local changes and pulls remote ones for a shared group.
/// Returns whether remote changes were applied.
pub async fn sync_group(state: &AppState, group_id: &str) -> Res<bool> {
    // One sync at a time, so the background loop and "Sync now" never push the same changes twice.
    let _guard = state.sync_lock.lock().await;
    let result = sync_group_inner(state, group_id).await;
    state.store().record_sync_result(group_id, &result);
    result
}

async fn sync_group_inner(state: &AppState, group_id: &str) -> Res<bool> {
    let (meta, outgoing) = {
        let store = state.store();
        let meta = store
            .sync_meta(group_id)
            .ok_or_else(|| "This group is not shared".to_string())?
            .clone();
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

    let keys = GroupKeys::derive(&meta.secret)?;
    if let Some((bytes, pushed)) = outgoing {
        push(&state.http, &meta.server_url, &keys, group_id, &bytes).await?;
        state.store().mark_pushed(group_id, &pushed)?;
    }

    let mut changed = false;
    loop {
        let after = state
            .store()
            .sync_meta(group_id)
            .map(|m| m.cursor)
            .ok_or_else(|| "This group is not shared".to_string())?;
        let (updates, has_more) =
            pull_page(&state.http, &meta.server_url, &keys, group_id, after).await?;
        if !updates.is_empty() {
            changed |= state.store().import_remote(group_id, &updates)?;
        }
        if !has_more {
            break;
        }
    }
    Ok(changed)
}

/// Starts sharing a local group through `server_url`. Nothing is kept if the first sync fails.
pub async fn enable_sync(state: &AppState, group_id: &str, server_url: &str) -> Res<()> {
    let server_url = normalize_server_url(server_url)?;
    {
        let mut store = state.store();
        if store.sync_meta(group_id).is_some() {
            return Ok(());
        }
        store.set_sync(group_id, SyncMeta::new(server_url, new_secret()?))?;
    }
    if let Err(e) = sync_group(state, group_id).await {
        state.store().clear_sync(group_id)?;
        return Err(e);
    }
    Ok(())
}

/// Downloads a shared group from its invite code and adds it to this device.
pub async fn join_group(state: &AppState, code: &str) -> Res<Group> {
    let invite = parse_invite(code)?;
    if state.store().contains(&invite.group_id) {
        return Err("This group is already on this device".to_string());
    }

    let keys = GroupKeys::derive(&invite.secret)?;
    let doc = LoroDoc::new();
    let mut server_vv = VersionVector::new();
    let mut cursor = 0;
    loop {
        let (updates, has_more) = pull_page(
            &state.http,
            &invite.server_url,
            &keys,
            &invite.group_id,
            cursor,
        )
        .await?;
        for (seq, bytes) in updates {
            import_remote_update(&doc, &bytes, &mut server_vv)?;
            cursor = seq;
        }
        if !has_more {
            break;
        }
    }

    let group = doc::read_group(&doc)
        .map_err(|_| "The sync server has no usable data for this group".to_string())?;
    if group.id != invite.group_id {
        return Err("The invite code does not match the group on the server".to_string());
    }

    let mut meta = SyncMeta::new(invite.server_url, invite.secret);
    meta.server_vv = server_vv;
    meta.cursor = cursor;
    meta.last_synced_at = Some(chrono::Utc::now());
    state.store().insert(doc, Some(meta))
}

/// Syncs every shared group in the background: right after local edits and on a timer.
pub fn spawn_background_sync(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let state = app.state::<AppState>();
            tokio::select! {
                _ = state.sync_wakeup.notified() => {}
                _ = tokio::time::sleep(POLL_INTERVAL) => {}
            }
            tokio::time::sleep(DEBOUNCE).await;

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
}

/// Two devices syncing through a real relay on a local port.
#[cfg(test)]
mod end_to_end {
    use super::*;
    use crate::models::ExpenseSplit;
    use crate::storage::Store;
    use std::path::PathBuf;

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

        fn group(&self, id: &str) -> Group {
            self.state.store().group(id).unwrap()
        }

        fn edit(&self, id: &str, change: impl FnOnce(&LoroDoc) -> Res<()>) {
            self.state.mutate(id, change).unwrap();
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

    fn split(id: &str) -> ExpenseSplit {
        ExpenseSplit {
            participant_id: id.to_string(),
            shares: 1,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn devices_share_and_converge() {
        let (url, relay_dir) = start_relay().await;
        let (a, b) = (Device::new(), Device::new());

        // Device A creates a group with one expense and shares it.
        let doc = doc::new_group_doc("Trip", "EUR", &["Alice".into(), "Bob".into()]).unwrap();
        let group = a.state.store().insert(doc, None).unwrap();
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
        enable_sync(&a.state, &gid, &format!("{url}/"))
            .await
            .unwrap();
        let invite = a.state.sync_info(&gid).unwrap().invite_code.unwrap();

        // Device B joins with the invite code and sees the same group.
        let joined = join_group(&b.state, &invite).await.unwrap();
        assert_eq!(joined, a.group(&gid));
        assert!(
            join_group(&b.state, &invite).await.is_err(),
            "joining twice is refused"
        );

        // Both edit while "offline", then sync in any order.
        let hotel = joined.expenses[0].id.clone();
        a.edit(&gid, |d| doc::add_participant(d, "Charlie"));
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
    async fn relay_sees_neither_data_nor_secret() {
        use sha2::{Digest, Sha256};

        let (url, relay_dir) = start_relay().await;
        let a = Device::new();
        let marker = "Confidential-Hotel-Name-7391";
        let doc = doc::new_group_doc(marker, "EUR", &["Zoe-Marker".into()]).unwrap();
        let gid = a.state.store().insert(doc, None).unwrap().id;
        // Sanity check: an unencrypted Loro update does contain the text.
        let plain = a
            .state
            .store()
            .doc(&gid)
            .unwrap()
            .export(ExportMode::Snapshot)
            .unwrap();
        assert!(plain.windows(marker.len()).any(|w| w == marker.as_bytes()));

        enable_sync(&a.state, &gid, &url).await.unwrap();
        let secret = a.state.store().sync_meta(&gid).unwrap().secret.clone();

        let relay = rusqlite::Connection::open(relay_dir.join("relay.sqlite3")).unwrap();
        let blobs: Vec<Vec<u8>> = relay
            .prepare("SELECT data FROM updates")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(!blobs.is_empty());
        for blob in &blobs {
            for needle in [marker.as_bytes(), b"Zoe-Marker"] {
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

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tampered_update_is_refused() {
        let (url, relay_dir) = start_relay().await;
        let (a, b) = (Device::new(), Device::new());
        let doc = doc::new_group_doc("Flat", "EUR", &["Ann".into()]).unwrap();
        let gid = a.state.store().insert(doc, None).unwrap().id;
        enable_sync(&a.state, &gid, &url).await.unwrap();
        let invite = a.state.sync_info(&gid).unwrap().invite_code.unwrap();

        // A malicious or faulty relay flips one byte of the stored update.
        let relay = rusqlite::Connection::open(relay_dir.join("relay.sqlite3")).unwrap();
        let mut blob: Vec<u8> = relay
            .query_row("SELECT data FROM updates LIMIT 1", [], |r| r.get(0))
            .unwrap();
        *blob.last_mut().unwrap() ^= 1;
        relay
            .execute("UPDATE updates SET data = ?1", [&blob])
            .unwrap();

        let err = join_group(&b.state, &invite).await.unwrap_err();
        assert!(err.contains("decrypted"), "{err}");
        assert!(!b.state.store().contains(&gid));

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn wrong_key_is_rejected() {
        let (url, relay_dir) = start_relay().await;
        let (a, b) = (Device::new(), Device::new());
        let doc = doc::new_group_doc("Flat", "EUR", &["Ann".into()]).unwrap();
        let gid = a.state.store().insert(doc, None).unwrap().id;
        enable_sync(&a.state, &gid, &url).await.unwrap();

        let forged = invite_code(&url, &gid, &new_secret().unwrap());
        let err = join_group(&b.state, &forged).await.unwrap_err();
        assert!(err.contains("rejected"), "{err}");

        let missing = invite_code(&url, "no-such-group", &new_secret().unwrap());
        assert!(join_group(&b.state, &missing).await.is_err());

        let _ = std::fs::remove_dir_all(relay_dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn unreachable_server_keeps_group_local() {
        let a = Device::new();
        let doc = doc::new_group_doc("Solo", "EUR", &["Ann".into()]).unwrap();
        let gid = a.state.store().insert(doc, None).unwrap().id;
        // Port 9 (discard) on localhost is essentially never listening.
        let err = enable_sync(&a.state, &gid, "http://127.0.0.1:9")
            .await
            .unwrap_err();
        assert!(err.contains("reach"), "{err}");
        assert!(!a.state.sync_info(&gid).unwrap().enabled);
    }
}
