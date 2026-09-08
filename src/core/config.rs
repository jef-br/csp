//! Compile-time tuning constants.
//!
//! Detection and geometry constants moved to `to review/config_full.rs` along with the
//! shot_classifier/processor logic that reads them. This file keeps only what the essential
//! load → preproc → save pipeline needs.

/// JPEG quality for the saved output.
pub const JPEG_QUALITY: u8 = 95;
