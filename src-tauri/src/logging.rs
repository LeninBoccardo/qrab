use std::path::PathBuf;
use tauri::plugin::TauriPlugin;
use tauri::Wry;
use tauri_plugin_log::{Builder, Target, TargetKind};

/// Project-level `logs/` directory, resolved via `CARGO_MANIFEST_DIR` so logs
/// land alongside the source tree during manual testing. Distributed builds
/// should switch this to `tauri::Manager::path().app_log_dir()`.
pub fn logs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
        .join("logs")
}

/// Builds the `tauri-plugin-log` instance: stdout + webview + a fresh file per
/// app start (timestamp in the filename so back-to-back launches never collide).
/// If the logs folder can't be created we fall back to stdout/webview only.
pub fn build_plugin() -> TauriPlugin<Wry> {
    let dir = logs_dir();
    let dir_ready = std::fs::create_dir_all(&dir).is_ok();

    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let file_name = format!("qrab-{timestamp}");

    let mut targets: Vec<Target> = vec![Target::new(TargetKind::Webview)];
    // Stdout target is debug-only — release builds use `windows_subsystem
    // = "windows"`, so stdout writes to a dead handle. Avoid the per-log
    // syscall overhead in production.
    #[cfg(debug_assertions)]
    targets.push(Target::new(TargetKind::Stdout));
    if dir_ready {
        targets.push(Target::new(TargetKind::Folder {
            path: dir,
            file_name: Some(file_name),
        }));
    }

    Builder::default()
        .level(log::LevelFilter::Debug)
        .targets(targets)
        .build()
}
