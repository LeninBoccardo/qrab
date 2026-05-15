use std::sync::Arc;
use tauri::{Manager, WindowEvent};

pub mod capture;
pub mod commands;
pub mod decoder;
pub mod error;
pub mod hotkey;
pub mod tray;
pub mod windows;

use capture::XcapCapturer;
use commands::{
    copy_to_clipboard, hide_results_window, open_url, scan_screen, AppState,
};
use decoder::RqrrDecoder;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState {
        capturer: Arc::new(XcapCapturer::new()),
        decoder: Arc::new(RqrrDecoder::new()),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            scan_screen,
            copy_to_clipboard,
            open_url,
            hide_results_window
        ])
        .setup(|app| {
            // macOS: no dock icon — the tray is the only persistent surface.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            hotkey::install_default(app.handle());
            tray::install(app.handle())?;

            // The X button on the results window should hide, not close.
            // Closing would terminate the only webview and (effectively)
            // the app; we want the tray to keep it running.
            if let Some(window) = app.get_webview_window(windows::RESULTS_WINDOW) {
                let to_hide = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = to_hide.hide();
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
