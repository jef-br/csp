//! R2 — the subject bleeds off all four edges (EIX `1111`).
//!
//! The one bleed case R1 cannot take. Everything else with a bled edge — one edge, two opposite,
//! two adjacent, three — is the same rule with different edges pinned, and lives in
//! [`super::center_and_stretch`]. Here the subject's box *is* the frame: every edge is blocked, so
//! there is nowhere to put the square's slack, and there is no background band anywhere to stretch
//! from. Both of R1's mechanisms are unavailable at once, which is why this is a route and not a
//! branch.
//!
//! `docs/csp-spec.md` §5 calls for a CoG/MSR square with extension: anchor on the saliency-weighted
//! centre of the most salient region, since with no background to read there is nothing else to
//! anchor on. Per §5's data note this fired on 2 of 112 CiMini images.
//!
//! Stub: returns the image unchanged.

use super::super::super::preprocessor::Prepared;
use super::Shot;
use image::RgbImage;

/// `shot.class.touches_edges` picks the behaviour from the table above; `shot.mask` is the subject
/// itself, which the 4-edge row needs in full rather than as a bounding box.
pub fn apply(prep: &Prepared, _shot: Shot<'_>) -> RgbImage {
    prep.original.clone()
}
