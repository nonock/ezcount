//! Groups, their members, balances and sync.

use ezcount_core::models::{AccountInfo, Group, ParticipantBalance, SettlementTransfer, SyncInfo};
use ezcount_core::{api, AppState};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub(crate) fn get_groups(state: State<AppState>) -> Vec<Group> {
    api::get_groups(&state)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_group(state: State<AppState>, group_id: String) -> Result<Group, String> {
    api::get_group(&state, &group_id)
}

#[tauri::command]
#[specta::specta]
/// The first participant is the user.
pub(crate) fn create_group(
    state: State<AppState>,
    name: String,
    currency: String,
    participants: Vec<String>,
) -> Result<Group, String> {
    api::create_group(&state, &name, &currency, &participants)
}

/// Creates a group from a CSV file's text, in the format `export_group_csv` writes. The user
/// then says who they are in it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn import_group_csv(
    state: State<'_, AppState>,
    name: String,
    csv: String,
) -> Result<Group, String> {
    api::import_group_csv(&state, &name, &csv)
}

/// The group as a CSV file's text: a line per expense, a column per person.
#[tauri::command]
#[specta::specta]
pub(crate) async fn export_group_csv(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<String, String> {
    api::export_group_csv(&state, &group_id)
}

/// Removes the group from the account, on all the user's devices. Other members keep it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn leave_group(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<(), String> {
    api::leave_group(&state, &group_id).await
}

/// Renames the group and sets its currency. Amounts are not converted.
/// Deletes the group for every member. While someone still owes something it takes
/// everyone's agreement: this gives the user's, and returns the group still waiting for the
/// others. Returns nothing once the group is deleted.
#[tauri::command]
#[specta::specta]
pub(crate) fn delete_group(
    state: State<AppState>,
    group_id: String,
) -> Result<Option<Group>, String> {
    api::delete_group(&state, &group_id)
}

/// Refuses the deletion some members asked for, or takes the request back.
#[tauri::command]
#[specta::specta]
pub(crate) fn refuse_group_deletion(
    state: State<AppState>,
    group_id: String,
) -> Result<Group, String> {
    api::refuse_group_deletion(&state, &group_id)
}

/// Puts a group away for the user (it stays theirs, listed apart), or back.
#[tauri::command]
#[specta::specta]
pub(crate) fn set_group_archived(
    state: State<AppState>,
    group_id: String,
    archived: bool,
) -> Result<AccountInfo, String> {
    api::set_group_archived(&state, &group_id, archived)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn update_group(
    state: State<AppState>,
    group_id: String,
    name: String,
    currency: String,
    description: String,
    image: Option<String>,
) -> Result<Group, String> {
    api::update_group(
        &state,
        &group_id,
        &name,
        &currency,
        &description,
        image.as_deref(),
    )
}

#[tauri::command]
#[specta::specta]
pub(crate) fn add_participant(
    state: State<AppState>,
    group_id: String,
    name: String,
) -> Result<Group, String> {
    api::add_participant(&state, &group_id, &name)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn remove_participant(
    state: State<AppState>,
    group_id: String,
    participant_id: String,
) -> Result<Group, String> {
    api::remove_participant(&state, &group_id, &participant_id)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn rename_participant(
    state: State<AppState>,
    group_id: String,
    participant_id: String,
    name: String,
) -> Result<Group, String> {
    api::rename_participant(&state, &group_id, &participant_id, &name)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_balances(
    state: State<AppState>,
    group_id: String,
) -> Result<Vec<ParticipantBalance>, String> {
    api::get_balances(&state, &group_id)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_settlements(
    state: State<AppState>,
    group_id: String,
) -> Result<Vec<SettlementTransfer>, String> {
    api::get_settlements(&state, &group_id)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_storage_warnings(state: State<AppState>) -> Vec<String> {
    api::get_storage_warnings(&state)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_sync_info(state: State<AppState>, group_id: String) -> Result<SyncInfo, String> {
    api::get_sync_info(&state, &group_id)
}

/// Syncs one group immediately. Failures are reported in the returned `last_error`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_now(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<SyncInfo, String> {
    api::sync_now(&state, &group_id).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn join_group(
    state: State<'_, AppState>,
    invite_code: String,
) -> Result<Group, String> {
    api::join_group(&state, &invite_code).await
}
