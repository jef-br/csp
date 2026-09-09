//! R2 — the subject reaches at least one edge (EIX set, non-zero).
//!
//! This route holds the five bleed behaviours from `docs/csp-spec.md` §5. Which one applies is
//! decided *here*, from `touches_edges` — it is a behaviour inside R2, not a routing decision, so
//! it never reaches `Route::select`:
//!
//! | edges touched | behaviour |
//! |---|---|
//! | 1 | flush to that edge, centre the other axis |
//! | 2 opposite | fill that whole axis |
//! | 2 adjacent | flush into the shared corner |
//! | 3 | fill the boxed-in axis |
//! | 4 | fully bled → CoG/MSR square with extension |
//!
//! Per §5's data note the 3-edge case has never fired on the CiMini set, so it is the one to watch
//! when the distribution is re-measured.
//!
//! **Saliency lives here, not in R1.** Only the 4-edge row needs it: a fully-bled subject reaches
//! every border, so there is no background left to anchor a square against and the crop has to be
//! placed from where the salient mass actually sits. Every other row anchors on the edges the
//! subject touches, and [`super::center_and_stretch`] anchors on the mask's bounding box.
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
