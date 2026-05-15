use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{Manager, WindowEvent};

pub mod capture;
pub mod commands;
pub mod decoder;
pub mod error;
pub mod hotkey;
pub mod screenshot;
pub mod tray;
pub mod windows;

use capture::XcapCapturer;
use commands::{
    copy_to_clipboard, hide_results_window, open_url, scan_screen, AppState,
};
use decoder::RqrrDecoder;
use screenshot::ScreenshotStore;

/// How often the TTL watcher wakes to check the held screenshot.
const SCREENSHOT_GC_INTERVAL: Duration = Duration::from_secs(10);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let screenshots = ScreenshotStore::new();
    let state = AppState {
        capturer: Arc::new(XcapCapturer::new()),
        decoder: Arc::new(RqrrDecoder::new()),
        screenshots: screenshots.clone(),
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
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            hotkey::install_default(app.handle());
            tray::install(app.handle())?;

            if let Some(window) = app.get_webview_window(windows::RESULTS_WINDOW) {
                let to_hide = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = to_hide.hide();
                    }
                });
            }

            // TTL watcher: periodically drop the held screenshot if it has
            // aged past `screenshot::TTL`. Sleeping in a background task is
            // cheap and avoids piling timers on every scan.
            let gc_store = screenshots.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(SCREENSHOT_GC_INTERVAL).await;
                    gc_store.clear_if_expired(Instant::now());
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
