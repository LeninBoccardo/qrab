//! `#[tauri::command]` functions — the IPC surface.
//!
//! Commands return `Result<T, String>` to keep the boundary stable; the
//! underlying typed errors live in [`crate::error`]. Heavy work (capture,
//! decode) runs inside `tokio::task::spawn_blocking` so the async runtime
//! stays responsive.

use crate::capture::{Capturer, MonitorImage};
use crate::decoder::{classify_kind, Decoder, QrKind};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

/// Tauri state container — one capturer and one decoder per app instance.
#[derive(Clone)]
pub struct AppState {
    pub capturer: Arc<dyn Capturer>,
    pub decoder: Arc<dyn Decoder>,
}

/// One decoded QR, shaped to match the SQLite schema (CLAUDE.md §7) so the
/// IPC type doesn't change when persistence lands in C12. Pre-DB, `id` is
/// `-1` and `opened`/`opened_at` are inert.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRow {
    pub id: i64,
    pub batch_id: String,
    pub content: String,
    pub kind: QrKind,
    pub monitor_index: i64,
    pub scanned_at: i64,
    pub opened: bool,
    pub opened_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub rows: Vec<ScanRow>,
    /// Opaque handle the frontend echoes back to `scan_region` (Phase 2).
    pub screenshot_id: String,
}

/// Capture every monitor, decode, and return the deduped rows.
#[tauri::command]
pub async fn scan_screen(state: State<'_, AppState>) -> Result<ScanResult, String> {
    let capturer = state.capturer.clone();
    let decoder = state.decoder.clone();

    let monitors: Vec<MonitorImage> =
        tokio::task::spawn_blocking(move || capturer.capture_all())
            .await
            .map_err(|e| format!("capture task panicked: {e}"))?
            .map_err(|e| e.to_string())?;

    let scanned_at = current_epoch_ms();
    let batch_id = new_id("batch");
    let screenshot_id = new_id("scan");
    let batch_id_for_decode = batch_id.clone();

    let rows = tokio::task::spawn_blocking(move || {
        decode_monitors(decoder.as_ref(), &monitors, &batch_id_for_decode, scanned_at)
    })
    .await
    .map_err(|e| format!("decode task panicked: {e}"))?;

    // Silence "batch_id only needed inside the closure" — keep it bound so
    // logging hooks added later can reference the value without a re-grep.
    let _ = batch_id;

    Ok(ScanResult { rows, screenshot_id })
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
}
