//! Routing: turn the shot classifier's verdict into one of three processing routes.
//!
//! Three routes, keyed on the edge-intersection (EIX) verdict:
//!
//! | Route | Verdict | Strategy |
//! |---|---|---|
//! | R1 [`Route::CenterAndStretch`] | EIX `0000` — subject touches no edge | crop to a square around the subject, background fills the rest |
//! | R2 [`Route::CropSquare`] | EIX set and non-zero — subject bleeds off one or more edges | crop into real pixels, anchored on the edges it touches |
//! | R3 [`Route::Fallback`] | EIX not set — no verdict at all | fit the whole image into a square canvas |
//!
//! **Three routes, six behaviours.** `docs/csp-spec.md` §5's table is not a competing route list —
//! it is the *inside* of these routes. Its `0 edges` row is R1's strategy; its `1`, `2 opposite`,
//! `2 adjacent`, `3` and `4` rows are cases [`crop_square`] matches on internally as R2 grows.
//! Choosing between them is a behaviour, not a routing decision, so it never reaches
//! [`Route::select`].
//!
//! R3 is not a spec row. It exists because "the model produced no verdict" is a genuinely different
//! state from "the model looked and found no edge touched" — see [`Route::select`].

pub mod center_and_stretch;
pub mod crop_square;
pub mod fallback;

use super::super::preprocessor::Prepared;
use csp_shot_classifier::ShotClassification;
use image::RgbImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// R1 — the subject stands clear of every edge.
    CenterAndStretch,
    /// R2 — the subject reaches at least one edge.
    CropSquare,
    /// R3 — no classification was produced.
    Fallback,
}

impl Route {
    /// Pick the route for a verdict.
    ///
    /// `None` means the classifier could not produce one — a failed inference, an unusable model
    /// output. That is deliberately distinct from `Some` with an empty `touches_edges`: the first
    /// is "we don't know", the second is "we looked, and it touches nothing". They take different
    /// routes, so a model failure degrades one image instead of silently being processed as a
    /// clean free-standing shot.
    pub fn select(class: Option<&ShotClassification>) -> Route {
        match class {
            None => Route::Fallback,
            Some(c) if c.touches_edges.is_empty() => Route::CenterAndStretch,
            Some(_) => Route::CropSquare,
        }
    }
}

/// Run the route the verdict selects.
pub fn dispatch(prep: &Prepared, class: Option<&ShotClassification>) -> RgbImage {
    match Route::select(class) {
        Route::CenterAndStretch => center_and_stretch::apply(prep, class),
        Route::CropSquare => crop_square::apply(prep, class),
        Route::Fallback => fallback::apply(prep),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use csp_shot_classifier::Edge;

    fn verdict(touches: &[Edge]) -> ShotClassification {
        ShotClassification { touches_edges: touches.to_vec(), refinements: Vec::new() }
    }

    #[test]
    fn no_verdict_falls_back() {
        assert_eq!(Route::select(None), Route::Fallback);
    }

    #[test]
    fn every_edge_combination_routes() {
        // All 16 subsets of the four edges. Only the empty set is free-standing; every other
        // combination bleeds off at least one edge and crops.
        for bits in 0u8..16 {
            let touches: Vec<Edge> = Edge::ALL
                .iter()
                .enumerate()
                .filter(|(i, _)| bits & (1 << i) != 0)
                .map(|(_, &e)| e)
                .collect();

            let expected = if bits == 0 { Route::CenterAndStretch } else { Route::CropSquare };
            let got = Route::select(Some(&verdict(&touches)));
            assert_eq!(got, expected, "bits {bits:04b} ({touches:?}) routed to {got:?}");
        }
    }

    #[test]
    fn an_empty_verdict_is_not_a_missing_one() {
        // The distinction R3 exists for: "touches nothing" and "no verdict" must not collapse.
        assert_ne!(Route::select(Some(&verdict(&[]))), Route::select(None));
    }
}
