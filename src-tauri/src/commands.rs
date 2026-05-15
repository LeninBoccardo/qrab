//! `#[tauri::command]` functions — the IPC surface.
//!
//! Commands return `Result<T, String>` to keep the boundary stable; the
//! underlying typed errors live in [`crate::error`]. Heavy work (capture,
//! decode) runs inside `tokio::task::spawn_blocking` so the async runtime
//! stays responsive.

use crate::capture::{Capturer, MonitorImage};
use crate::decoder::{classify_kind, Decoder, QrKind};
use crate::screenshot::{HeldScreenshot, ScreenshotStore};
use crate::storage::queries::{
    delete_all, delete_by_id, get_by_id, history_query as history_query_db,
    insert_batch, mark_opened as mark_opened_db, HistoryFilter, NewScanRow,
    ScanRow,
};
use crate::storage::Storage;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Cursor;
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
    let capturer = state.capturer.clone();
    let decoder = state.decoder.clone();
    let store = state.screenshots.clone();

    let monitors: Vec<MonitorImage> =
        tokio::task::spawn_blocking(move || capturer.capture_all())
            .await
            .map_err(|e| format!("capture task panicked: {e}"))?
            .map_err(|e| e.to_string())?;

    let scanned_at = current_epoch_ms();
    let screenshot_id = Ulid::new().to_string();
    let batch_id = Ulid::new().to_string();

    // Decode in spawn_blocking; the closure returns (rows, monitors) so the
    // images survive to be put in the screenshot store for region-select.
    let (mut rows, monitors) = tokio::task::spawn_blocking(move || {
        let rows = decode_monitors(decoder.as_ref(), &monitors, &batch_id, scanned_at);
        (rows, monitors)
    })
    .await
    .map_err(|e| format!("decode task panicked: {e}"))?;

    // Persist matches and fill in the real DB IDs.
    persist_rows(&state.storage, &mut rows)?;

    store.put(HeldScreenshot {
        id: screenshot_id.clone(),
        monitors,
        taken_at: Instant::now(),
    });

    Ok(ScanResult { rows, screenshot_id })
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
    let held = state
        .screenshots
        .get_if_id(&screenshot_id)
        .ok_or_else(|| "screenshot not found or expired".to_string())?;
    let decoder = state.decoder.clone();
    let scanned_at = current_epoch_ms();
    let batch_id = Ulid::new().to_string();

    let mut rows: Vec<ScanRow> = tokio::task::spawn_blocking(
        move || -> Result<Vec<ScanRow>, String> {
            let monitor = held
                .monitors
                .iter()
                .find(|m| m.index == bounds.monitor_index)
                .ok_or_else(|| {
                    format!(
                        "monitor index {} not in screenshot",
                        bounds.monitor_index
                    )
                })?;
            decode_region(
                decoder.as_ref(),
                &monitor.image,
                &bounds,
                &batch_id,
                scanned_at,
            )
        },
    )
    .await
    .map_err(|e| format!("region decode task panicked: {e}"))??;

    persist_rows(&state.storage, &mut rows)?;

    Ok(ScanResult { rows, screenshot_id })
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
    let ids =
        insert_batch(storage, &new_rows).map_err(|e| format!("storage: {e}"))?;
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
    if bounds.x.saturating_add(bounds.w) > img_w
        || bounds.y.saturating_add(bounds.h) > img_h
    {
        return Err(format!(
            "bounds out of image (image {img_w}x{img_h}, region {}+{}, {}+{})",
            bounds.x, bounds.w, bounds.y, bounds.h
        ));
    }

    let crop =
        image::imageops::crop_imm(image, bounds.x, bounds.y, bounds.w, bounds.h)
            .to_image();

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
        });
    }
    Ok(rows)
}

/// Write `text` to the system clipboard.
#[tauri::command]
pub async fn copy_to_clipboard(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

/// Open a stored row's URL in the user's default browser and mark the
/// row as opened. The frontend passes the row id rather than the URL so
/// the open-and-mark pair is atomic on the Rust side.
#[tauri::command]
pub async fn open_url(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let row = get_by_id(&state.storage, id)
        .map_err(|e| format!("storage: {e}"))?
        .ok_or_else(|| format!("row {id} not found"))?;
    if row.kind != QrKind::Url {
        return Err(format!("row {id} is not a URL ({:?})", row.kind));
    }
    app.opener()
        .open_url(&row.content, None::<&str>)
        .map_err(|e| e.to_string())?;
    mark_opened_db(&state.storage, id, current_epoch_ms())
        .map_err(|e| format!("storage: {e}"))?;
    Ok(())
}

/// Mark a row as opened without opening the URL — exposed for cases where
/// the frontend wants to record the interaction separately (e.g. when a
/// non-URL kind is "viewed").
#[tauri::command]
pub async fn mark_opened(
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    mark_opened_db(&state.storage, id, current_epoch_ms())
        .map_err(|e| format!("storage: {e}"))?;
    Ok(())
}

/// Run a paginated history query.
#[tauri::command]
pub async fn history_query(
    state: State<'_, AppState>,
    filter: HistoryFilter,
) -> Result<Vec<ScanRow>, String> {
    history_query_db(&state.storage, &filter)
        .map_err(|e| format!("storage: {e}"))
}

/// Delete one row from history.
#[tauri::command]
pub async fn history_delete(
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    delete_by_id(&state.storage, id).map_err(|e| format!("storage: {e}"))?;
    Ok(())
}

/// Wipe the whole history table.
#[tauri::command]
pub async fn history_clear(state: State<'_, AppState>) -> Result<(), String> {
    delete_all(&state.storage).map_err(|e| format!("storage: {e}"))?;
    Ok(())
}

/// Above this count the frontend must surface ConfirmOpenAll first
/// (CLAUDE.md §10). The Rust side double-checks via `open_urls_bulk` so
/// a buggy or compromised frontend can't tab-bomb the user.
pub const BULK_OPEN_CONFIRM_THRESHOLD: usize = 3;

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
    // Resolve rows up front so the count check uses URLs only — non-URL
    // rows in the selection don't tab-bomb the threshold.
    let mut url_rows: Vec<(i64, String)> = Vec::with_capacity(ids.len());
    let mut skipped_non_url = 0usize;
    for id in &ids {
        let row = get_by_id(&state.storage, *id)
            .map_err(|e| format!("storage: {e}"))?
            .ok_or_else(|| format!("row {id} not found"))?;
        if row.kind == QrKind::Url {
            url_rows.push((*id, row.content));
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

    let now_ms = current_epoch_ms();
    let opener = app.opener();
    let mut opened = Vec::new();
    let mut failed = Vec::new();

    for (id, url) in url_rows {
        match opener.open_url(&url, None::<&str>) {
            Ok(()) => {
                if let Err(e) = mark_opened_db(&state.storage, id, now_ms) {
                    eprintln!("[qrab] mark_opened failed for {id}: {e}");
                }
                opened.push(id);
            }
            Err(e) => failed.push(BulkOpenFailure { id, error: e.to_string() }),
        }
    }

    Ok(BulkOpenResult { opened, failed, skipped_non_url })
}

/// Hide the results window. Bound to Esc and the titlebar Close button.
#[tauri::command]
pub async fn hide_results_window(app: AppHandle) -> Result<(), String> {
    crate::windows::hide_results_window(&app);
    Ok(())
}

/// Atomically clear and return the pending-scan flag. The frontend calls
/// this on mount so a hotkey that fired before the listener attached
/// still produces a scan.
#[tauri::command]
pub async fn consume_pending_scan(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.pending_scan.swap(false, Ordering::SeqCst))
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
            ScreenshotMonitorMeta { index: m.index, width: w, height: h }
        })
        .collect())
}

/// Return one monitor's image as a PNG, served as a binary Tauri response
/// (no base64 inflation). The frontend wraps the result in a Blob URL.
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
    let bytes = tokio::task::spawn_blocking(
        move || -> Result<Vec<u8>, String> {
            let monitor = held
                .monitors
                .iter()
                .find(|m| m.index == monitor_index)
                .ok_or_else(|| {
                    format!("monitor index {monitor_index} not in screenshot")
                })?;
            let dyn_img =
                image::DynamicImage::ImageRgba8(monitor.image.clone());
            let mut buf = Cursor::new(Vec::<u8>::new());
            dyn_img
                .write_to(&mut buf, image::ImageFormat::Png)
                .map_err(|e| format!("png encode: {e}"))?;
            Ok(buf.into_inner())
        },
    )
    .await
    .map_err(|e| format!("encode task panicked: {e}"))??;
    Ok(Response::new(bytes))
}

/// Decode every monitor image and build `ScanRow`s, deduping identical
/// content across monitors (first-monitor-wins per CLAUDE.md §9).
///
/// Pure and generic over `Decoder` so tests can pass a fake.
fn decode_monitors<D: Decoder + ?Sized>(
    decoder: &D,
    monitors: &[MonitorImage],
    batch_id: &str,
    scanned_at: i64,
) -> Vec<ScanRow> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut rows = Vec::new();
    for m in monitors {
        for content in decoder.decode(&m.image) {
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
            Self { per_call, idx: AtomicUsize::new(0) }
        }
    }

    impl Decoder for FakeDecoder {
        fn decode(&self, _img: &RgbaImage) -> Vec<String> {
            let i = self.idx.fetch_add(1, Ordering::SeqCst);
            self.per_call.get(i).cloned().unwrap_or_default()
        }
    }

    fn monitor(index: usize) -> MonitorImage {
        MonitorImage { index, image: RgbaImage::new(1, 1) }
    }

    #[test]
    fn dedups_identical_content_across_monitors_keeping_first_monitor() {
        let decoder = FakeDecoder::new(vec![
            vec!["https://example.com".into()],
            vec!["https://example.com".into(), "other".into()],
        ]);
        let rows = decode_monitors(
            &decoder,
            &[monitor(0), monitor(1)],
            "batch-1",
            1000,
        );
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
    }

    fn bounds(x: u32, y: u32, w: u32, h: u32) -> RegionBounds {
        RegionBounds { x, y, w, h, monitor_index: 0 }
    }

    #[test]
    fn decode_region_rejects_zero_size() {
        let decoder = FakeDecoder::new(vec![]);
        let img = RgbaImage::new(100, 100);
        let err =
            decode_region(&decoder, &img, &bounds(0, 0, 0, 10), "b", 0).unwrap_err();
        assert!(err.contains("non-zero"));
        let err =
            decode_region(&decoder, &img, &bounds(0, 0, 10, 0), "b", 0).unwrap_err();
        assert!(err.contains("non-zero"));
    }

    #[test]
    fn decode_region_rejects_out_of_bounds() {
        let decoder = FakeDecoder::new(vec![]);
        let img = RgbaImage::new(100, 100);
        let err = decode_region(&decoder, &img, &bounds(50, 50, 60, 60), "b", 0)
            .unwrap_err();
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
        let rows =
            decode_region(&decoder, &img, &bounds(10, 10, 20, 20), "b-1", 7)
                .unwrap();
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
        let mut canvas =
            image::RgbaImage::from_pixel(600, 500, Rgba([255, 255, 255, 255]));
        image::imageops::overlay(&mut canvas, &qr_rgba, 50, 60);

        let decoder = RqrrDecoder::new();
        let rows = decode_region(
            &decoder,
            &canvas,
            &RegionBounds { x: 50, y: 60, w: qw, h: qh, monitor_index: 3 },
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
