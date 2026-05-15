use std::sync::Arc;

pub mod capture;
pub mod commands;
pub mod decoder;
pub mod error;
pub mod hotkey;
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
            hotkey::install_default(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
