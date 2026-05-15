//! Helpers for showing/hiding the app's Tauri windows by label.
//!
//! qrab is a single-Tauri-window app: results / region select / history /
//! settings are all the same OS window served by different hash routes
//! in the SolidJS app. Only OS-window-level helpers belong here; route
//! changes happen in the webview (see `tray.rs::navigate`).

use tauri::{AppHandle, Manager, Runtime};

pub const RESULTS_WINDOW: &str = "results";

/// Show and focus the results window. No-op if the window isn't defined.
///
/// Order matters on Windows: a minimized window must be unminimized
/// *before* `show` so it doesn't reappear in a minimized state. `set_focus`
/// last to override OS focus-stealing prevention as best as we can.
pub fn show_results_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(RESULTS_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Hide the results window. No-op if the window isn't defined.
pub fn hide_results_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(RESULTS_WINDOW) {
        let _ = window.hide();
    }
}
