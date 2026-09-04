//! Exporter: saves the processed image to `CSP-OUTPUT`, then removes the original from
//! `CSP-INPUT`. Delete only runs after a successful save, so a write failure leaves the source
//! untouched for a retry.

pub mod icc;
pub mod save;

use super::config;
use image::RgbImage;
use std::path::Path;

pub fn export(img: &RgbImage, dest: &Path, source: &Path) -> Result<(), String> {
    save::save_jpeg_srgb(img, dest, config::JPEG_QUALITY)?;
    std::fs::remove_file(source).map_err(|e| format!("delete original: {e}"))
}
