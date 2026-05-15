//! `#[tauri::command]` functions — the IPC surface.
//!
//! Commands return `Result<T, String>` to keep the boundary stable; the
//! underlying typed errors live in [`crate::error`]. Heavy work (capture,
//! decode) runs inside `tokio::task::spawn_blocking` so the async runtime
//! stays responsive.

use crate::capture::{Capturer, MonitorImage};
use crate::decoder::{classify_kind, Decoder};
use crate::screenshot::{HeldScreenshot, ScreenshotStore};
use crate::storage::queries::ScanRow;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::ipc::Response;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

/// Tauri state container.
#[derive(Clone)]
pub struct AppState {
    pub capturer: Arc<dyn Capturer>,
    pub decoder: Arc<dyn Decoder>,
    pub screenshots: ScreenshotStore,
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

/// Capture every monitor, decode, persist the screenshot for region-select,
/// and return the deduped rows.
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
    let screenshot_id = new_id("scan");
    let batch_id = new_id("batch");

    // Decode in spawn_blocking; the closure returns (rows, monitors) so the
    // images survive to be put in the screenshot store for region-select.
    let (rows, monitors) = tokio::task::spawn_blocking(move || {
        let rows = decode_monitors(decoder.as_ref(), &monitors, &batch_id, scanned_at);
        (rows, monitors)
    })
    .await
    .map_err(|e| format!("decode task panicked: {e}"))?;

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
    let batch_id = new_id("batch");

    let rows: Vec<ScanRow> = tokio::task::spawn_blocking(
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

    Ok(ScanResult { rows, screenshot_id })
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

/// Open `url` in the user's default browser. Pre-DB this is fire-and-forget;
/// C13 will additionally mark the row's `opened` flag in storage.
#[tauri::command]
pub async fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
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

/// Build an id of the form `"{prefix}-{epoch_ms}-{seq}"`. Stable enough for
/// scan/batch handles pre-DB; replaced by ULIDs in C11.
fn new_id(prefix: &str) -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    format!("{prefix}-{}-{n}", current_epoch_ms())
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
