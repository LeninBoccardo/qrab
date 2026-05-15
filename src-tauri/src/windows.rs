//! Helpers for showing/hiding the app's Tauri windows by label.
//!
//! The results window is currently the only one defined in
//! `tauri.conf.json`; region/history/settings join in later phases and
//! gain their own constants and helpers here as they do.

use tauri::{AppHandle, Manager, Runtime};

pub const RESULTS_WINDOW: &str = "results";

/// Show and focus the results window. No-op if the window isn't defined.
pub fn show_results_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(RESULTS_WINDOW) {
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
