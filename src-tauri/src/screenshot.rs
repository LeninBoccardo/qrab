//! In-memory store for the most recent captured screenshot.
//!
//! [`scan_screen`](crate::commands::scan_screen) puts the captured monitor
//! images here keyed by a generated `screenshot_id`; `scan_region` looks
//! that id up, crops the held image, and decodes the crop.
//!
//! Only the *latest* screenshot is kept — a fresh scan replaces the prior
//! one. The image is also cleared after [`TTL`] elapses, and when the
//! results window closes (the close handler in `lib.rs`) so we don't sit
//! on tens of megabytes of pixels indefinitely.
//!
//! PNG bytes are cached lazily on first request via [`HeldScreenshot::png_for`].
//! PNG encoding a 4K monitor is 30–200 ms; the region selector hits this
//! path every time the user switches monitors, so caching it cuts that
//! repeated cost to a single Arc clone after the first encode.

use crate::capture::MonitorImage;
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long a held screenshot stays in memory if no `scan_region` uses it.
pub const TTL: Duration = Duration::from_secs(60);

pub struct HeldScreenshot {
    pub id: String,
    pub monitors: Vec<MonitorImage>,
    pub taken_at: Instant,
    /// PNG-encoded bytes per monitor index. Populated on first request via
    /// [`png_for`](Self::png_for).
    png_cache: Mutex<HashMap<usize, Arc<Vec<u8>>>>,
}

impl HeldScreenshot {
    pub fn new(id: String, monitors: Vec<MonitorImage>, taken_at: Instant) -> Self {
        Self {
            id,
            monitors,
            taken_at,
            png_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Return the PNG-encoded bytes for `monitor_index`. First call encodes
    /// directly from the RGBA buffer (no `DynamicImage` clone). Subsequent
    /// calls return the cached `Arc` — cheap regardless of image size.
    pub fn png_for(&self, monitor_index: usize) -> Result<Arc<Vec<u8>>, String> {
        if let Some(bytes) = self
            .png_cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&monitor_index)
        {
            return Ok(bytes.clone());
        }

        let monitor = self
            .monitors
            .iter()
            .find(|m| m.index == monitor_index)
            .ok_or_else(|| {
                format!("monitor index {monitor_index} not in screenshot")
            })?;

        let mut buf = Vec::new();
        PngEncoder::new(&mut buf)
            .write_image(
                monitor.image.as_raw(),
                monitor.image.width(),
                monitor.image.height(),
                ExtendedColorType::Rgba8,
            )
            .map_err(|e| format!("png encode: {e}"))?;
        let bytes = Arc::new(buf);

        // Race-safe insert: if a concurrent caller raced us to encode, accept
        // theirs (Arc clone is cheap) and drop our duplicate.
        let mut guard = self.png_cache.lock().unwrap_or_else(|p| p.into_inner());
        Ok(guard.entry(monitor_index).or_insert(bytes).clone())
    }
}

/// Cloneable, thread-safe handle to the at-most-one held screenshot.
#[derive(Clone, Default)]
pub struct ScreenshotStore {
    inner: Arc<Mutex<Option<Arc<HeldScreenshot>>>>,
}

impl ScreenshotStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put(&self, held: HeldScreenshot) {
        *self.lock() = Some(Arc::new(held));
    }

    /// Return the held screenshot iff its id matches. Cheap — clones an
    /// `Arc`, not the pixel buffers.
    pub fn get_if_id(&self, id: &str) -> Option<Arc<HeldScreenshot>> {
        self.lock()
            .as_ref()
            .filter(|h| h.id == id)
            .map(Arc::clone)
    }

    pub fn clear(&self) {
        *self.lock() = None;
    }

    /// Clear the held screenshot if it has been around longer than [`TTL`].
    /// `now` is taken as a parameter so tests can drive the clock without
    /// sleeping.
    pub fn clear_if_expired(&self, now: Instant) {
        let mut guard = self.lock();
        if let Some(h) = guard.as_ref() {
            if now.saturating_duration_since(h.taken_at) > TTL {
                *guard = None;
            }
        }
    }

    /// Lock helper that recovers from poison rather than panicking — a
    /// poisoned `Option<Arc<HeldScreenshot>>` is safe to use as-is.
    fn lock(&self) -> MutexGuard<'_, Option<Arc<HeldScreenshot>>> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    fn held(id: &str, taken_at: Instant) -> HeldScreenshot {
        HeldScreenshot::new(
            id.to_string(),
            vec![MonitorImage { index: 0, image: RgbaImage::new(1, 1) }],
            taken_at,
        )
    }

    #[test]
    fn returns_held_when_id_matches() {
        let store = ScreenshotStore::new();
        store.put(held("scan-1", Instant::now()));
        assert!(store.get_if_id("scan-1").is_some());
    }

    #[test]
    fn returns_none_when_id_mismatch() {
        let store = ScreenshotStore::new();
        store.put(held("scan-1", Instant::now()));
        assert!(store.get_if_id("scan-2").is_none());
    }

    #[test]
    fn put_replaces_previous() {
        let store = ScreenshotStore::new();
        store.put(held("a", Instant::now()));
        store.put(held("b", Instant::now()));
        assert!(store.get_if_id("a").is_none());
        assert!(store.get_if_id("b").is_some());
    }

    #[test]
    fn clear_drops_held() {
        let store = ScreenshotStore::new();
        store.put(held("a", Instant::now()));
        store.clear();
        assert!(store.get_if_id("a").is_none());
    }

    #[test]
    fn expired_screenshots_are_cleared() {
        let store = ScreenshotStore::new();
        let past = Instant::now() - (TTL + Duration::from_secs(1));
        store.put(held("old", past));
        store.clear_if_expired(Instant::now());
        assert!(store.get_if_id("old").is_none());
    }

    #[test]
    fn unexpired_screenshots_are_kept() {
        let store = ScreenshotStore::new();
        store.put(held("fresh", Instant::now()));
        store.clear_if_expired(Instant::now());
        assert!(store.get_if_id("fresh").is_some());
    }

    #[test]
    fn png_for_returns_cached_arc_on_repeat_calls() {
        let held = held("p", Instant::now());
        let first = held.png_for(0).expect("first encode");
        let second = held.png_for(0).expect("cache hit");
        assert!(Arc::ptr_eq(&first, &second), "second call must return cached bytes");
        assert!(!first.is_empty(), "encoded PNG should not be empty");
    }

    #[test]
    fn png_for_errors_on_missing_monitor() {
        let held = held("p", Instant::now());
        assert!(held.png_for(99).is_err());
    }
}
