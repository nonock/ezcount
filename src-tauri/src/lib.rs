mod engine;
mod models;
mod storage;

use std::path::PathBuf;
use tauri::{Manager, State};

use crate::models::{Group, ParticipantBalance, SettlementTransfer};
use crate::storage::AppState;

#[tauri::command]
fn get_groups(state: State<AppState>) -> Vec<Group> {
    state.get_groups()
}

#[tauri::command]
fn get_group(state: State<AppState>, group_id: String) -> Option<Group> {
    state.get_group(&group_id)
}

#[tauri::command]
fn create_group(
    state: State<AppState>,
    name: String,
    currency: String,
    participants: Vec<String>,
) -> Result<Group, String> {
    state.create_group(name, currency, participants)
}

#[tauri::command]
fn delete_group(state: State<AppState>, group_id: String) -> Result<bool, String> {
    state.delete_group(&group_id)
}

#[tauri::command]
fn add_participant(
    state: State<AppState>,
    group_id: String,
    name: String,
) -> Result<Group, String> {
    state.add_participant(&group_id, name)
}

#[tauri::command]
fn add_expense(
    state: State<AppState>,
    group_id: String,
    title: String,
    amount_cents: i64,
    paid_by: String,
    split_among: Vec<String>,
) -> Result<Group, String> {
    state.add_expense(&group_id, title, amount_cents, paid_by, split_among)
}

#[tauri::command]
fn delete_expense(
    state: State<AppState>,
    group_id: String,
    expense_id: String,
) -> Result<Group, String> {
    state.delete_expense(&group_id, &expense_id)
}

#[tauri::command]
fn get_balances(
    state: State<AppState>,
    group_id: String,
) -> Result<Vec<ParticipantBalance>, String> {
    match state.get_group(&group_id) {
        Some(group) => Ok(engine::calculate_balances(&group)),
        None => Err("Group not found".to_string()),
    }
}

#[tauri::command]
fn get_settlements(
    state: State<AppState>,
    group_id: String,
) -> Result<Vec<SettlementTransfer>, String> {
    match state.get_group(&group_id) {
        Some(group) => Ok(engine::calculate_settlements(&group)),
        None => Err("Group not found".to_string()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("./"));
            let file_path = app_data.join("ezcount_data.json");
            app.manage(AppState::new(file_path));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_groups,
            get_group,
            create_group,
            delete_group,
            add_participant,
            add_expense,
            delete_expense,
            get_balances,
            get_settlements
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
