//! Sharing text through Android's share sheet, so an invite can go out through any messaging
//! app. The Kotlin side is `SharePlugin.kt` in the Android project. Other platforms copy the
//! invite instead; `native_features` tells the frontend which to offer.

use serde::Serialize;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Wry};

pub struct Share(PluginHandle<Wry>);

#[derive(Serialize)]
struct ShareArgs<'a> {
    text: &'a str,
    title: &'a str,
}

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("share")
        .setup(|app, api| {
            let handle = api.register_android_plugin("com.ezvany.ezcount", "SharePlugin")?;
            app.manage(Share(handle));
            Ok(())
        })
        .build()
}

/// Opens the share sheet. Returns once it is shown, not once something was sent.
pub fn share_text(app: &tauri::AppHandle, text: &str, title: &str) -> Result<(), String> {
    app.state::<Share>()
        .0
        .run_mobile_plugin::<serde_json::Value>("shareText", ShareArgs { text, title })
        .map(|_| ())
        .map_err(|e| format!("Could not open the share menu: {e}"))
}
