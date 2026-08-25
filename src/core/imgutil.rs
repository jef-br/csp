//! Small shared image utilities.

use image::{GrayImage, RgbImage};

/// Downscale by `scale` (<=1) with a triangle (area-like) filter. scale>=1 clones.
pub fn downscale(rgb: &RgbImage, scale: f64) -> RgbImage {
    if scale >= 1.0 {
        return rgb.clone();
    }
    let nw = ((rgb.width() as f64 * scale).round() as u32).max(8);
    let nh = ((rgb.height() as f64 * scale).round() as u32).max(8);
    image::imageops::resize(rgb, nw, nh, image::imageops::FilterType::Triangle)
}

/// Median value of a grayscale image via a 256-bin histogram.
pub fn median_u8(gray: &GrayImage) -> f64 {
    let mut hist = [0u32; 256];
    for p in gray.pixels() {
        hist[p[0] as usize] += 1;
    }
    let total: u32 = hist.iter().sum();
    if total == 0 {
        return 128.0;
    }
    let mut cum = 0u32;
    for (i, &c) in hist.iter().enumerate() {
        cum += c;
        if cum * 2 >= total {
            return i as f64;
        }
    }
    128.0
}
