//! Global hotkey registration.
//!
//! Default binding is platform-aware. On macOS we deliberately avoid
//! `Cmd+Shift+Q` because that's the system Log Out shortcut — binding
//! qrab there triggered both actions at once. Mac default is
//! `Cmd+Shift+M` (matches the platform convention in CLAUDE.md §13).
//! Windows / Linux keep `Ctrl+Shift+Q`.
//!
//! Registration is best-effort: on Wayland (and some restricted Linux
//! sessions) global shortcuts may fail to register. We log and continue
//! so the rest of the app still works — the tray-click fallback still
//! triggers the same scan flow.

use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::windows::RESULTS_WINDOW;

#[cfg(target_os = "macos")]
pub const DEFAULT_HOTKEY: &str = "Cmd+Shift+M";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Q";

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
    log::info!("trigger_scan: invoked");
    crate::windows::show_results_window(app);

    // Make sure the webview is on #results before we fire the scan signal.
    // Without this, hotkey-while-parked-on-#history or #settings pops the
    // window forward but ResultsWindow isn't mounted to consume the scan,
    // and the user thinks the hotkey did nothing. Setting the hash to its
    // current value is a no-op, so this is safe to call unconditionally.
    if let Some(window) = app.get_webview_window(RESULTS_WINDOW) {
        if let Err(e) = window.eval("window.location.hash = 'results';") {
            log::warn!("failed to navigate to #results from hotkey: {e}");
        }
    }

    let state = app.state::<crate::commands::AppState>();
    state.pending_scan.store(true, Ordering::SeqCst);

    if let Err(e) = app.emit(SCAN_EVENT, ()) {
        log::warn!("failed to emit '{SCAN_EVENT}': {e}");
    }
}
