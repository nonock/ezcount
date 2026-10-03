//! ezcount in the browser: `ezcount-core` compiled to WebAssembly.
//!
//! `src/web/worker.ts` loads it in a dedicated Web Worker, because the storage (SQLite in the
//! origin's private file system, OPFS) only works there, and so that password hashing doesn't
//! freeze the page. It calls `start` once, then `invoke` for each command the page sends:
//! the same commands, with the same JSON arguments and results, as the Tauri app's.
#![cfg(target_family = "wasm")]

use std::cell::Cell;
use std::pin::pin;
use std::time::Duration;

use ezcount_core::storage::Store;
use ezcount_core::sync::{self, SyncEvent};
use ezcount_core::{api, AppState};
use futures_util::future::select;
use gloo_timers::future::sleep;
use sqlite_wasm_vfs::sahpool::{install as install_opfs, OpfsSAHPoolCfg};
use wasm_bindgen::prelude::*;

/// How often shared groups are synced in the background, besides right after local edits.
const POLL_INTERVAL: Duration = Duration::from_secs(20);
/// Short pause after a wake-up so a burst of edits goes out as one push.
const DEBOUNCE: Duration = Duration::from_millis(500);
/// The database file, in the OPFS pool.
const DATABASE: &str = "ezcount.sqlite3";

thread_local! {
    // Set once by `start` and kept for the worker's lifetime.
    static STATE: Cell<Option<&'static AppState>> = const { Cell::new(None) };
}

fn state() -> Result<&'static AppState, JsValue> {
    STATE
        .with(Cell::get)
        .ok_or_else(|| JsValue::from_str("ezcount has not started"))
}

/// Opens the database and starts background sync. `on_event(name, payload)` receives the
/// events the Tauri app emits (`sync-updated`, `account-updated`), the payload as JSON.
#[wasm_bindgen]
pub async fn start(on_event: js_sys::Function) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    if STATE.with(Cell::get).is_some() {
        return Ok(());
    }
    install_opfs::<sqlite_wasm_rs::WasmOsCallback>(&OpfsSAHPoolCfg::default(), true)
        .await
        .map_err(|e| JsValue::from_str(&format!("Could not open the browser's storage: {e}")))?;
    let (store, warnings) = Store::open_in_browser(DATABASE).map_err(|e| JsValue::from_str(&e))?;
    let state = AppState::new(store, warnings).map_err(|e| JsValue::from_str(&e))?;
    let state: &'static AppState = Box::leak(Box::new(state));
    STATE.with(|s| s.set(Some(state)));
    wasm_bindgen_futures::spawn_local(background_sync(state, on_event));
    Ok(())
}

/// Runs a command, as Tauri's IPC would: JSON arguments in, JSON result out. An error
/// rejects with the message to show.
#[wasm_bindgen]
pub async fn invoke(command: String, args: String) -> Result<String, JsValue> {
    api::invoke(state()?, &command, &args)
        .await
        .map_err(|e| JsValue::from_str(&e))
}

/// The Tauri app's background loop (`src-tauri/src/background.rs`), on browser timers.
async fn background_sync(state: &'static AppState, on_event: js_sys::Function) {
    let emit = |name: &str, payload: String| {
        let _ = on_event.call2(
            &JsValue::NULL,
            &JsValue::from_str(name),
            &JsValue::from_str(&payload),
        );
    };
    loop {
        select(
            pin!(state.sync_wakeup.notified()),
            pin!(sleep(POLL_INTERVAL)),
        )
        .await;
        sleep(DEBOUNCE).await;
        sync::sync_all(state, |event| match event {
            SyncEvent::Account => emit("account-updated", "null".to_string()),
            SyncEvent::Group { group_id, changed } => emit(
                "sync-updated",
                serde_json::json!({ "group_id": group_id, "changed": changed }).to_string(),
            ),
        })
        .await;
    }
}
