//! Final sizing: uniformly scale the square so its side lands in [MIN_SIZE, MAX_SIZE].
//!
//! The product is only ever scaled here, and only uniformly. A large square is scaled down; a
//! small square is scaled up toward MIN_SIZE but never by more than MAX_UPSCALE (the fill stage
//! grows the background enough that MIN_SIZE is reachable within that cap).

use super::config::{MAX_SIZE, MAX_UPSCALE, MIN_SIZE};
use image::{imageops, RgbImage};

/// The minimum square side the fill stage must reach so a <=MAX_UPSCALE upscale hits MIN_SIZE.
pub fn min_fill_side() -> i32 {
    (MIN_SIZE as f64 / MAX_UPSCALE).ceil() as i32
}

pub fn resize_to_spec(square: &RgbImage) -> RgbImage {
    let side = square.width().max(square.height());
    if side > MAX_SIZE {
        return imageops::resize(square, MAX_SIZE, MAX_SIZE, imageops::FilterType::Lanczos3);
    }
    if side < MIN_SIZE {
        return imageops::resize(square, MIN_SIZE, MIN_SIZE, imageops::FilterType::Lanczos3);
    }
    square.clone()
}
