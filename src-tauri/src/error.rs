use crate::capture::CaptureError;
use crate::decoder::DecodeError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("capture failed: {0}")]
    Capture(#[from] CaptureError),

    #[error("decode failed: {0}")]
    Decode(#[from] DecodeError),

    #[error("image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type AppResult<T> = Result<T, AppError>;
