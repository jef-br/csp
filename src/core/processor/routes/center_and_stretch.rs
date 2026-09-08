//! R1 — the subject touches no edge (EIX `0000`).
//!
//! Strategy (`docs/csp-spec.md` §5, row `0 edges`): the subject stands clear of every border, so
//! there are real background pixels on all sides. Crop a square around the subject anchored on the
//! saliency-weighted centre of gravity, preferring real pixels, and only stretch background to
//! reach the square when a real-pixel crop cannot hold the subject — see §6's CoG/MSR algorithm.
//!
//! Stub: returns the image unchanged.

use super::super::super::preprocessor::Prepared;
use csp_shot_classifier::ShotClassification;
use image::RgbImage;

pub fn apply(prep: &Prepared, _class: Option<&ShotClassification>) -> RgbImage {
    prep.original.clone()
}
