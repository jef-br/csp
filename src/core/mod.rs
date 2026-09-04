//! Imaging core: preprocess (load) → classify the shot → process (route) → export.
//!
//! One folder per container from `docs/diagrams/JB-A2B.drawio.svg`: `preprocessor`,
//! `shot_classifier`, `processor`, `exporter`. `config` and `types` are shared kernel — constants
//! and geometry types used across more than one container — so they sit here at the top level
//! instead of inside any single container.

pub mod config;
pub mod exporter;
pub mod preprocessor;
pub mod processor;
pub mod shot_classifier;
pub mod types;

use image::{GrayImage, RgbImage};
use std::path::Path;

/// Full per-file pipeline: load, reposition, export (save + delete original).
pub fn process_file(input: &Path, output: &Path) -> Result<(), String> {
    let loaded = preprocessor::load_image(input)?;
    let out = reposition(&loaded.rgb, loaded.alpha.as_ref());
    exporter::export(&out, output, input)
}

/// Reposition an in-memory image onto its final square canvas.
pub fn reposition(rgb: &RgbImage, alpha: Option<&GrayImage>) -> RgbImage {
    let det = shot_classifier::classify(rgb, alpha);
    let mut layout = processor::geometry::plan(&det, rgb.width() as i32, rgb.height() as i32);

    // Grow the fill target so the final (<=MAX_UPSCALE) upscale can reach MIN_SIZE for tiny subjects.
    let min_fill = processor::resize::min_fill_side();
    if !layout.already_square && layout.side < min_fill {
        layout.side = min_fill;
    }

    let square = processor::fill::render(rgb, &layout);
    processor::resize::resize_to_spec(&square)
}
