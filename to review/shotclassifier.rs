//! Shot-classifier entry point: Takes preprocessor.rs output
//! runs it through `classify`, then hands the result to processor.rs

use super::super::preprocessor::Loaded;
use super::super::processor;
use super::super::types::{Box, Detection, DetectionKind, EdgeIntersects};
use image::RgbImage;

/// Accepts the preprocessor's output and drives it through classification and processing.
pub fn main(loaded: &Loaded) -> RgbImage {
    let loaded = classify(loaded);
    let (w, h) = (loaded.rgb.width() as i32, loaded.rgb.height() as i32);

    // Placeholder detection (whole frame) until `classify` does real work of its own.
    let det = Detection {
        box_: Box::new(0, 0, w, h),
        intersects: EdgeIntersects::default(),
        kind: DetectionKind::WholeFrame,
        confidence: 0.0,
        hard_shadow_fraction: 0.0,
    };
    let layout = processor::geometry::plan(&det, w, h);
    let square = processor::fill::render(&loaded.rgb, &layout);
    processor::resize::resize_to_spec(&square)
}

/// Placeholder for the future shot-classification logic — currently a no-op pass-through.
fn classify(loaded: &Loaded) -> &Loaded {
    loaded
}