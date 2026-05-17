//! Integration tests for the rqrr-backed [`Decoder`].
//!
//! Fixtures are generated at test time via the `qrcode` crate (dev-dep)
//! rather than committed as PNGs. Keeps the repo binary-free and lets
//! each test express its inputs inline.

use image::Rgba;
use qrab_lib::decoder::{Decoder, RqrrDecoder};
use qrcode::QrCode;

/// Render `content` as a QR code on an RGBA canvas.
fn render_qr(content: &str) -> image::RgbaImage {
    let code = QrCode::new(content.as_bytes()).expect("qrcode encode");
    let gray: image::GrayImage = code
        .render::<image::Luma<u8>>()
        .quiet_zone(true)
        .module_dimensions(8, 8)
        .build();
    image::DynamicImage::ImageLuma8(gray).to_rgba8()
}

#[test]
fn decodes_a_single_url_qr() {
    let img = render_qr("https://example.com/test");
    let results = RqrrDecoder::new().decode(&img);
    assert_eq!(results, vec!["https://example.com/test".to_string()]);
}

#[test]
fn decodes_plain_text() {
    let img = render_qr("hello world");
    let results = RqrrDecoder::new().decode(&img);
    assert_eq!(results, vec!["hello world".to_string()]);
}

#[test]
fn returns_empty_on_image_without_qr() {
    let img = image::RgbaImage::from_pixel(200, 200, Rgba([255, 255, 255, 255]));
    let results = RqrrDecoder::new().decode(&img);
    assert!(
        results.is_empty(),
        "expected no decodes from a blank image, got: {:?}",
        results
    );
}

#[test]
fn decodes_two_codes_in_one_image() {
    let a = render_qr("first");
    let b = render_qr("second");
    let (aw, ah) = a.dimensions();
    let (bw, bh) = b.dimensions();
    let h = ah.max(bh);
    let gap = 40u32;
    let total_w = aw + gap + bw;
    let mut canvas = image::RgbaImage::from_pixel(total_w, h, Rgba([255, 255, 255, 255]));
    image::imageops::overlay(&mut canvas, &a, 0, 0);
    image::imageops::overlay(&mut canvas, &b, (aw + gap) as i64, 0);

    let mut results = RqrrDecoder::new().decode(&canvas);
    results.sort();
    assert_eq!(results, vec!["first".to_string(), "second".to_string()]);
}
