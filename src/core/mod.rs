//! Imaging core: preprocess (load) → export (save). Shot classification and processing
//! (repositioning the subject onto a square canvas) were parked in `to review/` for a
//! from-scratch rebuild — see that folder for the prior implementation and `config_full.rs` /
//! `types.rs` for the constants and geometry types they used.

pub mod config;
pub mod exporter;
pub mod preprocessor;

use std::path::Path;

/// Full per-file pipeline: load, export (save + delete original).
pub fn process_file(input: &Path, output: &Path) -> Result<(), String> {
    let loaded = preprocessor::load_image(input)?;
    exporter::export(&loaded.rgb, output, input)
}
