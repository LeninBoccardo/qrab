//! Screen capture abstraction.
//!
//! [`Capturer`] is the seam the decoder pipeline tests against — production
//! code uses [`xcap_impl::XcapCapturer`], tests can substitute a fake.

use image::RgbaImage;

#[derive(Debug, Clone)]
pub struct MonitorImage {
    pub index: usize,
    pub image: RgbaImage,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("failed to enumerate monitors: {0}")]
    Enumerate(String),

    #[error("failed to capture monitor {index}: {message}")]
    Monitor { index: usize, message: String },

    #[error("screen recording permission denied")]
    PermissionDenied,
}

pub trait Capturer: Send + Sync {
    fn capture_all(&self) -> Result<Vec<MonitorImage>, CaptureError>;
}

pub mod xcap_impl;
pub use xcap_impl::XcapCapturer;
