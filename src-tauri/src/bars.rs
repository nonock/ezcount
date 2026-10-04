//! The color Android shows behind its status and navigation bars: the app's own, so they
//! look part of it. The Kotlin side is `BarsPlugin.kt` in the Android project.

use serde::Serialize;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Wry};

pub struct Bars(PluginHandle<Wry>);

#[derive(Serialize)]
struct ColorArgs<'a> {
    color: &'a str,
    dark: bool,
}

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("bars")
        .setup(|app, api| {
            let handle = api.register_android_plugin("com.ezvany.ezcount", "BarsPlugin")?;
            app.manage(Bars(handle));
            Ok(())
        })
        .build()
}

/// `color` is `#rrggbb`; `dark` says the bars' icons must be light.
pub fn set_color(app: &tauri::AppHandle, color: &str, dark: bool) -> Result<(), String> {
    app.state::<Bars>()
        .0
        .run_mobile_plugin::<serde_json::Value>("setColor", ColorArgs { color, dark })
        .map(|_| ())
        .map_err(|e| format!("Could not color the system bars: {e}"))
}
