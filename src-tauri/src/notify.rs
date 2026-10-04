//! Android notifications of what the other members did, shown when a sync brings changes
//! while the app isn't on screen. The Kotlin side is `NotifyPlugin.kt`, `Notifier.kt` and
//! `SyncWorker.kt` in the Android project.
//!
//! Two things sync a phone that nobody is looking at. While the app's process lives, the
//! background loop does, and hands what it found to the plugin (`post`). Once Android has
//! frozen or ended it, a periodic job (`SyncWorker`, every 15 minutes at best) calls
//! `backgroundSync` below: with the app still in the process it runs one pass of that same
//! loop, and without it it opens the data by itself for the time of one pass.

use ezcount_core::notices::Notice;
use ezcount_core::storage::Store;
use ezcount_core::{sync, AppState};
use jni::objects::{JObject, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use serde::Serialize;
use std::path::Path;
use std::sync::OnceLock;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{AppHandle, Manager, Wry};

/// The running app, when this process has one: a job started by Android alone doesn't.
static APP: OnceLock<AppHandle> = OnceLock::new();

pub struct Notify(PluginHandle<Wry>);

#[derive(Serialize)]
struct PostArgs {
    // The notices as JSON, which `Notifier.kt` reads.
    notices: String,
}

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("notify")
        .setup(|app, api| {
            let handle = api.register_android_plugin("com.ezvany.ezcount", "NotifyPlugin")?;
            app.manage(Notify(handle));
            Ok(())
        })
        .build()
}

/// Says the app is running in this process, with its data open.
pub fn started(app: AppHandle) {
    let _ = APP.set(app);
}

/// Shows notifications for what a sync found, unless the app is on screen.
pub fn post(app: &AppHandle, notices: &[Notice]) {
    if notices.is_empty() {
        return;
    }
    let Ok(notices) = serde_json::to_string(notices) else {
        return;
    };
    if let Err(e) = app
        .state::<Notify>()
        .0
        .run_mobile_plugin::<serde_json::Value>("post", PostArgs { notices })
    {
        eprintln!("[notify] could not show notifications: {e}");
    }
}

/// One sync pass with the data opened for it, for a process that has no app running.
fn sync_alone(data_dir: &str) -> Result<Vec<Notice>, String> {
    let dir = Path::new(data_dir);
    let (store, _) = Store::open(&dir.join("ezcount.sqlite3"), &dir.join("ezcount_data.json"))?;
    let state = AppState::new(store, Vec::new())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Could not start the sync: {e}"))?;
    Ok(runtime.block_on(sync::sync_all_noticing(&state, |_| {})))
}

/// `SyncWorker.backgroundSync`: syncs once and returns what the others did, as the JSON
/// `Notifier.kt` reads. `[]` when there is nothing, or when it failed.
#[no_mangle]
pub extern "system" fn Java_com_ezvany_ezcount_SyncWorker_backgroundSync<'local>(
    mut env: JNIEnv<'local>,
    _worker: JObject<'local>,
    data_dir: JString<'local>,
) -> jstring {
    let data_dir: Option<String> = env.get_string(&data_dir).ok().map(Into::into);
    // A panic must not cross into the Java side.
    let found = std::panic::catch_unwind(|| match (APP.get(), data_dir) {
        (Some(app), _) => Ok(tauri::async_runtime::block_on(
            crate::background::sync_once(app),
        )),
        (None, Some(dir)) => sync_alone(&dir),
        (None, None) => Err("No data folder".to_string()),
    });
    let notices = match found {
        Ok(Ok(notices)) => notices,
        Ok(Err(e)) => {
            eprintln!("[notify] background sync failed: {e}");
            Vec::new()
        }
        Err(_) => Vec::new(),
    };
    let json = serde_json::to_string(&notices).unwrap_or_else(|_| "[]".to_string());
    match env.new_string(json) {
        Ok(text) => text.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}
