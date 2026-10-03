//! The background sync loop: `sync::sync_all` right after local edits and on a timer, with
//! its findings sent to the frontend as events.

use ezcount_core::sync::{self, SyncEvent};
use ezcount_core::AppState;
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// How often shared groups are synced in the background, besides right after local edits.
const POLL_INTERVAL: Duration = Duration::from_secs(20);
/// Short pause after a wake-up so a burst of edits goes out as one push.
const DEBOUNCE: Duration = Duration::from_millis(500);

/// Frontend event emitted after each background sync attempt of a group.
const SYNC_EVENT: &str = "sync-updated";
/// Frontend event emitted when the account's groups or identities changed on another device.
const ACCOUNT_EVENT: &str = "account-updated";

#[derive(Clone, Serialize)]
struct GroupSynced {
    group_id: String,
    changed: bool,
}

pub fn spawn_sync(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let state = app.state::<AppState>();
            tokio::select! {
                _ = state.sync_wakeup.notified() => {}
                _ = tokio::time::sleep(POLL_INTERVAL) => {}
            }
            tokio::time::sleep(DEBOUNCE).await;

            sync::sync_all(&state, |event| {
                let _ = match event {
                    SyncEvent::Account => app.emit(ACCOUNT_EVENT, ()),
                    SyncEvent::Group { group_id, changed } => {
                        app.emit(SYNC_EVENT, GroupSynced { group_id, changed })
                    }
                };
            })
            .await;
        }
    });
}
