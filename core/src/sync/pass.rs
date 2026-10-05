//! A pass of sync: each group and the account with the relay, then the device's groups
//! brought in line with the account's.

use super::*;

/// Pushes local changes and pulls remote ones for a shared group (or the account document).
/// Returns whether remote changes were applied.
pub async fn sync_group(state: &AppState, group_id: &str) -> Res<bool> {
    // One sync at a time, so the background loop and "Sync now" never push the same changes twice.
    let _guard = state.sync_lock.lock().await;
    let result = sync_group_inner(state, group_id).await;
    state.store().record_sync_result(group_id, &result);
    result
}

pub(super) async fn sync_group_inner(state: &AppState, group_id: &str) -> Res<bool> {
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
        // A list this version of the app may misread adds and removes nothing here.
        if account::needs_newer_app(store.account_doc()?) {
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

/// What a `sync_all` pass found, reported as it goes so the interface can refresh early.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    /// The account's groups or identities changed on another device, or the account was
    /// deleted there and this device logged out, or whether this version of the app is too
    /// old changed (`AppState::update_required`).
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
    let was_too_old = state.update_required();
    let account = sync_account(state).await;
    // Deleted on another device: the groups below get what this one still had for them,
    // without the user's profile, then it logs out.
    let deleted = matches!(&account, Err(e) if e == ACCOUNT_DELETED);
    if deleted {
        hide_profile(state);
    }
    // Becoming too old for the relay or the account, or no longer being, is news for the
    // interface too.
    let account_changed = matches!(account, Ok(true)) || state.update_required() != was_too_old;
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
    if deleted && leave_deleted_account(state).await {
        report(SyncEvent::Account);
    }
    notices
}
