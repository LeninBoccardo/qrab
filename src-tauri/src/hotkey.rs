//! Global hotkey registration.
//!
//! Default binding is `CmdOrCtrl+Shift+Q` — Tauri's `CmdOrCtrl` modifier
//! resolves to Cmd on macOS and Ctrl elsewhere, matching the platform
//! convention in CLAUDE.md §13.
//!
//! Registration is best-effort: on Wayland (and some restricted Linux
//! sessions) global shortcuts may fail to register. We log and continue
//! so the rest of the app still works — a tray-click fallback lands in
//! C5 to keep the app usable without the hotkey.

use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const DEFAULT_HOTKEY: &str = "CmdOrCtrl+Shift+Q";
pub const SCAN_EVENT: &str = "qrab:scan";

/// Register the default hotkey. Failures are logged, not panicked on.
pub fn install_default<R: Runtime>(app: &AppHandle<R>) {
    let registered =
        app.global_shortcut()
            .on_shortcut(DEFAULT_HOTKEY, |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    trigger_scan(app);
                }
            });
    if let Err(e) = registered {
        eprintln!(
            "[qrab] could not register global shortcut '{DEFAULT_HOTKEY}': {e}. \
             Window UI and tray-click fallback still work."
        );
    }
}

/// Show the results window and emit the scan event the frontend listens
/// for. Shared by the hotkey handler and the tray "Scan now" entry.
pub fn trigger_scan<R: Runtime>(app: &AppHandle<R>) {
    crate::windows::show_results_window(app);
    if let Err(e) = app.emit(SCAN_EVENT, ()) {
        eprintln!("[qrab] failed to emit '{SCAN_EVENT}': {e}");
    }
}
