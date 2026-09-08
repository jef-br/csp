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
//! Stub: returns the image unchanged.

use super::super::super::preprocessor::Prepared;
use crate::core::shot_classifier::ShotClassification;
use image::RgbImage;

pub fn apply(prep: &Prepared, _class: Option<&ShotClassification>) -> RgbImage {
    prep.original.clone()
}
