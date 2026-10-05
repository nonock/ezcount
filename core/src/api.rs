//! What the app's commands do, for both shells: the Tauri app wraps each function in a
//! `#[tauri::command]` (which is what generates `src/bindings.ts`), and the web build calls
//! them through `invoke`, by command name with JSON arguments, like Tauri's own IPC.
//!
//! Commands that only make sense natively (`native_features`, `share_text`, `share_file`,
//! `save_download`, `save_file`) stay in each shell.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::models::{
    AccountInfo, ExpenseInput, Group, LoginLink, ParticipantBalance, PasswordStrength, Received,
    SettlementTransfer, SignedIn, SyncInfo,
};
use crate::{csv_file, doc, engine, sync, AppState};

type Res<T> = Result<T, String>;

mod invoke;

pub use self::invoke::*;

/// Adds the repeated expenses of a group whose day has come, and empties its trash of what
/// was deleted long ago. Reading a group is when the app does it: there is no one else to.
fn keep_up(state: &AppState, group_id: &str) {
    let now = chrono::Utc::now();
    let due = state
        .store()
        .doc(group_id)
        .is_ok_and(|d| doc::has_due_expenses(d, now) || doc::has_old_trash(d, now));
    if due {
        let done = state.mutate(group_id, |d| {
            doc::add_due_expenses(d, now)?;
            doc::empty_old_trash(d, now)
        });
        if let Err(e) = done {
            eprintln!("[api] could not bring {group_id} up to date: {e}");
        }
    }
}

pub fn get_groups(state: &AppState) -> Vec<Group> {
    let ids = state.store().group_ids();
    for id in ids {
        keep_up(state, &id);
    }
    state.store().groups()
}

pub fn get_group(state: &AppState, group_id: &str) -> Res<Group> {
    keep_up(state, group_id);
    state.store().group(group_id)
}

/// The first participant is the user.
pub fn create_group(
    state: &AppState,
    name: &str,
    currency: &str,
    participants: &[String],
) -> Res<Group> {
    sync::create_group(state, name, currency, participants)
}

/// Creates a group from a CSV file's text, in the format `export_group_csv` writes. The user
/// then says who they are in it.
pub fn import_group_csv(state: &AppState, name: &str, csv: &str) -> Res<Group> {
    sync::import_group(state, name, csv)
}

/// The group as a CSV file's text: a line per expense, a column per person.
pub fn export_group_csv(state: &AppState, group_id: &str) -> Res<String> {
    csv_file::export(&state.store().group(group_id)?)
}

/// The exchange rate to suggest for an expense paid in `from` on `date` (`YYYY-MM-DD`) in a
/// group counting in `to`, from the account's relay. `None` when it has none.
pub async fn suggest_exchange_rate(
    state: &AppState,
    from: &str,
    to: &str,
    date: Option<&str>,
) -> Res<Option<String>> {
    sync::suggested_rate(state, from, to, date).await
}

/// Sends an idea or a problem to whoever runs the account's relay. `contact` is how to
/// answer, when an answer is wanted; `app` says which app it comes from.
pub async fn send_feedback(
    state: &AppState,
    message: &str,
    contact: Option<&str>,
    app: Option<&str>,
) -> Res<()> {
    sync::send_feedback(state, message, contact, app).await
}

/// Deletes the group for every member. While someone still owes something it takes
/// everyone's agreement: this gives the user's, and returns the group still waiting for the
/// others. Returns nothing once the group is deleted.
pub fn delete_group(state: &AppState, group_id: &str) -> Res<Option<Group>> {
    sync::delete_group(state, group_id)
}

/// Refuses the deletion some members asked for, or takes the request back.
pub fn refuse_group_deletion(state: &AppState, group_id: &str) -> Res<Group> {
    state.mutate(group_id, doc::refuse_deletion)
}

/// Puts a group away for the user (it stays theirs, listed apart), or back.
pub fn set_group_archived(state: &AppState, group_id: &str, archived: bool) -> Res<AccountInfo> {
    sync::set_group_archived(state, group_id, archived)?;
    state.require_account_info()
}

/// Removes the group from the account, on all the user's devices. Other members keep it.
pub async fn leave_group(state: &AppState, group_id: &str) -> Res<()> {
    sync::leave_group(state, group_id).await
}

/// Sets the group's name, currency, description and picture (a `data:` URL). Amounts are not
/// converted.
pub fn update_group(
    state: &AppState,
    group_id: &str,
    name: &str,
    currency: &str,
    description: &str,
    image: Option<&str>,
) -> Res<Group> {
    state.mutate(group_id, |d| {
        doc::update_group(d, name, currency, description, image)
    })
}

/// The participant the user is in a group, when they said.
fn me(state: &AppState, group_id: &str) -> Option<String> {
    state.account_info().ok()??.identities.remove(group_id)
}

pub fn add_participant(state: &AppState, group_id: &str, name: &str) -> Res<Group> {
    let by = me(state, group_id);
    state.mutate(group_id, |d| {
        doc::add_participant(d, name, doc::AddedBy::Member(by.as_deref())).map(|_| ())
    })
}

pub fn remove_participant(state: &AppState, group_id: &str, participant_id: &str) -> Res<Group> {
    let by = me(state, group_id);
    state.mutate(group_id, |d| {
        doc::remove_participant(d, participant_id, by.as_deref())
    })
}

pub fn rename_participant(
    state: &AppState,
    group_id: &str,
    participant_id: &str,
    name: &str,
) -> Res<Group> {
    state.mutate(group_id, |d| {
        doc::rename_participant(d, participant_id, name)
    })
}

fn label(expense: &ExpenseInput) -> doc::Label {
    doc::Label::new(&expense.title, expense.category.as_deref())
}

/// Adds an expense, or money that came in (`income`). With `repeat`, it comes back every
/// week, month or year.
pub fn add_expense(state: &AppState, group_id: &str, expense: ExpenseInput) -> Res<Group> {
    let by = me(state, group_id);
    state.mutate(group_id, |d| {
        doc::add_expense_as(
            d,
            label(&expense),
            expense.amount_cents,
            doc::PaidBy::new(expense.paid_by, expense.payers),
            expense.splits,
            expense.created_at,
            expense.original,
            doc::Adding {
                income: expense.income,
                by: by.as_deref(),
                repeat: expense.repeat.as_deref(),
                items: expense.items,
            },
        )
    })
}

/// Replaces an expense with what the form holds, recording what changed in its history.
pub fn update_expense(
    state: &AppState,
    group_id: &str,
    expense_id: &str,
    expense: ExpenseInput,
) -> Res<Group> {
    let by = me(state, group_id);
    state.mutate(group_id, |d| {
        doc::update_expense_as(
            d,
            expense_id,
            label(&expense),
            expense.amount_cents,
            doc::PaidBy::new(expense.paid_by, expense.payers),
            expense.splits,
            expense.created_at,
            expense.original,
            doc::Editing {
                by: by.as_deref(),
                items: expense.items,
            },
        )
    })
}

/// Moves an expense to the group's trash.
pub fn delete_expense(state: &AppState, group_id: &str, expense_id: &str) -> Res<Group> {
    let by = me(state, group_id);
    state.mutate(group_id, |d| {
        doc::delete_expense(d, expense_id, by.as_deref())
    })
}

/// Puts a deleted expense back.
pub fn restore_expense(state: &AppState, group_id: &str, expense_id: &str) -> Res<Group> {
    let by = me(state, group_id);
    state.mutate(group_id, |d| {
        doc::restore_expense(d, expense_id, by.as_deref())
    })
}

/// Removes a deleted expense from the trash, for good.
pub fn purge_expense(state: &AppState, group_id: &str, expense_id: &str) -> Res<Group> {
    state.mutate(group_id, |d| doc::purge_expense(d, expense_id))
}

/// Writes a comment under an expense.
pub fn add_expense_comment(
    state: &AppState,
    group_id: &str,
    expense_id: &str,
    text: &str,
) -> Res<Group> {
    let by = me(state, group_id);
    state.mutate(group_id, |d| {
        doc::add_comment(d, expense_id, text, by.as_deref())
    })
}

/// Removes a comment from under an expense.
pub fn delete_expense_comment(state: &AppState, group_id: &str, comment_id: &str) -> Res<Group> {
    state.mutate(group_id, |d| doc::delete_comment(d, comment_id))
}

/// Stops a repeated expense. The ones already added stay.
pub fn stop_recurring_expense(state: &AppState, group_id: &str, recurring_id: &str) -> Res<Group> {
    state.mutate(group_id, |d| doc::stop_recurring(d, recurring_id))
}

pub fn get_balances(state: &AppState, group_id: &str) -> Res<Vec<ParticipantBalance>> {
    Ok(engine::calculate_balances(&state.store().group(group_id)?))
}

pub fn get_settlements(state: &AppState, group_id: &str) -> Res<Vec<SettlementTransfer>> {
    Ok(engine::calculate_settlements(
        &state.store().group(group_id)?,
    ))
}

pub fn record_reimbursement(
    state: &AppState,
    group_id: &str,
    from_id: String,
    to_id: String,
    amount_cents: i64,
    notes: Option<String>,
) -> Res<Group> {
    let by = me(state, group_id);
    state.mutate(group_id, |d| {
        doc::record_reimbursement(d, from_id, to_id, amount_cents, notes, by.as_deref())
    })
}

pub fn get_storage_warnings(state: &AppState) -> Vec<String> {
    state.warnings.clone()
}

pub fn get_sync_info(state: &AppState, group_id: &str) -> Res<SyncInfo> {
    state.sync_info(group_id)
}

/// Syncs one group immediately. Failures are reported in the returned `last_error`.
pub async fn sync_now(state: &AppState, group_id: &str) -> Res<SyncInfo> {
    let _ = sync::sync_group(state, group_id).await;
    state.sync_info(group_id)
}

pub async fn join_group(state: &AppState, invite_code: &str) -> Res<Group> {
    sync::join_group(state, invite_code).await
}

pub fn get_account(state: &AppState) -> Res<Option<AccountInfo>> {
    state.account_info()
}

pub async fn sign_up(
    state: &AppState,
    server_url: &str,
    username: &str,
    password: &str,
) -> Res<SignedIn> {
    let recovery_key = sync::sign_up(state, server_url, username, password).await?;
    Ok(SignedIn {
        account: state.require_account_info()?,
        recovery_key,
    })
}

/// Sets a new password with the recovery key and logs in. Returns the replacement recovery
/// key: each one works once.
pub async fn recover_account(
    state: &AppState,
    server_url: &str,
    username: &str,
    recovery_key: &str,
    new_password: &str,
) -> Res<SignedIn> {
    let next =
        sync::recover_account(state, server_url, username, recovery_key, new_password).await?;
    Ok(SignedIn {
        account: state.require_account_info()?,
        recovery_key: Some(next),
    })
}

pub async fn change_password(
    state: &AppState,
    current_password: &str,
    new_password: &str,
) -> Res<()> {
    sync::change_password(state, current_password, new_password).await
}

/// A new recovery key, replacing the old one. Returned to show once.
pub async fn replace_recovery_key(state: &AppState, password: &str) -> Res<String> {
    sync::replace_recovery_key(state, password).await
}

pub async fn log_in(
    state: &AppState,
    server_url: &str,
    username: &str,
    password: &str,
) -> Res<AccountInfo> {
    sync::log_in(state, server_url, username, password).await?;
    state.require_account_info()
}

/// A link that logs another device into the account, once and for a short time.
pub async fn create_login_link(state: &AppState, password: &str) -> Res<LoginLink> {
    sync::create_login_link(state, password).await
}

/// Logs in with a link made by `create_login_link` on another device.
pub async fn log_in_with_link(state: &AppState, link: &str) -> Res<AccountInfo> {
    sync::log_in_with_link(state, link).await?;
    state.require_account_info()
}

/// The link a device shows as a QR code to get something from a phone that scans it:
/// `purpose` is "login" to be logged into the phone's account, "group" to join a group.
pub fn receive_link(server_url: &str, purpose: &str) -> Res<String> {
    sync::receive_link(server_url, purpose)
}

/// Asks once whether a phone scanned the code this device shows (`receive_link`). Nothing
/// while none did; then the device is logged in, or has joined the group.
pub async fn receive(state: &AppState, link: &str) -> Res<Option<Received>> {
    sync::receive(state, link).await
}

/// Logs the device showing the scanned `link` into this account.
pub async fn send_login(state: &AppState, link: &str, password: &str) -> Res<()> {
    sync::send_login(state, link, password).await
}

/// Lets the device showing the scanned `link` join a group.
pub async fn send_group_invite(state: &AppState, group_id: &str, link: &str) -> Res<()> {
    sync::send_group_invite(state, group_id, link).await
}

/// Removes the account and its groups from this device. Fails while changes are not uploaded,
/// unless `force` is set.
pub async fn log_out(state: &AppState, force: bool) -> Res<()> {
    sync::log_out(state, force).await
}

/// Records which participant the user is in a group.
pub fn set_identity(state: &AppState, group_id: &str, participant_id: &str) -> Res<AccountInfo> {
    sync::set_identity(state, group_id, participant_id)?;
    state.require_account_info()
}

/// Sets the name, picture (a `data:` URL) and bank account (an IBAN, to be paid back on) the
/// user shows, in every group where they said who they are. An empty name keeps the names
/// the groups have.
pub fn update_profile(
    state: &AppState,
    name: &str,
    avatar: Option<&str>,
    iban: Option<&str>,
) -> Res<AccountInfo> {
    sync::update_profile(state, name, avatar, iban)?;
    state.require_account_info()
}

/// Adds the user to a group as a new participant.
pub fn add_self(state: &AppState, group_id: &str, name: &str) -> Res<Group> {
    sync::add_self(state, group_id, name)
}

/// How hard a password is to guess. Signing up requires `acceptable`.
pub fn password_strength(password: &str, username: &str) -> PasswordStrength {
    sync::password_strength(password, username)
}

#[cfg(test)]
mod tests;
