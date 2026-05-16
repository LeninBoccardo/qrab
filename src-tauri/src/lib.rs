use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{Manager, WindowEvent};

pub mod capture;
pub mod commands;
pub mod decoder;
pub mod hotkey;
pub mod logging;
pub mod screenshot;
pub mod settings;
pub mod storage;
pub mod tray;
pub mod windows;

use capture::XcapCapturer;
use commands::{
    consume_pending_scan, copy_row, copy_rows_as_json, copy_to_clipboard,
    get_app_info, get_screenshot_monitor_png, get_screenshot_monitors,
    get_settings, hide_results_window, history_clear, history_delete,
    history_query, open_url, open_urls_bulk, scan_region, scan_screen,
    set_settings, AppState,
};
use decoder::RqrrDecoder;
use screenshot::ScreenshotStore;
use settings::SettingsStore;
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
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            scan_screen,
            scan_region,
            copy_to_clipboard,
            copy_row,
            copy_rows_as_json,
            open_url,
            open_urls_bulk,
            history_query,
            history_delete,
            history_clear,
            hide_results_window,
            consume_pending_scan,
            get_screenshot_monitors,
            get_screenshot_monitor_png,
            get_settings,
            set_settings,
            get_app_info
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

            let loaded_settings = settings::load_from_store(app.handle());

            let screenshots = ScreenshotStore::new();
            let state = AppState {
                capturer: Arc::new(XcapCapturer::new()),
                decoder: Arc::new(RqrrDecoder::new()),
                screenshots: screenshots.clone(),
                storage,
                pending_scan: Arc::new(AtomicBool::new(false)),
                settings: SettingsStore::new(loaded_settings.clone()),
            };
            app.manage(state);

            hotkey::register(app.handle(), &loaded_settings.hotkey);
            tray::install(app.handle())?;

            // Size the window proportionally to the primary monitor (60%
            // capped at 1200×800 logical) so the history table has room
            // to breathe on big displays without dwarfing small ones.
            // tauri.conf.json keeps a small default as the fallback when
            // monitor enumeration fails.
            if let Ok(Some(monitor)) = app.primary_monitor() {
                if let Some(window) = app.get_webview_window(windows::RESULTS_WINDOW) {
                    let scale = monitor.scale_factor();
                    let logical_w = monitor.size().width as f64 / scale;
                    let logical_h = monitor.size().height as f64 / scale;
                    let target_w = (logical_w * 0.6).clamp(520.0, 1200.0);
                    let target_h = (logical_h * 0.6).clamp(420.0, 800.0);
                    if let Err(e) =
                        window.set_size(tauri::LogicalSize::new(target_w, target_h))
                    {
                        log::warn!("could not resize results window: {e}");
                    }
                    let _ = window.center();
                    log::info!(
                        "results window sized to {:.0}x{:.0} (monitor {:.0}x{:.0} @ {}x)",
                        target_w,
                        target_h,
                        logical_w,
                        logical_h,
                        scale
                    );
                }
            }

            // In debug builds, surface the window on launch so dev iteration
            // doesn't require clicking the tray after every reload. Release
            // builds stay tray-only per the Raycast-style design (CLAUDE.md
            // §5: "App starts hidden; tray icon only").
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window(windows::RESULTS_WINDOW) {
                let _ = window.show();
                let _ = window.set_focus();
            }

            if let Some(window) = app.get_webview_window(windows::RESULTS_WINDOW) {
                let to_hide = window.clone();
                let screenshots_on_close = screenshots.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = to_hide.hide();
                        // CLAUDE.md §9: held screenshot is freed when the
                        // window closes. With one Tauri webview + hash
                        // routing, "the window closes" maps to this event.
                        // Reclaims ~24-64 MB immediately rather than
                        // waiting for the 60s TTL watcher to catch up.
                        screenshots_on_close.clear();
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
