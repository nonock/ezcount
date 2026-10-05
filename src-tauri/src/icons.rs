//! Window icons on Windows, sharp at every display scale.

use std::sync::Mutex;

/// Icons `set_crisp_window_icons` gave each window, by (window, ICON_SMALL or ICON_BIG), so
/// they can be freed once replaced. Not the icon Tauri set first: Tauri frees that one. The
/// last ones live as long as their window; Windows frees them when the app exits.
static CRISP_ICONS: Mutex<std::collections::BTreeMap<(usize, u32), usize>> =
    Mutex::new(std::collections::BTreeMap::new());

/// Windows draws the title bar and taskbar icons by scaling the one bitmap Tauri gives the
/// window, which blurs them on scaled displays. Load them from the .exe's icon instead: it
/// holds a version drawn for each size, and Windows picks the one for the window's DPI.
pub(crate) fn set_crisp_window_icons(hwnd: *mut std::ffi::c_void) {
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
