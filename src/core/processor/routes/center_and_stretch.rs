//! R1 — the subject touches no edge (EIX `0000`).
//!
//! Strategy (`docs/csp-spec.md` §5, row `0 edges`): the subject stands clear of every border, so
//! there are real background pixels on all sides. Crop a square around the subject, preferring
//! real pixels, and only stretch background to reach the square when a real-pixel crop cannot hold
//! the subject — see §6 for the composition rules and the 42% stretch cap.
//!
//! The subject region is the segmentation mask's own bounding box. **No saliency here**: the
//! saliency-weighted centre of gravity belongs to [`super::crop_square`], where a fully-bled
//! subject leaves no background to anchor against. R1 always has real background on all four
//! sides, so the mask's bbox is enough to place the square.
//!
//! Stub: returns the image unchanged.

use super::super::super::preprocessor::Prepared;
use super::Shot;
use image::RgbImage;

/// `shot` carries the verdict and the working-resolution mask; `shot.subject_bbox_in(prep)` gives
/// the subject's box in `prep.original`'s coordinates, which is what the square gets built around.
pub fn apply(prep: &Prepared, _shot: Shot<'_>) -> RgbImage {
    prep.original.clone()
}
