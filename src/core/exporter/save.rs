//! JPEG output: quality 95, no chroma subsampling (4:4:4), embedded sRGB ICC profile in APP2.

use super::icc;
use image::RgbImage;
use jpeg_encoder::{ColorType, Density, Encoder, SamplingFactor};
use std::path::Path;

pub fn save_jpeg_srgb(img: &RgbImage, path: &Path, quality: u8) -> Result<(), String> {
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut encoder = Encoder::new(&mut buf, quality);
        encoder.set_sampling_factor(SamplingFactor::F_1_1); // 4:4:4, no chroma subsampling
        encoder.set_density(Density::Inch { x: 72, y: 72 });
        encoder
            .add_app_segment(2, &icc_app2_payload())
            .map_err(|e| format!("icc: {e:?}"))?;
        encoder
            .encode(img.as_raw(), img.width() as u16, img.height() as u16, ColorType::Rgb)
            .map_err(|e| format!("encode: {e:?}"))?;
    }
    std::fs::write(path, &buf).map_err(|e| format!("write: {e}"))
}

// APP2 ICC payload: "ICC_PROFILE\0" + sequence(1) + count(1) + profile bytes.
fn icc_app2_payload() -> Vec<u8> {
    let profile = icc::srgb_profile();
    let mut p = Vec::with_capacity(14 + profile.len());
    p.extend_from_slice(b"ICC_PROFILE\0");
    p.push(1); // sequence number
    p.push(1); // total segments
    p.extend_from_slice(&profile);
    p
}
