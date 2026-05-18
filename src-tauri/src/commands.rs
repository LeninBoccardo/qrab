//! `#[tauri::command]` functions — the IPC surface.
//!
//! Commands return `Result<T, String>` to keep the boundary stable; the
//! underlying typed errors live in each module (`CaptureError`,
//! `DecodeError`, `rusqlite::Error`). Heavy work (capture, decode, every
//! SQLite touch) runs inside `tokio::task::spawn_blocking` so the async
//! runtime stays responsive.

use crate::capture::{Capturer, MonitorImage};
use crate::decoder::{classify_kind, Decoder, QrKind};
use crate::screenshot::{HeldScreenshot, ScreenshotStore};
use crate::settings::{Settings, SettingsStore};
use crate::storage::queries::{
    delete_all, delete_by_id, delete_by_ids, get_by_id, get_by_ids,
    history_query as history_query_db, insert_batch, mark_copied as mark_copied_db,
    mark_copied_many, mark_opened as mark_opened_db, mark_opened_many, HistoryFilter, NewScanRow,
    ScanRow,
};
use crate::storage::Storage;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::ipc::Response;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;
use ulid::Ulid;

/// Tauri state container.
#[derive(Clone)]
pub struct AppState {
    pub capturer: Arc<dyn Capturer>,
    pub decoder: Arc<dyn Decoder>,
    pub screenshots: ScreenshotStore,
    pub storage: Storage,
    /// Set by the hotkey/tray when they want a scan; consumed by the
    /// frontend on mount so a hotkey that fires *before* the JS listener
    /// is attached (cold WebView2 on first open) still triggers a scan.
    pub pending_scan: Arc<AtomicBool>,
    /// User-facing settings (CLAUDE.md §9). Persisted via
    /// `tauri-plugin-store`; hotkey + autostart side effects applied on
    /// `set_settings`.
    pub settings: SettingsStore,
    /// Outcome of the most recent `hotkey::register` call. `false` means
    /// the OS rejected the binding (Wayland restriction, conflict with
    /// another app, or invalid accelerator). Read by the Settings UI to
    /// surface a visible warning to the user.
    pub hotkey_registered: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub rows: Vec<ScanRow>,
    /// Opaque handle the frontend echoes back to `scan_region`.
    pub screenshot_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionBounds {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub monitor_index: usize,
}

/// Capture every monitor, decode, persist matches to the database, hold
/// the screenshot for region-select, and return the rows with real IDs.
#[tauri::command]
pub async fn scan_screen(state: State<'_, AppState>) -> Result<ScanResult, String> {
    log::info!("scan_screen: start");
    let capturer = state.capturer.clone();
    let decoder = state.decoder.clone();
    let storage = state.storage.clone();
    let store = state.screenshots.clone();

    let monitors: Vec<MonitorImage> = tokio::task::spawn_blocking(move || capturer.capture_all())
        .await
        .map_err(|e| format!("capture task panicked: {e}"))?
        .map_err(|e| {
            log::warn!("scan_screen: capture failed: {e}");
            e.to_string()
        })?;
    log::info!("scan_screen: captured {} monitor(s)", monitors.len());

    let scanned_at = current_epoch_ms();
    let screenshot_id = Ulid::new().to_string();
    let batch_id = Ulid::new().to_string();
    let batch_log = batch_id.clone();

    // Decode + persist on the blocking pool — SQLite writes are sync and
    // were previously running on the async runtime, holding a worker for
    // the duration of the transaction. Returning `monitors` from the
    // closure lets us still feed them to the screenshot store afterward.
    let (rows, monitors) = tokio::task::spawn_blocking(
        move || -> Result<(Vec<ScanRow>, Vec<MonitorImage>), String> {
            let mut rows = decode_monitors(decoder.as_ref(), &monitors, &batch_id, scanned_at);
            persist_rows(&storage, &mut rows)?;
            Ok((rows, monitors))
        },
    )
    .await
    .map_err(|e| format!("decode task panicked: {e}"))??;

    store.put(HeldScreenshot::new(
        screenshot_id.clone(),
        monitors,
        Instant::now(),
    ));

    log::info!(
        "scan_screen: decoded {} code(s), batch={}, screenshot={}",
        rows.len(),
        batch_log,
        screenshot_id
    );
    Ok(ScanResult {
        rows,
        screenshot_id,
    })
}

/// Sentinel monitor index for rows whose image came from a file instead
/// of a screen capture. The schema requires NOT NULL, so we use -1
/// rather than introducing a nullable column and a migration.
const FILE_SOURCE_MONITOR_INDEX: i64 = -1;

/// Hard cap on the encoded size of a file accepted by `decode_image_file`.
/// 50 MB comfortably fits any real-world QR-bearing image (a 4K JPEG is
/// ~5 MB; a 4K PNG ~10–20 MB) while shutting down accidental drops of
/// huge raw / multi-layer files that would balloon decoded RAM.
const MAX_INPUT_FILE_BYTES: u64 = 50 * 1024 * 1024;

/// Hard cap on decoded pixel count. A 4K image is ~8.3 MP; 80 MP allows
/// for ~8K and stitched panoramas while rejecting decompression-bomb
/// PNGs (e.g. 100000×100000 transparent canvases) that would otherwise
/// allocate tens of GB on the way to to_rgba8.
const MAX_DECODED_PIXELS: u64 = 80_000_000;

/// Image file extensions accepted by `decode_image_file`. Single source
/// of truth — exposed to the frontend via `get_supported_image_extensions`
/// so the file-picker filter and drag-drop allow-list can't drift from
/// what the backend actually supports. Adding a new format here requires
/// the matching feature in the `image` crate (see Cargo.toml).
const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];

/// Returns the lowercase extensions [`decode_image_file`] accepts (without
/// leading dot). Frontend uses this to populate the open-file dialog and
/// to filter drag-and-drop payloads before invoking the decode IPC.
#[tauri::command]
pub async fn get_supported_image_extensions() -> Result<Vec<String>, String> {
    Ok(SUPPORTED_IMAGE_EXTENSIONS
        .iter()
        .map(|s| (*s).to_string())
        .collect())
}

/// Pure check on encoded file size. Factored out so the cap is unit-testable
/// without writing a real file to disk.
fn check_input_file_size(len: u64) -> Result<(), String> {
    if len > MAX_INPUT_FILE_BYTES {
        Err(format!(
            "image too large: {len} bytes (max {MAX_INPUT_FILE_BYTES})"
        ))
    } else {
        Ok(())
    }
}

/// Pure check on the decoded pixel count.
fn check_decoded_pixels(w: u32, h: u32) -> Result<(), String> {
    let pixels = u64::from(w) * u64::from(h);
    if pixels > MAX_DECODED_PIXELS {
        Err(format!(
            "image dimensions too large: {w}x{h} ({pixels} pixels, max {MAX_DECODED_PIXELS})"
        ))
    } else {
        Ok(())
    }
}

/// Decode a QR code from an image file on disk. Same persistence path as
/// `scan_screen`, but the held-screenshot store is left untouched —
/// region select doesn't apply, so the returned `screenshot_id` is empty.
/// Supports PNG, JPEG, and WebP (the image crate features wired in
/// Cargo.toml).
#[tauri::command]
pub async fn decode_image_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<ScanResult, String> {
    log::info!("decode_image_file: path={path}");
    let decoder = state.decoder.clone();
    let storage = state.storage.clone();
    let scanned_at = current_epoch_ms();
    let batch_id = Ulid::new().to_string();
    let batch_log = batch_id.clone();

    let rows: Vec<ScanRow> =
        tokio::task::spawn_blocking(move || -> Result<Vec<ScanRow>, String> {
            // 1. Reject huge encoded files outright. Bounded source ⇒ bounded
            //    decode RAM regardless of whatever ratio the format implies.
            let meta =
                std::fs::metadata(&path).map_err(|e| format!("could not stat '{path}': {e}"))?;
            check_input_file_size(meta.len())?;

            // 2. Read just the format header to learn the decoded dimensions
            //    before we ask the image crate to allocate the RGBA buffer.
            //    Catches decompression-bomb PNGs that the file-size cap can't.
            let reader = image::ImageReader::open(&path)
                .map_err(|e| format!("could not open '{path}': {e}"))?
                .with_guessed_format()
                .map_err(|e| format!("could not detect format for '{path}': {e}"))?;
            let (w, h) = reader
                .into_dimensions()
                .map_err(|e| format!("could not read dimensions for '{path}': {e}"))?;
            check_decoded_pixels(w, h)?;

            // 3. Now the full decode is safe — bounded by both caps above.
            let img = image::open(&path)
                .map_err(|e| format!("could not open '{path}': {e}"))?
                .to_rgba8();
            let started = Instant::now();
            let decoded = decoder.decode(&img);
            log::info!(
                "decode_image_file: {}x{} found={} took={}ms",
                img.width(),
                img.height(),
                decoded.len(),
                started.elapsed().as_millis()
            );
            let mut seen: HashSet<String> = HashSet::new();
            let mut rows = Vec::new();
            for content in decoded {
                if !seen.insert(content.clone()) {
                    continue;
                }
                let kind = classify_kind(&content);
                rows.push(ScanRow {
                    id: -1,
                    batch_id: batch_id.clone(),
                    content,
                    kind,
                    monitor_index: FILE_SOURCE_MONITOR_INDEX,
                    scanned_at,
                    opened: false,
                    opened_at: None,
                    copied: false,
                    copied_at: None,
                });
            }
            persist_rows(&storage, &mut rows)?;
            Ok(rows)
        })
        .await
        .map_err(|e| format!("file decode task panicked: {e}"))??;

    log::info!(
        "decode_image_file: decoded {} code(s), batch={}",
        rows.len(),
        batch_log
    );
    // No screenshot held — region select doesn't apply to file scans.
    Ok(ScanResult {
        rows,
        screenshot_id: String::new(),
    })
}

/// Decode a sub-rectangle of a previously-held screenshot.
///
/// The frontend passes the `screenshot_id` it received from `scan_screen`
/// together with the user's rubber-banded `bounds` (in image pixels of
/// the chosen monitor). The held image is cropped and the decoder runs
/// on the crop only.
#[tauri::command]
pub async fn scan_region(
    state: State<'_, AppState>,
    screenshot_id: String,
    bounds: RegionBounds,
) -> Result<ScanResult, String> {
    log::info!(
        "scan_region: screenshot={} monitor={} bounds={}x{}+{}+{}",
        screenshot_id,
        bounds.monitor_index,
        bounds.w,
        bounds.h,
        bounds.x,
        bounds.y
    );
    let held = state
        .screenshots
        .get_if_id(&screenshot_id)
        .ok_or_else(|| "screenshot not found or expired".to_string())?;
    let decoder = state.decoder.clone();
    let storage = state.storage.clone();
    let scanned_at = current_epoch_ms();
    let batch_id = Ulid::new().to_string();
    let batch_log = batch_id.clone();

    // Crop + decode + persist all happen on the blocking pool so the
    // SQLite insert doesn't park the async runtime.
    let rows: Vec<ScanRow> =
        tokio::task::spawn_blocking(move || -> Result<Vec<ScanRow>, String> {
            let monitor = held
                .monitors
                .iter()
                .find(|m| m.index == bounds.monitor_index)
                .ok_or_else(|| {
                    format!("monitor index {} not in screenshot", bounds.monitor_index)
                })?;
            let mut rows = decode_region(
                decoder.as_ref(),
                &monitor.image,
                &bounds,
                &batch_id,
                scanned_at,
            )?;
            persist_rows(&storage, &mut rows)?;
            Ok(rows)
        })
        .await
        .map_err(|e| format!("region decode task panicked: {e}"))??;

    log::info!(
        "scan_region: decoded {} code(s), batch={}",
        rows.len(),
        batch_log
    );
    Ok(ScanResult {
        rows,
        screenshot_id,
    })
}

/// Insert any rows where `id == -1` (i.e. freshly decoded) and overwrite
/// their `id` with the DB-assigned one. Empty input is a no-op.
fn persist_rows(storage: &Storage, rows: &mut [ScanRow]) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }
    let new_rows: Vec<NewScanRow<'_>> = rows
        .iter()
        .map(|r| NewScanRow {
            batch_id: &r.batch_id,
            content: &r.content,
            kind: r.kind,
            monitor_index: r.monitor_index,
            scanned_at: r.scanned_at,
        })
        .collect();
    let ids = insert_batch(storage, &new_rows).map_err(|e| format!("storage: {e}"))?;
    for (row, id) in rows.iter_mut().zip(ids.iter()) {
        row.id = *id;
    }
    Ok(())
}

/// Crop `image` to `bounds` and decode the crop. Validates bounds against
/// the source image so an out-of-range region surfaces as a typed error
/// rather than a panic from `crop_imm`.
fn decode_region<D: Decoder + ?Sized>(
    decoder: &D,
    image: &image::RgbaImage,
    bounds: &RegionBounds,
    batch_id: &str,
    scanned_at: i64,
) -> Result<Vec<ScanRow>, String> {
    if bounds.w == 0 || bounds.h == 0 {
        return Err("region must have non-zero width and height".into());
    }
    let (img_w, img_h) = image.dimensions();
    if bounds.x.saturating_add(bounds.w) > img_w || bounds.y.saturating_add(bounds.h) > img_h {
        return Err(format!(
            "bounds out of image (image {img_w}x{img_h}, region {}+{}, {}+{})",
            bounds.x, bounds.w, bounds.y, bounds.h
        ));
    }

    let crop = image::imageops::crop_imm(image, bounds.x, bounds.y, bounds.w, bounds.h).to_image();

    let mut seen: HashSet<String> = HashSet::new();
    let mut rows = Vec::new();
    for content in decoder.decode(&crop) {
        if !seen.insert(content.clone()) {
            continue;
        }
        let kind = classify_kind(&content);
        rows.push(ScanRow {
            id: -1,
            batch_id: batch_id.to_string(),
            content,
            kind,
            monitor_index: bounds.monitor_index as i64,
            scanned_at,
            opened: false,
            opened_at: None,
            copied: false,
            copied_at: None,
        });
    }
    Ok(rows)
}

/// Write `text` to the system clipboard. Generic helper for callers that
/// don't have a row id — row-based copy paths use [`copy_row`] instead so
/// the row gets stamped as copied atomically.
#[tauri::command]
pub async fn copy_to_clipboard(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

/// Copy a stored row's content to the clipboard and stamp the row as
/// copied. Atomic on the Rust side so the frontend never has to glue a
/// `copy_to_clipboard` + `mark_copied` pair (which could race).
#[tauri::command]
pub async fn copy_row(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let storage = state.storage.clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let row = get_by_id(&storage, id)
            .map_err(|e| format!("storage: {e}"))?
            .ok_or_else(|| format!("row {id} not found"))?;
        let kind = row.kind;
        app.clipboard()
            .write_text(row.content)
            .map_err(|e| e.to_string())?;
        mark_copied_db(&storage, id, current_epoch_ms()).map_err(|e| format!("storage: {e}"))?;
        log::info!("copy_row: id={id} kind={kind:?}");
        Ok(())
    })
    .await
    .map_err(|e| format!("copy task panicked: {e}"))?
}

/// Bulk copy: serialize `ids` rows to JSON, write the JSON once to the
/// clipboard, and stamp every row as copied. Returns the number of rows
/// successfully copied. Bulk-action surface for the History window
/// (CLAUDE.md §10 "copy as JSON").
#[tauri::command]
pub async fn copy_rows_as_json(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<i64>,
) -> Result<usize, String> {
    let storage = state.storage.clone();
    tokio::task::spawn_blocking(move || -> Result<usize, String> {
        if ids.is_empty() {
            return Ok(0);
        }
        let rows = get_by_ids(&storage, &ids).map_err(|e| format!("storage: {e}"))?;
        if rows.len() != ids.len() {
            // Surface the first missing id so the UI can show a useful error
            // — happens if rows were deleted between selection and copy.
            let found: HashSet<i64> = rows.iter().map(|r| r.id).collect();
            let missing = ids.iter().find(|id| !found.contains(id));
            if let Some(id) = missing {
                return Err(format!("row {id} not found"));
            }
        }
        let json = serde_json::to_string_pretty(&rows).map_err(|e| format!("serialize: {e}"))?;
        app.clipboard()
            .write_text(json)
            .map_err(|e| e.to_string())?;
        if let Err(e) = mark_copied_many(&storage, &ids, current_epoch_ms()) {
            log::warn!("mark_copied_many failed: {e}");
        }
        Ok(rows.len())
    })
    .await
    .map_err(|e| format!("copy task panicked: {e}"))?
}

/// Open a stored row's URL in the user's default browser and mark the
/// row as opened. The frontend passes the row id rather than the URL so
/// the open-and-mark pair is atomic on the Rust side.
#[tauri::command]
pub async fn open_url(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let storage = state.storage.clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let row = get_by_id(&storage, id)
            .map_err(|e| format!("storage: {e}"))?
            .ok_or_else(|| format!("row {id} not found"))?;
        if row.kind != QrKind::Url {
            return Err(format!("row {id} is not a URL ({:?})", row.kind));
        }
        app.opener()
            .open_url(&row.content, None::<&str>)
            .map_err(|e| e.to_string())?;
        mark_opened_db(&storage, id, current_epoch_ms()).map_err(|e| format!("storage: {e}"))?;
        log::info!("open_url: id={id}");
        Ok(())
    })
    .await
    .map_err(|e| format!("open task panicked: {e}"))?
}

/// Run a paginated history query.
#[tauri::command]
pub async fn history_query(
    state: State<'_, AppState>,
    filter: HistoryFilter,
) -> Result<Vec<ScanRow>, String> {
    let storage = state.storage.clone();
    tokio::task::spawn_blocking(move || history_query_db(&storage, &filter))
        .await
        .map_err(|e| format!("history task panicked: {e}"))?
        .map_err(|e| format!("storage: {e}"))
}

/// Delete one row from history.
#[tauri::command]
pub async fn history_delete(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let storage = state.storage.clone();
    tokio::task::spawn_blocking(move || delete_by_id(&storage, id))
        .await
        .map_err(|e| format!("history task panicked: {e}"))?
        .map_err(|e| format!("storage: {e}"))?;
    Ok(())
}

/// Delete many rows in one statement. Replaces an N-roundtrip frontend loop
/// when the user multi-selects rows in the History window.
#[tauri::command]
pub async fn history_delete_bulk(
    state: State<'_, AppState>,
    ids: Vec<i64>,
) -> Result<usize, String> {
    if ids.is_empty() {
        return Ok(0);
    }
    let storage = state.storage.clone();
    tokio::task::spawn_blocking(move || delete_by_ids(&storage, &ids))
        .await
        .map_err(|e| format!("history task panicked: {e}"))?
        .map_err(|e| format!("storage: {e}"))
}

/// Wipe the whole history table.
#[tauri::command]
pub async fn history_clear(state: State<'_, AppState>) -> Result<(), String> {
    let storage = state.storage.clone();
    tokio::task::spawn_blocking(move || delete_all(&storage))
        .await
        .map_err(|e| format!("history task panicked: {e}"))?
        .map_err(|e| format!("storage: {e}"))?;
    Ok(())
}

/// Above this count the frontend must surface ConfirmOpenAll first
/// (CLAUDE.md §10). The Rust side double-checks via `open_urls_bulk` so
/// a buggy or compromised frontend can't tab-bomb the user.
pub const BULK_OPEN_CONFIRM_THRESHOLD: usize = 3;

/// Hard cap on persisted hotkey strings. Real accelerators are well under
/// 30 chars (`CmdOrCtrl+Shift+Q` = 17); the cap is defense against a
/// malformed `set_settings` payload trying to wedge megabytes into the
/// settings store.
const MAX_HOTKEY_LEN: usize = 64;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkOpenResult {
    pub opened: Vec<i64>,
    pub failed: Vec<BulkOpenFailure>,
    pub skipped_non_url: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkOpenFailure {
    pub id: i64,
    pub error: String,
}

/// Open every URL row in `ids` and mark each opened. Non-URL rows are
/// silently skipped (counted in `skipped_non_url`). If the URL count
/// exceeds [`BULK_OPEN_CONFIRM_THRESHOLD`] and `confirmed` is false,
/// returns an error — the frontend must show ConfirmOpenAll first.
#[tauri::command]
pub async fn open_urls_bulk(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<i64>,
    confirmed: bool,
) -> Result<BulkOpenResult, String> {
    let storage = state.storage.clone();
    tokio::task::spawn_blocking(move || -> Result<BulkOpenResult, String> {
        // Resolve rows up front so the count check uses URLs only — non-URL
        // rows in the selection don't tab-bomb the threshold.
        let rows = get_by_ids(&storage, &ids).map_err(|e| format!("storage: {e}"))?;
        if rows.len() != ids.len() {
            let found: HashSet<i64> = rows.iter().map(|r| r.id).collect();
            let missing = ids.iter().find(|id| !found.contains(id));
            if let Some(id) = missing {
                return Err(format!("row {id} not found"));
            }
        }
        let mut url_rows: Vec<(i64, String)> = Vec::with_capacity(rows.len());
        let mut skipped_non_url = 0usize;
        for row in rows {
            if row.kind == QrKind::Url {
                url_rows.push((row.id, row.content));
            } else {
                skipped_non_url += 1;
            }
        }

        if url_rows.len() > BULK_OPEN_CONFIRM_THRESHOLD && !confirmed {
            return Err(format!(
                "Confirmation required for opening more than {} URLs",
                BULK_OPEN_CONFIRM_THRESHOLD
            ));
        }

        let opener = app.opener();
        let mut opened = Vec::new();
        let mut failed = Vec::new();

        for (id, url) in url_rows {
            match opener.open_url(&url, None::<&str>) {
                Ok(()) => opened.push(id),
                Err(e) => failed.push(BulkOpenFailure {
                    id,
                    error: e.to_string(),
                }),
            }
        }

        if !opened.is_empty() {
            if let Err(e) = mark_opened_many(&storage, &opened, current_epoch_ms()) {
                log::warn!("mark_opened_many failed: {e}");
            }
        }

        Ok(BulkOpenResult {
            opened,
            failed,
            skipped_non_url,
        })
    })
    .await
    .map_err(|e| format!("open task panicked: {e}"))?
}

/// Hide the results window. Bound to Esc and the titlebar Close button.
#[tauri::command]
pub async fn hide_results_window(app: AppHandle) -> Result<(), String> {
    crate::windows::hide_results_window(&app);
    Ok(())
}

/// Open the macOS Screen Recording privacy pane via the system URL
/// scheme. No-op on Windows / Linux — those don't need a Screen Recording
/// permission step. Bound to the actionable button on the results-window
/// banner that appears when capture fails with PermissionDenied.
#[tauri::command]
pub async fn open_screen_recording_prefs(
    #[allow(unused_variables)] app: AppHandle,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        app.opener()
            .open_url(
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
                None::<&str>,
            )
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Atomically clear and return the pending-scan flag. The frontend calls
/// this on mount so a hotkey that fired before the listener attached
/// still produces a scan.
#[tauri::command]
pub async fn consume_pending_scan(state: State<'_, AppState>) -> Result<bool, String> {
    let pending = state.pending_scan.swap(false, Ordering::SeqCst);
    if pending {
        log::info!("consume_pending_scan: returning true (frontend will scan)");
    }
    Ok(pending)
}

/// Return the current user settings.
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.settings.get())
}

/// Return a fresh `Settings::default()`. Lets the frontend offer a
/// "Reset to defaults" action without hardcoding the defaults — Rust
/// stays the source of truth, including the platform-aware hotkey.
#[tauri::command]
pub async fn get_default_settings() -> Result<Settings, String> {
    Ok(Settings::default())
}

/// Current hotkey binding + whether the OS accepted its registration.
/// Surfaced in the Settings UI so the user sees a visible warning when
/// the chord could not be bound (Wayland, conflict, invalid combo).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
    pub binding: String,
    pub registered: bool,
}

#[tauri::command]
pub async fn get_hotkey_status(state: State<'_, AppState>) -> Result<HotkeyStatus, String> {
    Ok(HotkeyStatus {
        binding: state.settings.get().hotkey,
        registered: state.hotkey_registered.load(Ordering::SeqCst),
    })
}

/// Static app metadata sourced from Cargo at compile time — name,
/// version, author, description. Used by the About section in #config.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub author: &'static str,
    pub description: &'static str,
}

/// Manual update check — hits `api.github.com` per the CLAUDE.md §5
/// carve-out. Returns the latest release tag and a flag indicating
/// whether it's newer than the running version.
#[tauri::command]
pub async fn check_for_updates() -> Result<crate::update::UpdateStatus, String> {
    log::info!("check_for_updates: invoked");
    let status = crate::update::check_for_updates().await?;
    log::info!(
        "check_for_updates: current={} latest={:?} has_update={}",
        status.current_version,
        status.latest_version,
        status.has_update
    );
    Ok(status)
}

#[tauri::command]
pub async fn get_app_info() -> Result<AppInfo, String> {
    Ok(AppInfo {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        author: env!("CARGO_PKG_AUTHORS"),
        description: env!("CARGO_PKG_DESCRIPTION"),
    })
}

/// Replace the user settings. Persists to disk and applies side effects:
/// re-registers the global hotkey if it changed, and enables/disables the
/// OS autostart entry if that toggle changed.
#[tauri::command]
pub async fn set_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<(), String> {
    if settings.hotkey.len() > MAX_HOTKEY_LEN {
        return Err(format!(
            "hotkey string too long (max {MAX_HOTKEY_LEN} characters)"
        ));
    }

    let prev = state.settings.get();
    state.settings.set(settings.clone());

    crate::settings::save_to_store(&app, &settings)
        .map_err(|e| format!("persist settings: {e}"))?;

    if prev.hotkey != settings.hotkey {
        // Failure here is non-fatal — settings still saved, user sees the
        // warning in logs and the Settings UI flag from hotkey_registered.
        let ok = crate::hotkey::register(&app, &settings.hotkey);
        state.hotkey_registered.store(ok, Ordering::SeqCst);
    }

    if prev.autostart != settings.autostart {
        crate::settings::sync_autostart(&app, settings.autostart)
            .map_err(|e| format!("autostart sync: {e}"))?;
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotMonitorMeta {
    pub index: usize,
    pub width: u32,
    pub height: u32,
}

/// Return per-monitor metadata for a held screenshot. The region UI uses
/// this to lay out a monitor picker and to know each image's native
/// dimensions for coordinate math.
#[tauri::command]
pub async fn get_screenshot_monitors(
    state: State<'_, AppState>,
    screenshot_id: String,
) -> Result<Vec<ScreenshotMonitorMeta>, String> {
    let held = state
        .screenshots
        .get_if_id(&screenshot_id)
        .ok_or_else(|| "screenshot not found or expired".to_string())?;
    Ok(held
        .monitors
        .iter()
        .map(|m| {
            let (w, h) = m.image.dimensions();
            ScreenshotMonitorMeta {
                index: m.index,
                width: w,
                height: h,
            }
        })
        .collect())
}

/// Return one monitor's image as a PNG, served as a binary Tauri response
/// (no base64 inflation). The frontend wraps the result in a Blob URL.
///
/// The PNG bytes are cached inside the held screenshot, so repeated calls
/// for the same monitor (e.g. user toggling monitors in the region
/// selector) hit a cheap Arc clone rather than re-encoding.
#[tauri::command]
pub async fn get_screenshot_monitor_png(
    state: State<'_, AppState>,
    screenshot_id: String,
    monitor_index: usize,
) -> Result<Response, String> {
    let held = state
        .screenshots
        .get_if_id(&screenshot_id)
        .ok_or_else(|| "screenshot not found or expired".to_string())?;
    let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let arc = held.png_for(monitor_index)?;
        Ok((*arc).clone())
    })
    .await
    .map_err(|e| format!("encode task panicked: {e}"))??;
    Ok(Response::new(bytes))
}

/// Decode every monitor image and build `ScanRow`s, deduping identical
/// content across monitors (first-monitor-wins per CLAUDE.md §9).
///
/// Per-monitor decode time is logged at INFO so users (and us) can see
/// whether decode is the dominant cost on a given setup — the answer
/// drives whether downsampling/parallelism is worth pursuing later.
///
/// Pure-ish: generic over `Decoder` so tests can pass a fake. The only
/// non-purity is the timing log, which is fine in tests (the logger is
/// uninitialized there and the call is a no-op).
fn decode_monitors<D: Decoder + ?Sized>(
    decoder: &D,
    monitors: &[MonitorImage],
    batch_id: &str,
    scanned_at: i64,
) -> Vec<ScanRow> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut rows = Vec::new();
    for m in monitors {
        let (w, h) = m.image.dimensions();
        let started = Instant::now();
        let decoded = decoder.decode(&m.image);
        let elapsed = started.elapsed();
        log::info!(
            "decode: monitor={} {}x{} found={} took={}ms",
            m.index,
            w,
            h,
            decoded.len(),
            elapsed.as_millis()
        );
        for content in decoded {
            if !seen.insert(content.clone()) {
                continue;
            }
            let kind = classify_kind(&content);
            rows.push(ScanRow {
                id: -1,
                batch_id: batch_id.to_string(),
                content,
                kind,
                monitor_index: m.index as i64,
                scanned_at,
                opened: false,
                opened_at: None,
                copied: false,
                copied_at: None,
            });
        }
    }
    rows
}

fn current_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::QrKind;
    use image::RgbaImage;
    use std::sync::atomic::AtomicUsize;

    struct FakeDecoder {
        per_call: Vec<Vec<String>>,
        idx: AtomicUsize,
    }

    impl FakeDecoder {
        fn new(per_call: Vec<Vec<String>>) -> Self {
            Self {
                per_call,
                idx: AtomicUsize::new(0),
            }
        }
    }

    impl Decoder for FakeDecoder {
        fn decode(&self, _img: &RgbaImage) -> Vec<String> {
            let i = self.idx.fetch_add(1, Ordering::SeqCst);
            self.per_call.get(i).cloned().unwrap_or_default()
        }
    }

    fn monitor(index: usize) -> MonitorImage {
        MonitorImage {
            index,
            image: RgbaImage::new(1, 1),
        }
    }

    #[test]
    fn dedups_identical_content_across_monitors_keeping_first_monitor() {
        let decoder = FakeDecoder::new(vec![
            vec!["https://example.com".into()],
            vec!["https://example.com".into(), "other".into()],
        ]);
        let rows = decode_monitors(&decoder, &[monitor(0), monitor(1)], "batch-1", 1000);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].content, "https://example.com");
        assert_eq!(rows[0].monitor_index, 0, "first monitor wins on dedup");
        assert_eq!(rows[1].content, "other");
        assert_eq!(rows[1].monitor_index, 1);
    }

    #[test]
    fn returns_empty_when_no_codes_found() {
        let decoder = FakeDecoder::new(vec![vec![]]);
        let rows = decode_monitors(&decoder, &[monitor(0)], "batch-1", 1000);
        assert!(rows.is_empty());
    }

    #[test]
    fn classifies_kind_and_fills_row_fields() {
        let decoder = FakeDecoder::new(vec![vec![
            "https://example.com".into(),
            "plain text".into(),
            "mailto:a@b.com".into(),
        ]]);
        let rows = decode_monitors(&decoder, &[monitor(0)], "batch-1", 42);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].kind, QrKind::Url);
        assert_eq!(rows[1].kind, QrKind::Text);
        assert_eq!(rows[2].kind, QrKind::Email);
        assert!(rows.iter().all(|r| r.id == -1));
        assert!(rows.iter().all(|r| r.batch_id == "batch-1"));
        assert!(rows.iter().all(|r| r.scanned_at == 42));
        assert!(rows.iter().all(|r| !r.opened));
        assert!(rows.iter().all(|r| r.opened_at.is_none()));
        assert!(rows.iter().all(|r| !r.copied));
        assert!(rows.iter().all(|r| r.copied_at.is_none()));
    }

    fn bounds(x: u32, y: u32, w: u32, h: u32) -> RegionBounds {
        RegionBounds {
            x,
            y,
            w,
            h,
            monitor_index: 0,
        }
    }

    #[test]
    fn check_input_file_size_accepts_below_cap() {
        assert!(check_input_file_size(0).is_ok());
        assert!(check_input_file_size(MAX_INPUT_FILE_BYTES).is_ok());
    }

    #[test]
    fn check_input_file_size_rejects_above_cap() {
        let err = check_input_file_size(MAX_INPUT_FILE_BYTES + 1).unwrap_err();
        assert!(err.contains("too large"));
        assert!(err.contains(&MAX_INPUT_FILE_BYTES.to_string()));
    }

    #[test]
    fn check_decoded_pixels_accepts_realistic_dimensions() {
        // 4K = 3840 × 2160 ≈ 8.3 MP
        assert!(check_decoded_pixels(3840, 2160).is_ok());
        // 8K = 7680 × 4320 ≈ 33 MP
        assert!(check_decoded_pixels(7680, 4320).is_ok());
    }

    #[test]
    fn check_decoded_pixels_rejects_above_cap() {
        // 10000 × 10000 = 100 MP > 80 MP cap
        let err = check_decoded_pixels(10_000, 10_000).unwrap_err();
        assert!(err.contains("too large"));
        assert!(err.contains("10000x10000"));
    }

    #[test]
    fn check_decoded_pixels_rejects_decompression_bomb() {
        // The classic PNG bomb: 100000 × 100000 transparent canvas
        let err = check_decoded_pixels(100_000, 100_000).unwrap_err();
        assert!(err.contains("too large"));
    }

    #[test]
    fn decode_region_rejects_zero_size() {
        let decoder = FakeDecoder::new(vec![]);
        let img = RgbaImage::new(100, 100);
        let err = decode_region(&decoder, &img, &bounds(0, 0, 0, 10), "b", 0).unwrap_err();
        assert!(err.contains("non-zero"));
        let err = decode_region(&decoder, &img, &bounds(0, 0, 10, 0), "b", 0).unwrap_err();
        assert!(err.contains("non-zero"));
    }

    #[test]
    fn decode_region_rejects_out_of_bounds() {
        let decoder = FakeDecoder::new(vec![]);
        let img = RgbaImage::new(100, 100);
        let err = decode_region(&decoder, &img, &bounds(50, 50, 60, 60), "b", 0).unwrap_err();
        assert!(err.contains("out of image"));
    }

    #[test]
    fn decode_region_returns_decoder_results_within_bounds() {
        let decoder = FakeDecoder::new(vec![vec![
            "https://example.com".into(),
            "https://example.com".into(), // duplicate — should dedup
            "other".into(),
        ]]);
        let img = RgbaImage::new(100, 100);
        let rows = decode_region(&decoder, &img, &bounds(10, 10, 20, 20), "b-1", 7).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].content, "https://example.com");
        assert_eq!(rows[1].content, "other");
        assert!(rows.iter().all(|r| r.batch_id == "b-1"));
        assert!(rows.iter().all(|r| r.scanned_at == 7));
        assert!(rows.iter().all(|r| r.monitor_index == 0));
    }

    #[test]
    fn decode_region_end_to_end_crops_and_decodes() {
        use crate::decoder::RqrrDecoder;
        use image::Rgba;

        let qr_img: image::GrayImage = qrcode::QrCode::new(b"https://qrab.test")
            .expect("qrcode encode")
            .render::<image::Luma<u8>>()
            .quiet_zone(true)
            .module_dimensions(8, 8)
            .build();
        let qr_rgba = image::DynamicImage::ImageLuma8(qr_img).to_rgba8();
        let (qw, qh) = qr_rgba.dimensions();

        // Place the QR at (50, 60) on a 600x500 canvas filled with noise-free
        // white so anything OUTSIDE the bounds wouldn't decode.
        let mut canvas = image::RgbaImage::from_pixel(600, 500, Rgba([255, 255, 255, 255]));
        image::imageops::overlay(&mut canvas, &qr_rgba, 50, 60);

        let decoder = RqrrDecoder::new();
        let rows = decode_region(
            &decoder,
            &canvas,
            &RegionBounds {
                x: 50,
                y: 60,
                w: qw,
                h: qh,
                monitor_index: 3,
            },
            "batch-region",
            123,
        )
        .expect("decode_region ok");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, "https://qrab.test");
        assert_eq!(rows[0].kind, QrKind::Url);
        assert_eq!(rows[0].monitor_index, 3);
        assert_eq!(rows[0].batch_id, "batch-region");
        assert_eq!(rows[0].scanned_at, 123);
    }
}
