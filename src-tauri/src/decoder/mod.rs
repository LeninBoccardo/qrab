//! QR decoding abstraction.
//!
//! [`Decoder`] is mocked in tests; production code uses
//! [`rqrr_impl::RqrrDecoder`].

use image::RgbaImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QrKind {
    Url,
    Text,
    Wifi,
    Vcard,
    Email,
    Phone,
    Other,
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("decode failed: {0}")]
    Failure(String),
}

pub trait Decoder: Send + Sync {
    fn decode(&self, img: &RgbaImage) -> Vec<String>;
}

/// Classify the decoded payload into a [`QrKind`] for UI display.
///
/// Pure function — no I/O. URL scheme check is case-insensitive so
/// `HTTPS://...` still counts as a URL. Whitespace-only or empty input
/// returns [`QrKind::Other`].
pub fn classify_kind(s: &str) -> QrKind {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return QrKind::Other;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        QrKind::Url
    } else if trimmed.starts_with("WIFI:") {
        QrKind::Wifi
    } else if trimmed.starts_with("BEGIN:VCARD") {
        QrKind::Vcard
    } else if lower.starts_with("mailto:") {
        QrKind::Email
    } else if lower.starts_with("tel:") {
        QrKind::Phone
    } else {
        QrKind::Text
    }
}

pub mod rqrr_impl;
pub use rqrr_impl::RqrrDecoder;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_http_url() {
        assert_eq!(classify_kind("http://example.com"), QrKind::Url);
    }

    #[test]
    fn classifies_https_url() {
        assert_eq!(classify_kind("https://example.com/p?q=1"), QrKind::Url);
    }

    #[test]
    fn url_scheme_is_case_insensitive() {
        assert_eq!(classify_kind("HTTPS://example.com"), QrKind::Url);
    }

    #[test]
    fn classifies_wifi_payload() {
        assert_eq!(classify_kind("WIFI:T:WPA;S:MyNet;P:secret;;"), QrKind::Wifi);
    }

    #[test]
    fn classifies_vcard() {
        assert_eq!(
            classify_kind("BEGIN:VCARD\nVERSION:3.0\nFN:Alice\nEND:VCARD"),
            QrKind::Vcard
        );
    }

    #[test]
    fn classifies_mailto_as_email() {
        assert_eq!(classify_kind("mailto:a@b.com"), QrKind::Email);
    }

    #[test]
    fn classifies_tel_as_phone() {
        assert_eq!(classify_kind("tel:+15551234567"), QrKind::Phone);
    }

    #[test]
    fn falls_back_to_text() {
        assert_eq!(classify_kind("just a plain string"), QrKind::Text);
    }

    #[test]
    fn empty_or_whitespace_is_other() {
        assert_eq!(classify_kind(""), QrKind::Other);
        assert_eq!(classify_kind("   "), QrKind::Other);
        assert_eq!(classify_kind("\n\t"), QrKind::Other);
    }
}
