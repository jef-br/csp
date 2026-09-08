//! Exporter: enforce the output size envelope, save to `CSP-OUTPUT`, then remove the original from
//! `CSP-INPUT`.
//!
//! Every route ends here, so the envelope is applied here — one place, no route can bypass it.
//! Delete only runs after a successful save, so a write failure leaves the source untouched for a
//! retry.

pub mod icc;
pub mod resize;
pub mod save;

use super::config;
use image::RgbImage;
use std::path::Path;

pub fn export(img: &RgbImage, dest: &Path, source: &Path) -> Result<(), String> {
    let sized = resize::to_envelope(img);
    save::save_jpeg_srgb(&sized, dest, config::JPEG_QUALITY)?;
    std::fs::remove_file(source).map_err(|e| format!("delete original: {e}"))
}
