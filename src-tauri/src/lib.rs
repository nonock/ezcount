//! The desktop and Android app: `ezcount-core` (in `../core`) exposed as Tauri commands, plus
//! the background sync loop and the native bits (window icons, deep links, share sheet).

mod background;
#[cfg(target_os = "android")]
mod notify;
#[cfg(target_os = "android")]
mod share;

use ezcount_core::models::{
    AccountInfo, ExpenseInput, Group, LoginLink, NativeFeatures, ParticipantBalance,
    PasswordStrength, Received, SettlementTransfer, SignedIn, SyncInfo,
};
use ezcount_core::storage::Store;
use ezcount_core::{api, AppState};
use std::path::PathBuf;
#[cfg(windows)]
use std::sync::Mutex;
use tauri::{Manager, State};

#[tauri::command]
#[specta::specta]
fn get_groups(state: State<AppState>) -> Vec<Group> {
    api::get_groups(&state)
}

#[tauri::command]
#[specta::specta]
fn get_group(state: State<AppState>, group_id: String) -> Result<Group, String> {
    api::get_group(&state, &group_id)
}

#[tauri::command]
#[specta::specta]
/// The first participant is the user.
fn create_group(
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
async fn import_group_csv(
    state: State<'_, AppState>,
    name: String,
    csv: String,
) -> Result<Group, String> {
    api::import_group_csv(&state, &name, &csv)
}

/// The group as a CSV file's text: a line per expense, a column per person.
#[tauri::command]
#[specta::specta]
async fn export_group_csv(state: State<'_, AppState>, group_id: String) -> Result<String, String> {
    api::export_group_csv(&state, &group_id)
}

/// The exchange rate to suggest for an expense paid in `from` on `date` (`YYYY-MM-DD`) in a
/// group counting in `to`, from the account's relay. Null when it has none.
#[tauri::command]
#[specta::specta]
async fn suggest_exchange_rate(
    state: State<'_, AppState>,
    from: String,
    to: String,
    date: Option<String>,
) -> Result<Option<String>, String> {
    api::suggest_exchange_rate(&state, &from, &to, date.as_deref()).await
}

/// Removes the group from the account, on all the user's devices. Other members keep it.
#[tauri::command]
#[specta::specta]
async fn leave_group(state: State<'_, AppState>, group_id: String) -> Result<(), String> {
    api::leave_group(&state, &group_id).await
}

/// Renames the group and sets its currency. Amounts are not converted.
/// Deletes the group for every member. While someone still owes something it takes
/// everyone's agreement: this gives the user's, and returns the group still waiting for the
/// others. Returns nothing once the group is deleted.
#[tauri::command]
#[specta::specta]
fn delete_group(state: State<AppState>, group_id: String) -> Result<Option<Group>, String> {
    api::delete_group(&state, &group_id)
}

/// Refuses the deletion some members asked for, or takes the request back.
#[tauri::command]
#[specta::specta]
fn refuse_group_deletion(state: State<AppState>, group_id: String) -> Result<Group, String> {
    api::refuse_group_deletion(&state, &group_id)
}

/// Puts a group away for the user (it stays theirs, listed apart), or back.
#[tauri::command]
#[specta::specta]
fn set_group_archived(
    state: State<AppState>,
    group_id: String,
    archived: bool,
) -> Result<AccountInfo, String> {
    api::set_group_archived(&state, &group_id, archived)
}

#[tauri::command]
#[specta::specta]
fn update_group(
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
fn add_participant(
    state: State<AppState>,
    group_id: String,
    name: String,
) -> Result<Group, String> {
    api::add_participant(&state, &group_id, &name)
}

#[tauri::command]
#[specta::specta]
fn remove_participant(
    state: State<AppState>,
    group_id: String,
    participant_id: String,
) -> Result<Group, String> {
    api::remove_participant(&state, &group_id, &participant_id)
}

#[tauri::command]
#[specta::specta]
fn rename_participant(
    state: State<AppState>,
    group_id: String,
    participant_id: String,
    name: String,
) -> Result<Group, String> {
    api::rename_participant(&state, &group_id, &participant_id, &name)
}

#[tauri::command]
#[specta::specta]
fn add_expense(
    state: State<AppState>,
    group_id: String,
    expense: ExpenseInput,
) -> Result<Group, String> {
    api::add_expense(&state, &group_id, expense)
}

/// Replaces an expense with what the form holds, recording what changed in its history.
#[tauri::command]
#[specta::specta]
fn update_expense(
    state: State<AppState>,
    group_id: String,
    expense_id: String,
    expense: ExpenseInput,
) -> Result<Group, String> {
    api::update_expense(&state, &group_id, &expense_id, expense)
}

#[tauri::command]
#[specta::specta]
fn delete_expense(
    state: State<AppState>,
    group_id: String,
    expense_id: String,
) -> Result<Group, String> {
    api::delete_expense(&state, &group_id, &expense_id)
}

/// Puts a deleted expense back.
#[tauri::command]
#[specta::specta]
fn restore_expense(
    state: State<AppState>,
    group_id: String,
    expense_id: String,
) -> Result<Group, String> {
    api::restore_expense(&state, &group_id, &expense_id)
}

/// Removes a deleted expense from the trash, for good.
#[tauri::command]
#[specta::specta]
fn purge_expense(
    state: State<AppState>,
    group_id: String,
    expense_id: String,
) -> Result<Group, String> {
    api::purge_expense(&state, &group_id, &expense_id)
}

/// Writes a comment under an expense.
#[tauri::command]
#[specta::specta]
fn add_expense_comment(
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
fn delete_expense_comment(
    state: State<AppState>,
    group_id: String,
    comment_id: String,
) -> Result<Group, String> {
    api::delete_expense_comment(&state, &group_id, &comment_id)
}

/// Stops a repeated expense. The ones already added stay.
#[tauri::command]
#[specta::specta]
fn stop_recurring_expense(
    state: State<AppState>,
    group_id: String,
    recurring_id: String,
) -> Result<Group, String> {
    api::stop_recurring_expense(&state, &group_id, &recurring_id)
}

#[tauri::command]
#[specta::specta]
fn get_balances(
    state: State<AppState>,
    group_id: String,
) -> Result<Vec<ParticipantBalance>, String> {
    api::get_balances(&state, &group_id)
}

#[tauri::command]
#[specta::specta]
fn get_settlements(
    state: State<AppState>,
    group_id: String,
) -> Result<Vec<SettlementTransfer>, String> {
    api::get_settlements(&state, &group_id)
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
    api::record_reimbursement(&state, &group_id, from_id, to_id, amount_cents, notes)
}

#[tauri::command]
#[specta::specta]
fn get_storage_warnings(state: State<AppState>) -> Vec<String> {
    api::get_storage_warnings(&state)
}

#[tauri::command]
#[specta::specta]
fn get_sync_info(state: State<AppState>, group_id: String) -> Result<SyncInfo, String> {
    api::get_sync_info(&state, &group_id)
}

/// Syncs one group immediately. Failures are reported in the returned `last_error`.
#[tauri::command]
#[specta::specta]
async fn sync_now(state: State<'_, AppState>, group_id: String) -> Result<SyncInfo, String> {
    api::sync_now(&state, &group_id).await
}

#[tauri::command]
#[specta::specta]
async fn join_group(state: State<'_, AppState>, invite_code: String) -> Result<Group, String> {
    api::join_group(&state, &invite_code).await
}

#[tauri::command]
#[specta::specta]
fn get_account(state: State<AppState>) -> Result<Option<AccountInfo>, String> {
    api::get_account(&state)
}

#[tauri::command]
#[specta::specta]
async fn sign_up(
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
async fn recover_account(
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
async fn change_password(
    state: State<'_, AppState>,
    current_password: String,
    new_password: String,
) -> Result<(), String> {
    api::change_password(&state, &current_password, &new_password).await
}

/// A new recovery key, replacing the old one. Returned to show once.
#[tauri::command]
#[specta::specta]
async fn replace_recovery_key(
    state: State<'_, AppState>,
    password: String,
) -> Result<String, String> {
    api::replace_recovery_key(&state, &password).await
}

#[tauri::command]
#[specta::specta]
async fn log_in(
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
async fn create_login_link(
    state: State<'_, AppState>,
    password: String,
) -> Result<LoginLink, String> {
    api::create_login_link(&state, &password).await
}

/// Logs in with a link made by `create_login_link` on another device.
#[tauri::command]
#[specta::specta]
async fn log_in_with_link(state: State<'_, AppState>, link: String) -> Result<AccountInfo, String> {
    api::log_in_with_link(&state, &link).await
}

/// Removes the account and its groups from this device. Fails while changes are not uploaded,
/// unless `force` is set.
#[tauri::command]
#[specta::specta]
async fn log_out(state: State<'_, AppState>, force: bool) -> Result<(), String> {
    api::log_out(&state, force).await
}

/// Records which participant the user is in a group.
#[tauri::command]
#[specta::specta]
fn set_identity(
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
fn update_profile(
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
fn add_self(state: State<AppState>, group_id: String, name: String) -> Result<Group, String> {
    api::add_self(&state, &group_id, &name)
}

/// How hard a password is to guess. Signing up requires `acceptable`.
#[tauri::command]
#[specta::specta]
async fn password_strength(password: String, username: String) -> PasswordStrength {
    api::password_strength(&password, &username)
}

/// The link a device shows as a QR code to get something from a phone that scans it:
/// `purpose` is "login" to be logged into the phone's account, "group" to join a group.
#[tauri::command]
#[specta::specta]
fn receive_link(server_url: String, purpose: String) -> Result<String, String> {
    api::receive_link(&server_url, &purpose)
}

/// Asks once whether a phone scanned the code this device shows (`receive_link`). Nothing
/// while none did; then the device is logged in, or has joined the group.
#[tauri::command]
#[specta::specta]
async fn receive(state: State<'_, AppState>, link: String) -> Result<Option<Received>, String> {
    api::receive(&state, &link).await
}

/// Logs the device showing the scanned `link` into this account.
#[tauri::command]
#[specta::specta]
async fn send_login(
    state: State<'_, AppState>,
    link: String,
    password: String,
) -> Result<(), String> {
    api::send_login(&state, &link, &password).await
}

/// Lets the device showing the scanned `link` join a group.
#[tauri::command]
#[specta::specta]
async fn send_group_invite(
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
async fn send_feedback(
    state: State<'_, AppState>,
    message: String,
    contact: Option<String>,
    app: Option<String>,
) -> Result<(), String> {
    api::send_feedback(&state, &message, contact.as_deref(), app.as_deref()).await
}

/// What this platform can do natively, beyond the web view.
#[tauri::command]
#[specta::specta]
fn native_features() -> NativeFeatures {
    NativeFeatures {
        share: cfg!(target_os = "android"),
        scan: cfg!(mobile),
        save: cfg!(desktop),
    }
}

/// Saves `text` as a file named `file_name` in the Downloads folder, next to any file of that
/// name already there, and returns where it is. Only where `native_features().save` is true.
#[tauri::command]
#[specta::specta]
async fn save_download(
    app: tauri::AppHandle,
    file_name: String,
    text: String,
) -> Result<String, String> {
    save_in_downloads(&app, &file_name, text.as_bytes())
}

/// The same for a file that isn't text, such as a PDF.
#[tauri::command]
#[specta::specta]
async fn save_file(
    app: tauri::AppHandle,
    file_name: String,
    data: Vec<u8>,
) -> Result<String, String> {
    save_in_downloads(&app, &file_name, &data)
}

/// A file name as it is, when nothing in it may lead out of the folder it goes in.
fn plain_file_name(file_name: &str) -> Result<&std::ffi::OsStr, String> {
    std::path::Path::new(file_name)
        .file_name()
        .filter(|name| *name == std::ffi::OsStr::new(file_name))
        .ok_or_else(|| "This file name can't be used".to_string())
}

fn save_in_downloads(
    app: &tauri::AppHandle,
    file_name: &str,
    contents: &[u8],
) -> Result<String, String> {
    let name = plain_file_name(file_name)?;
    let folder = app
        .path()
        .download_dir()
        .map_err(|_| "Could not find the Downloads folder".to_string())?;
    let (stem, extension) = (
        std::path::Path::new(name).file_stem().unwrap_or(name),
        std::path::Path::new(name).extension(),
    );
    let mut path = folder.join(name);
    let mut copy = 2;
    while path.exists() {
        let mut numbered = stem.to_os_string();
        numbered.push(format!(" ({copy})"));
        if let Some(extension) = extension {
            numbered.push(".");
            numbered.push(extension);
        }
        path = folder.join(numbered);
        copy += 1;
    }
    std::fs::write(&path, contents).map_err(|e| format!("Could not save the file: {e}"))?;
    Ok(path.display().to_string())
}

/// Hands a file (a PDF, say) to the system share sheet, to send it or save it. Only where
/// `native_features().share` is true.
#[tauri::command]
#[specta::specta]
#[allow(unused_variables)]
async fn share_file(
    app: tauri::AppHandle,
    file_name: String,
    mime: String,
    data: Vec<u8>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        // Other apps read it through the app's file provider, which serves its cache folder.
        let folder = app
            .path()
            .app_cache_dir()
            .map_err(|_| "Could not find a folder for the file".to_string())?
            .join("shared");
        std::fs::create_dir_all(&folder).map_err(|e| format!("Could not save the file: {e}"))?;
        let path = folder.join(plain_file_name(&file_name)?);
        std::fs::write(&path, data).map_err(|e| format!("Could not save the file: {e}"))?;
        return share::share_file(&app, &path.display().to_string(), &mime, &file_name);
    }
    #[cfg(not(target_os = "android"))]
    Err("Sharing is not available on this device".to_string())
}

/// Opens the system share sheet with `text`. Only where `native_features().share` is true.
#[tauri::command]
#[specta::specta]
#[allow(unused_variables)]
async fn share_text(app: tauri::AppHandle, text: String, title: String) -> Result<(), String> {
    #[cfg(target_os = "android")]
    return share::share_text(&app, &text, &title);
    #[cfg(not(target_os = "android"))]
    Err("Sharing is not available on this device".to_string())
}

pub fn create_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        get_groups,
        get_group,
        create_group,
        import_group_csv,
        export_group_csv,
        suggest_exchange_rate,
        leave_group,
        delete_group,
        refuse_group_deletion,
        set_group_archived,
        update_group,
        add_participant,
        rename_participant,
        remove_participant,
        add_expense,
        update_expense,
        delete_expense,
        restore_expense,
        purge_expense,
        stop_recurring_expense,
        record_reimbursement,
        get_balances,
        get_settlements,
        get_storage_warnings,
        get_sync_info,
        sync_now,
        join_group,
        get_account,
        update_profile,
        add_expense_comment,
        delete_expense_comment,
        sign_up,
        log_in,
        log_out,
        set_identity,
        add_self,
        native_features,
        share_text,
        share_file,
        save_download,
        save_file,
        password_strength,
        send_feedback,
        receive_link,
        receive,
        send_login,
        send_group_invite,
        recover_account,
        change_password,
        replace_recovery_key,
        create_login_link,
        log_in_with_link
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

/// Icons `set_crisp_window_icons` gave each window, by (window, ICON_SMALL or ICON_BIG), so
/// they can be freed once replaced. Not the icon Tauri set first: Tauri frees that one. The
/// last ones live as long as their window; Windows frees them when the app exits.
#[cfg(windows)]
static CRISP_ICONS: Mutex<std::collections::BTreeMap<(usize, u32), usize>> =
    Mutex::new(std::collections::BTreeMap::new());

/// Windows draws the title bar and taskbar icons by scaling the one bitmap Tauri gives the
/// window, which blurs them on scaled displays. Load them from the .exe's icon instead: it
/// holds a version drawn for each size, and Windows picks the one for the window's DPI.
#[cfg(windows)]
fn set_crisp_window_icons(hwnd: *mut std::ffi::c_void) {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR,
        SM_CXICON, SM_CXSMICON, WM_SETICON,
    };
    // The resource id tauri-build gives the app icon (IDI_APPLICATION).
    const APP_ICON: usize = 32512;
    let mut ours = CRISP_ICONS.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: plain Win32 calls on this process's own module and a live window handle; a
    // missing resource makes LoadImageW return null, and the icon Tauri set stays. An icon is
    // destroyed only after the window switched to its replacement, and only if we loaded it.
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        let dpi = GetDpiForWindow(hwnd);
        for (kind, metric) in [(ICON_SMALL, SM_CXSMICON), (ICON_BIG, SM_CXICON)] {
            let size = GetSystemMetricsForDpi(metric, dpi);
            let icon = LoadImageW(
                module,
                APP_ICON as *const u16,
                IMAGE_ICON,
                size,
                size,
                LR_DEFAULTCOLOR,
            );
            if !icon.is_null() {
                SendMessageW(hwnd, WM_SETICON, kind as usize, icon as isize);
                if let Some(replaced) = ours.insert((hwnd as usize, kind), icon as usize) {
                    DestroyIcon(replaced as _);
                }
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = create_specta_builder();

    #[cfg(all(debug_assertions, not(mobile)))]
    export_bindings(&builder).expect("Failed to export typescript bindings");

    let mut app = tauri::Builder::default();
    // First, so a second launch (say, from an invite link) hands its link to this instance
    // and quits.
    #[cfg(desktop)]
    {
        app = app.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }));
    }
    app = app
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init());
    #[cfg(mobile)]
    {
        app = app.plugin(tauri_plugin_barcode_scanner::init());
    }
    #[cfg(target_os = "android")]
    {
        app = app.plugin(share::init()).plugin(notify::init());
    }

    #[cfg(windows)]
    {
        // Moved to a screen with another scale: load the icons drawn for that size.
        app = app.on_window_event(|window, event| {
            if let tauri::WindowEvent::ScaleFactorChanged { .. } = event {
                if let Ok(hwnd) = window.hwnd() {
                    set_crisp_window_icons(hwnd.0);
                }
            }
        });
    }

    app.setup(|app| {
        #[cfg(windows)]
        {
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(hwnd) = window.hwnd() {
                    set_crisp_window_icons(hwnd.0);
                }
            }
        }

        // Installers register the ezcount:// scheme; this covers portable copies. Not dev
        // builds: links would start a debug copy outside `tauri dev`, without its dev server,
        // and that copy would then hold the single instance. Pasting invites always works.
        #[cfg(all(any(windows, target_os = "linux"), not(debug_assertions)))]
        {
            use tauri_plugin_deep_link::DeepLinkExt;
            if let Err(e) = app.deep_link().register_all() {
                eprintln!("[deep-link] could not register ezcount://: {e}");
            }
        }

        let app_data = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("./"));
        let (store, warnings) = Store::open(&app_data.join("ezcount.sqlite3"))?;
        for warning in &warnings {
            eprintln!("[storage] {warning}");
        }
        app.manage(AppState::new(store, warnings)?);
        #[cfg(target_os = "android")]
        notify::started(app.handle().clone());
        background::spawn_sync(app.handle().clone());
        Ok(())
    })
    .invoke_handler(builder.invoke_handler())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
