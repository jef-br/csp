//! Imaging core: load → preprocess → classify → dispatch → export.
//!
//! Classification and routing are being rebuilt around the BiRefNet shot classifier; until that
//! lands, this runs load → preprocess → export.

pub mod config;
pub mod exporter;
pub mod preprocessor;

use std::path::Path;

/// Full per-file pipeline.
pub fn process_file(input: &Path, output: &Path) -> Result<(), String> {
    let prep = preprocessor::prepare(input)?;
    exporter::export(&prep.original, output, input)
}
