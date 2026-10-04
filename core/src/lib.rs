//! ezcount without a user interface: groups as Loro documents (`doc`), balances (`engine`),
//! encryption (`crypto`), the account (`account`), local storage (`storage`) and sync with the
//! relay (`sync`). It knows nothing of Tauri: the desktop and Android app (`src-tauri`) exposes
//! it as commands and runs the background sync loop around `sync::sync_all`.

mod account;
pub mod api;
mod crypto;
pub mod csv_file;
pub mod doc;
pub mod engine;
pub mod models;
pub mod notices;
pub mod storage;
pub mod sync;

use loro::LoroDoc;
use std::sync::{Mutex, MutexGuard};

use crate::models::{AccountInfo, Group, SyncInfo};
use crate::storage::Store;

pub struct AppState {
    store: Mutex<Store>,
    pub http: reqwest::Client,
    /// Serializes sync runs.
    pub sync_lock: tokio::sync::Mutex<()>,
    /// Wakes the background sync loop after a local edit.
    pub sync_wakeup: tokio::sync::Notify,
    /// Problems found while loading data, shown to the user once.
    pub warnings: Vec<String>,
}

impl AppState {
    pub fn new(store: Store, warnings: Vec<String>) -> Result<Self, String> {
        Ok(Self {
            store: Mutex::new(store),
            http: sync::http_client()?,
            sync_lock: tokio::sync::Mutex::new(()),
            sync_wakeup: tokio::sync::Notify::new(),
            warnings,
        })
    }

    pub fn store(&self) -> MutexGuard<'_, Store> {
        // Every write is committed to SQLite before memory is touched, so the data is still
        // consistent after a panic in another command.
        self.store.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Applies a change to a group, saves it and schedules a sync if the group is shared.
    pub fn mutate(
        &self,
        group_id: &str,
        change: impl FnOnce(&LoroDoc) -> Result<(), String>,
    ) -> Result<Group, String> {
        let mut store = self.store();
        let group = store.update(group_id, change)?;
        if store.sync_meta(group_id).is_some() {
            self.sync_wakeup.notify_one();
        }
        Ok(group)
    }

    pub fn account_info(&self) -> Result<Option<AccountInfo>, String> {
        let store = self.store();
        let Some(session) = store.session() else {
            return Ok(None);
        };
        let profile = account::profile(store.account_doc()?);
        Ok(Some(AccountInfo {
            username: session.username.clone(),
            server_url: session.server_url.clone(),
            display_name: profile.name,
            avatar: profile.avatar,
            iban: profile.iban,
            archived: account::archived(store.account_doc()?),
            identities: account::identities(store.account_doc()?)?,
        }))
    }

    pub fn require_account_info(&self) -> Result<AccountInfo, String> {
        self.account_info()?
            .ok_or_else(|| "You are not logged in".to_string())
    }

    pub fn sync_info(&self, group_id: &str) -> Result<SyncInfo, String> {
        let store = self.store();
        store.doc(group_id)?;
        let meta = store.sync_meta(group_id);
        Ok(SyncInfo {
            group_id: group_id.to_string(),
            enabled: meta.is_some(),
            server_url: meta.map(|m| m.server_url.clone()),
            invite_code: meta.map(|m| sync::invite_code(&m.server_url, group_id, &m.secret)),
            last_synced_at: meta.and_then(|m| m.last_synced_at),
            last_error: meta.and_then(|m| m.last_error.clone()),
        })
    }
}
