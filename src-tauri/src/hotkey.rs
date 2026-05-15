//! Global hotkey registration.
//!
//! Default binding is `CmdOrCtrl+Shift+Q` — Tauri's `CmdOrCtrl` modifier
//! resolves to Cmd on macOS and Ctrl elsewhere, matching the platform
//! convention in CLAUDE.md §13.
//!
//! Registration is best-effort: on Wayland (and some restricted Linux
//! sessions) global shortcuts may fail to register. We log and continue
//! so the rest of the app still works — the tray-click fallback still
//! triggers the same scan flow.

use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const DEFAULT_HOTKEY: &str = "CmdOrCtrl+Shift+Q";
pub const SCAN_EVENT: &str = "qrab:scan";

/// Register `hotkey` as the global scan shortcut. Any previously-registered
/// shortcut is removed first, so this also serves as the re-register entry
/// point when the user rebinds in settings.
///
/// Returns `true` on success. Failures are logged and the function returns
/// `false` — callers should not treat a failed registration as fatal.
pub fn register<R: Runtime>(app: &AppHandle<R>, hotkey: &str) -> bool {
    let shortcut = app.global_shortcut();
    let _ = shortcut.unregister_all();
    match shortcut.on_shortcut(hotkey, |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            trigger_scan(app);
        }
    }) {
        Ok(()) => {
            log::info!("registered global shortcut '{hotkey}'");
            true
        }
        Err(e) => {
            log::warn!(
                "could not register global shortcut '{hotkey}': {e}. \
                 Window UI and tray-click fallback still work."
            );
            false
        }
    }
}

/// Show the results window and ask the frontend to start a scan. Used by
/// both the hotkey handler and the tray "Scan now" entry.
///
/// Two paths reach the frontend so neither cold-start nor steady-state
/// drops the request:
///   1. A `pending_scan` flag in Tauri state, consumed by the frontend on
///      mount. Catches the cold case where the JS listener isn't attached
///      yet (first hotkey press after launch, WebView2 still warming up).
///   2. A `qrab:scan` event emit. Catches the warm case where the listener
///      is alive and we want an immediate trigger without polling.
pub fn trigger_scan<R: Runtime>(app: &AppHandle<R>) {
    crate::windows::show_results_window(app);

    let state = app.state::<crate::commands::AppState>();
    state.pending_scan.store(true, Ordering::SeqCst);

    if let Err(e) = app.emit(SCAN_EVENT, ()) {
        log::warn!("failed to emit '{SCAN_EVENT}': {e}");
    }
}
