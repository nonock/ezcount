//! What only the app can do, beyond the web view: saving and sharing files, and coloring
//! the phone's bars.

use ezcount_core::models::NativeFeatures;
use tauri::Manager;

/// What this platform can do natively, beyond the web view.
#[tauri::command]
#[specta::specta]
pub(crate) fn native_features() -> NativeFeatures {
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
pub(crate) async fn save_download(
    app: tauri::AppHandle,
    file_name: String,
    text: String,
) -> Result<String, String> {
    save_in_downloads(&app, &file_name, text.as_bytes())
}

/// The same for a file that isn't text, such as a PDF.
#[tauri::command]
#[specta::specta]
pub(crate) async fn save_file(
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
pub(crate) async fn share_file(
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
        return crate::share::share_file(&app, &path.display().to_string(), &mime, &file_name);
    }
    #[cfg(not(target_os = "android"))]
    Err("Sharing is not available on this device".to_string())
}

/// Colors the phone's status and navigation bars like the app (`#rrggbb`; `dark` for light
/// icons). Does nothing where the system draws no bars over the app.
#[tauri::command]
#[specta::specta]
#[allow(unused_variables)]
pub(crate) async fn set_bars_color(
    app: tauri::AppHandle,
    color: String,
    dark: bool,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    return crate::bars::set_color(&app, &color, dark);
    #[cfg(not(target_os = "android"))]
    Ok(())
}

/// Opens the system share sheet with `text`. Only where `native_features().share` is true.
#[tauri::command]
#[specta::specta]
#[allow(unused_variables)]
pub(crate) async fn share_text(
    app: tauri::AppHandle,
    text: String,
    title: String,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    return crate::share::share_text(&app, &text, &title);
    #[cfg(not(target_os = "android"))]
    Err("Sharing is not available on this device".to_string())
}

#[cfg(test)]
mod tests;
