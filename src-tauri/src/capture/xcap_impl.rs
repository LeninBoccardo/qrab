use super::{CaptureError, Capturer, MonitorImage};
use xcap::Monitor;

/// Production [`Capturer`] backed by the `xcap` crate.
#[derive(Default)]
pub struct XcapCapturer;

impl XcapCapturer {
    pub fn new() -> Self {
        Self
    }
}

impl Capturer for XcapCapturer {
    fn capture_all(&self) -> Result<Vec<MonitorImage>, CaptureError> {
        let monitors =
            Monitor::all().map_err(|e| classify_xcap_error(e.to_string(), None))?;

        let mut images = Vec::with_capacity(monitors.len());
        for (index, monitor) in monitors.iter().enumerate() {
            let img = monitor
                .capture_image()
                .map_err(|e| classify_xcap_error(e.to_string(), Some(index)))?;
            images.push(MonitorImage { index, image: img });
        }
        Ok(images)
    }
}

fn classify_xcap_error(msg: String, index: Option<usize>) -> CaptureError {
    let lower = msg.to_lowercase();
    if lower.contains("permission")
        || lower.contains("denied")
        || lower.contains("not allowed")
    {
        return CaptureError::PermissionDenied;
    }
    match index {
        Some(i) => CaptureError::Monitor { index: i, message: msg },
        None => CaptureError::Enumerate(msg),
    }
}
