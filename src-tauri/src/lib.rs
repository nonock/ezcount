mod crypto;
mod doc;
mod engine;
pub mod models;
pub mod storage;
pub mod sync;

use chrono::{DateTime, Utc};
use loro::LoroDoc;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use tauri::{Manager, State};

use crate::models::{ExpenseSplit, Group, ParticipantBalance, SettlementTransfer, SyncInfo};
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
    fn mutate(
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

    fn sync_info(&self, group_id: &str) -> Result<SyncInfo, String> {
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

#[tauri::command]
#[specta::specta]
fn get_groups(state: State<AppState>) -> Vec<Group> {
    state.store().groups()
}

#[tauri::command]
#[specta::specta]
fn get_group(state: State<AppState>, group_id: String) -> Result<Group, String> {
    state.store().group(&group_id)
}

#[tauri::command]
#[specta::specta]
fn create_group(
    state: State<AppState>,
    name: String,
    currency: String,
    participants: Vec<String>,
) -> Result<Group, String> {
    let doc = doc::new_group_doc(&name, &currency, &participants)?;
    state.store().insert(doc, None)
}

/// Removes the group from this device. Other members of a shared group keep it.
#[tauri::command]
#[specta::specta]
fn delete_group(state: State<AppState>, group_id: String) -> Result<bool, String> {
    state.store().delete(&group_id)
}

#[tauri::command]
#[specta::specta]
fn add_participant(
    state: State<AppState>,
    group_id: String,
    name: String,
) -> Result<Group, String> {
    state.mutate(&group_id, |d| doc::add_participant(d, &name))
}

#[tauri::command]
#[specta::specta]
fn remove_participant(
    state: State<AppState>,
    group_id: String,
    participant_id: String,
) -> Result<Group, String> {
    state.mutate(&group_id, |d| doc::remove_participant(d, &participant_id))
}

#[tauri::command]
#[specta::specta]
fn add_expense(
    state: State<AppState>,
    group_id: String,
    title: String,
    amount_cents: i64,
    paid_by: String,
    splits: Vec<ExpenseSplit>,
    created_at: Option<DateTime<Utc>>,
) -> Result<Group, String> {
    state.mutate(&group_id, |d| {
        doc::add_expense(d, &title, amount_cents, paid_by, splits, created_at)
    })
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
fn update_expense(
    state: State<AppState>,
    group_id: String,
    expense_id: String,
    title: String,
    amount_cents: i64,
    paid_by: String,
    splits: Vec<ExpenseSplit>,
    created_at: Option<DateTime<Utc>>,
) -> Result<Group, String> {
    state.mutate(&group_id, |d| {
        doc::update_expense(
            d,
            &expense_id,
            &title,
            amount_cents,
            paid_by,
            splits,
            created_at,
        )
    })
}

#[tauri::command]
#[specta::specta]
fn delete_expense(
    state: State<AppState>,
    group_id: String,
    expense_id: String,
) -> Result<Group, String> {
    state.mutate(&group_id, |d| doc::delete_expense(d, &expense_id))
}

#[tauri::command]
#[specta::specta]
fn get_balances(
    state: State<AppState>,
    group_id: String,
) -> Result<Vec<ParticipantBalance>, String> {
    Ok(engine::calculate_balances(&state.store().group(&group_id)?))
}

#[tauri::command]
#[specta::specta]
fn get_settlements(
    state: State<AppState>,
    group_id: String,
) -> Result<Vec<SettlementTransfer>, String> {
    Ok(engine::calculate_settlements(
        &state.store().group(&group_id)?,
    ))
}

#[tauri::command]
#[specta::specta]
fn record_reimbursement(
    state: State<AppState>,
    group_id: String,
    from_id: String,
    to_id: String,
    amount_cents: i64,
    notes: Option<String>,
) -> Result<Group, String> {
    state.mutate(&group_id, |d| {
        doc::record_reimbursement(d, from_id, to_id, amount_cents, notes)
    })
}

#[tauri::command]
#[specta::specta]
fn get_storage_warnings(state: State<AppState>) -> Vec<String> {
    state.warnings.clone()
}

#[tauri::command]
#[specta::specta]
fn get_sync_info(state: State<AppState>, group_id: String) -> Result<SyncInfo, String> {
    state.sync_info(&group_id)
}

/// Starts sharing a group through a sync server and returns its invite code.
#[tauri::command]
#[specta::specta]
async fn enable_sync(
    state: State<'_, AppState>,
    group_id: String,
    server_url: String,
) -> Result<SyncInfo, String> {
    sync::enable_sync(&state, &group_id, &server_url).await?;
    state.sync_info(&group_id)
}

/// Syncs one group immediately. Failures are reported in the returned `last_error`.
#[tauri::command]
#[specta::specta]
async fn sync_now(state: State<'_, AppState>, group_id: String) -> Result<SyncInfo, String> {
    let _ = sync::sync_group(&state, &group_id).await;
    state.sync_info(&group_id)
}

#[tauri::command]
#[specta::specta]
async fn join_group(state: State<'_, AppState>, invite_code: String) -> Result<Group, String> {
    sync::join_group(&state, &invite_code).await
}

pub fn create_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        get_groups,
        get_group,
        create_group,
        delete_group,
        add_participant,
        remove_participant,
        add_expense,
        update_expense,
        delete_expense,
        record_reimbursement,
        get_balances,
        get_settlements,
        get_storage_warnings,
        get_sync_info,
        enable_sync,
        sync_now,
        join_group
    ])
}

/// Writes `src/bindings.ts`. Used by both the debug app startup and the `export_bindings`
/// binary (pre-commit hook), so the two always produce identical files.
pub fn export_bindings(
    builder: &tauri_specta::Builder<tauri::Wry>,
) -> Result<std::path::PathBuf, String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/bindings.ts");
    builder
        .export(
            specta_typescript::Typescript::default()
                .bigint(specta_typescript::BigIntExportBehavior::Number)
                .header(
                    "// @ts-nocheck\n// Auto-generated by tauri-specta. Do not edit manually.\n",
                ),
            &path,
        )
        .map_err(|e| format!("Failed to export TypeScript bindings: {e:?}"))?;
    Ok(path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = create_specta_builder();

    #[cfg(all(debug_assertions, not(mobile)))]
    export_bindings(&builder).expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("./"));
            let (store, warnings) = Store::open(
                &app_data.join("ezcount.sqlite3"),
                &app_data.join("ezcount_data.json"),
            )?;
            for warning in &warnings {
                eprintln!("[storage] {warning}");
            }
            app.manage(AppState::new(store, warnings)?);
            sync::spawn_background_sync(app.handle().clone());
            Ok(())
        })
        .invoke_handler(builder.invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
