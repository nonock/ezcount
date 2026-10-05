//! Expenses, payments, comments and repeated expenses.

use ezcount_core::models::{ExpenseInput, Group};
use ezcount_core::{api, AppState};
use tauri::State;

/// The exchange rate to suggest for an expense paid in `from` on `date` (`YYYY-MM-DD`) in a
/// group counting in `to`, from the account's relay. Null when it has none.
#[tauri::command]
#[specta::specta]
pub(crate) async fn suggest_exchange_rate(
    state: State<'_, AppState>,
    from: String,
    to: String,
    date: Option<String>,
) -> Result<Option<String>, String> {
    api::suggest_exchange_rate(&state, &from, &to, date.as_deref()).await
}

#[tauri::command]
#[specta::specta]
pub(crate) fn add_expense(
    state: State<AppState>,
    group_id: String,
    expense: ExpenseInput,
) -> Result<Group, String> {
    api::add_expense(&state, &group_id, expense)
}

/// Replaces an expense with what the form holds, recording what changed in its history.
#[tauri::command]
#[specta::specta]
pub(crate) fn update_expense(
    state: State<AppState>,
    group_id: String,
    expense_id: String,
    expense: ExpenseInput,
) -> Result<Group, String> {
    api::update_expense(&state, &group_id, &expense_id, expense)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_expense(
    state: State<AppState>,
    group_id: String,
    expense_id: String,
) -> Result<Group, String> {
    api::delete_expense(&state, &group_id, &expense_id)
}

/// Puts a deleted expense back.
#[tauri::command]
#[specta::specta]
pub(crate) fn restore_expense(
    state: State<AppState>,
    group_id: String,
    expense_id: String,
) -> Result<Group, String> {
    api::restore_expense(&state, &group_id, &expense_id)
}

/// Removes a deleted expense from the trash, for good.
#[tauri::command]
#[specta::specta]
pub(crate) fn purge_expense(
    state: State<AppState>,
    group_id: String,
    expense_id: String,
) -> Result<Group, String> {
    api::purge_expense(&state, &group_id, &expense_id)
}

/// Writes a comment under an expense.
#[tauri::command]
#[specta::specta]
pub(crate) fn add_expense_comment(
    state: State<AppState>,
    group_id: String,
    expense_id: String,
    text: String,
) -> Result<Group, String> {
    api::add_expense_comment(&state, &group_id, &expense_id, &text)
}

/// Removes a comment from under an expense.
#[tauri::command]
#[specta::specta]
pub(crate) fn delete_expense_comment(
    state: State<AppState>,
    group_id: String,
    comment_id: String,
) -> Result<Group, String> {
    api::delete_expense_comment(&state, &group_id, &comment_id)
}

/// Stops a repeated expense. The ones already added stay.
#[tauri::command]
#[specta::specta]
pub(crate) fn stop_recurring_expense(
    state: State<AppState>,
    group_id: String,
    recurring_id: String,
) -> Result<Group, String> {
    api::stop_recurring_expense(&state, &group_id, &recurring_id)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn record_reimbursement(
    state: State<AppState>,
    group_id: String,
    from_id: String,
    to_id: String,
    amount_cents: i64,
    notes: Option<String>,
) -> Result<Group, String> {
    api::record_reimbursement(&state, &group_id, from_id, to_id, amount_cents, notes)
}
