//! Imaging core: detect → plan layout → background fill → final resize → save.

pub mod clahe;
pub mod config;
pub mod detect;
pub mod enhance;
pub mod fill;
pub mod geometry;
pub mod icc;
pub mod imgmath;
pub mod imgutil;
pub mod load;
pub mod resize;
pub mod saliency;
pub mod save;
pub mod segment;
pub mod superpixel;
pub mod types;

use image::{GrayImage, RgbImage};
use std::path::Path;

/// Full per-file pipeline: load, reposition, save as sRGB JPEG.
pub fn process_file(input: &Path, output: &Path) -> Result<(), String> {
    let loaded = load::load_image(input)?;
    let out = reposition(&loaded.rgb, loaded.alpha.as_ref());
    save::save_jpeg_srgb(&out, output, config::JPEG_QUALITY)
}

/// Reposition an in-memory image onto its final square canvas.
pub fn reposition(rgb: &RgbImage, alpha: Option<&GrayImage>) -> RgbImage {
    let det = detect::detect(rgb, alpha);
    let mut layout = geometry::plan(&det, rgb.width() as i32, rgb.height() as i32);

    // Grow the fill target so the final (<=MAX_UPSCALE) upscale can reach MIN_SIZE for tiny subjects.
    let min_fill = resize::min_fill_side();
    if !layout.already_square && layout.side < min_fill {
        layout.side = min_fill;
    }

    let square = fill::render(rgb, &layout);
    resize::resize_to_spec(&square)
}
