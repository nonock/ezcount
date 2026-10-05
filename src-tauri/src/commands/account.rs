//! The account: signing up and logging in, the profile, handing a login or an invite to
//! another device, and feedback.

use ezcount_core::models::{AccountInfo, Group, LoginLink, PasswordStrength, Received, SignedIn};
use ezcount_core::{api, AppState};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub(crate) fn get_account(state: State<AppState>) -> Result<Option<AccountInfo>, String> {
    api::get_account(&state)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn sign_up(
    state: State<'_, AppState>,
    server_url: String,
    username: String,
    password: String,
) -> Result<SignedIn, String> {
    api::sign_up(&state, &server_url, &username, &password).await
}

/// Sets a new password with the recovery key and logs in. Returns the replacement recovery
/// key: each one works once.
#[tauri::command]
#[specta::specta]
pub(crate) async fn recover_account(
    state: State<'_, AppState>,
    server_url: String,
    username: String,
    recovery_key: String,
    new_password: String,
) -> Result<SignedIn, String> {
    api::recover_account(&state, &server_url, &username, &recovery_key, &new_password).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn change_password(
    state: State<'_, AppState>,
    current_password: String,
    new_password: String,
) -> Result<(), String> {
    api::change_password(&state, &current_password, &new_password).await
}

/// A new recovery key, replacing the old one. Returned to show once.
#[tauri::command]
#[specta::specta]
pub(crate) async fn replace_recovery_key(
    state: State<'_, AppState>,
    password: String,
) -> Result<String, String> {
    api::replace_recovery_key(&state, &password).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn log_in(
    state: State<'_, AppState>,
    server_url: String,
    username: String,
    password: String,
) -> Result<AccountInfo, String> {
    api::log_in(&state, &server_url, &username, &password).await
}

/// A link that logs another device into the account, once and for a short time. Shown as a
/// QR code.
#[tauri::command]
#[specta::specta]
pub(crate) async fn create_login_link(
    state: State<'_, AppState>,
    password: String,
) -> Result<LoginLink, String> {
    api::create_login_link(&state, &password).await
}

/// Logs in with a link made by `create_login_link` on another device.
#[tauri::command]
#[specta::specta]
pub(crate) async fn log_in_with_link(
    state: State<'_, AppState>,
    link: String,
) -> Result<AccountInfo, String> {
    api::log_in_with_link(&state, &link).await
}

/// Removes the account and its groups from this device. Fails while changes are not uploaded,
/// unless `force` is set.
#[tauri::command]
#[specta::specta]
pub(crate) async fn log_out(state: State<'_, AppState>, force: bool) -> Result<(), String> {
    api::log_out(&state, force).await
}

/// Records which participant the user is in a group.
#[tauri::command]
#[specta::specta]
pub(crate) fn set_identity(
    state: State<AppState>,
    group_id: String,
    participant_id: String,
) -> Result<AccountInfo, String> {
    api::set_identity(&state, &group_id, &participant_id)
}

/// Sets the name, picture (a `data:` URL) and bank account (an IBAN, to be paid back on) the
/// user shows, in every group where they said who they are. An empty name keeps the names
/// the groups have.
#[tauri::command]
#[specta::specta]
pub(crate) fn update_profile(
    state: State<AppState>,
    name: String,
    avatar: Option<String>,
    iban: Option<String>,
) -> Result<AccountInfo, String> {
    api::update_profile(&state, &name, avatar.as_deref(), iban.as_deref())
}

/// Adds the user to a group as a new participant.
#[tauri::command]
#[specta::specta]
pub(crate) fn add_self(
    state: State<AppState>,
    group_id: String,
    name: String,
) -> Result<Group, String> {
    api::add_self(&state, &group_id, &name)
}

/// How hard a password is to guess. Signing up requires `acceptable`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn password_strength(password: String, username: String) -> PasswordStrength {
    api::password_strength(&password, &username)
}

/// The link a device shows as a QR code to get something from a phone that scans it:
/// `purpose` is "login" to be logged into the phone's account, "group" to join a group.
#[tauri::command]
#[specta::specta]
pub(crate) fn receive_link(server_url: String, purpose: String) -> Result<String, String> {
    api::receive_link(&server_url, &purpose)
}

/// Asks once whether a phone scanned the code this device shows (`receive_link`). Nothing
/// while none did; then the device is logged in, or has joined the group.
#[tauri::command]
#[specta::specta]
pub(crate) async fn receive(
    state: State<'_, AppState>,
    link: String,
) -> Result<Option<Received>, String> {
    api::receive(&state, &link).await
}

/// Logs the device showing the scanned `link` into this account.
#[tauri::command]
#[specta::specta]
pub(crate) async fn send_login(
    state: State<'_, AppState>,
    link: String,
    password: String,
) -> Result<(), String> {
    api::send_login(&state, &link, &password).await
}

/// Lets the device showing the scanned `link` join a group.
#[tauri::command]
#[specta::specta]
pub(crate) async fn send_group_invite(
    state: State<'_, AppState>,
    group_id: String,
    link: String,
) -> Result<(), String> {
    api::send_group_invite(&state, &group_id, &link).await
}

/// Sends an idea or a problem to whoever runs the account's relay. `contact` is how to
/// answer, when an answer is wanted; `app` says which app it comes from.
#[tauri::command]
#[specta::specta]
pub(crate) async fn send_feedback(
    state: State<'_, AppState>,
    message: String,
    contact: Option<String>,
    app: Option<String>,
) -> Result<(), String> {
    api::send_feedback(&state, &message, contact.as_deref(), app.as_deref()).await
}
