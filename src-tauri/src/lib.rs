use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{Manager, WindowEvent};

pub mod capture;
pub mod commands;
pub mod decoder;
pub mod error;
pub mod hotkey;
pub mod logging;
pub mod screenshot;
pub mod storage;
pub mod tray;
pub mod windows;

use capture::XcapCapturer;
use commands::{
    consume_pending_scan, copy_to_clipboard, get_screenshot_monitor_png,
    get_screenshot_monitors, hide_results_window, history_clear,
    history_delete, history_query, mark_opened, open_url, open_urls_bulk,
    scan_region, scan_screen, AppState,
};
use decoder::RqrrDecoder;
use screenshot::ScreenshotStore;
use storage::Storage;

/// How often the TTL watcher wakes to check the held screenshot.
const SCREENSHOT_GC_INTERVAL: Duration = Duration::from_secs(10);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(logging::build_plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            scan_screen,
            scan_region,
            copy_to_clipboard,
            open_url,
            open_urls_bulk,
            mark_opened,
            history_query,
            history_delete,
            history_clear,
            hide_results_window,
            consume_pending_scan,
            get_screenshot_monitors,
            get_screenshot_monitor_png
        ])
        .setup(|app| {
            log::info!("qrab starting (logs dir: {})", logging::logs_dir().display());

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Open the on-disk SQLite store at app_data_dir/qrab.db,
            // creating the dir if first run.
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("resolving app_data_dir: {e}"))?;
            std::fs::create_dir_all(&app_data_dir)
                .map_err(|e| format!("creating app_data_dir: {e}"))?;
            log::info!("app data dir: {}", app_data_dir.display());
            let storage = Storage::open(app_data_dir.join("qrab.db"))
                .map_err(|e| format!("opening qrab.db: {e}"))?;

            let screenshots = ScreenshotStore::new();
            let state = AppState {
                capturer: Arc::new(XcapCapturer::new()),
                decoder: Arc::new(RqrrDecoder::new()),
                screenshots: screenshots.clone(),
                storage,
                pending_scan: Arc::new(AtomicBool::new(false)),
            };
            app.manage(state);

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
            // aged past `screenshot::TTL`.
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(SCREENSHOT_GC_INTERVAL).await;
                    screenshots.clear_if_expired(Instant::now());
                }
            });

            log::info!("qrab setup complete");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
