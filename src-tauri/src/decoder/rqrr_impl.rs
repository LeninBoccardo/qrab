use super::Decoder;
use image::RgbaImage;
use rqrr::PreparedImage;

/// Production [`Decoder`] backed by `rqrr`.
#[derive(Default)]
pub struct RqrrDecoder;

impl RqrrDecoder {
    pub fn new() -> Self {
        Self
    }
}

impl Decoder for RqrrDecoder {
    fn decode(&self, img: &RgbaImage) -> Vec<String> {
        // rqrr operates on a grayscale image. `imageops::grayscale` produces
        // a `GrayImage` (= `ImageBuffer<Luma<u8>, Vec<u8>>`), which is what
        // `PreparedImage::prepare` expects.
        let gray = image::imageops::grayscale(img);
        let mut prepared = PreparedImage::prepare(gray);
        prepared
            .detect_grids()
            .into_iter()
            .filter_map(|grid| grid.decode().ok().map(|(_, content)| content))
            .collect()
    }
}
