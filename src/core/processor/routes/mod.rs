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
use crate::core::shot_classifier::{geometry::Rect, Mask, ShotClassification};
use image::RgbImage;

/// What the classifier produced for one image: the edge verdict, and the mask it was read from.
///
/// The two travel together because they are produced together — there is no verdict without a
/// mask — so routes never have to handle "verdict but no mask". Absence of the whole thing is the
/// no-verdict case, and that is R3's business.
///
/// **Resolutions.** `mask` is aligned with [`Prepared::working`] (longest side ≤ `WORKING_SIZE`),
/// while routes compose their output from [`Prepared::original`]. Anything derived from the mask
/// has to cross that gap; [`Shot::subject_bbox_in`] is the crossing, so routes do not each
/// rediscover the scale factor.
#[derive(Clone, Copy)]
pub struct Shot<'a> {
    pub class: &'a ShotClassification,
    /// Working-resolution subject mask.
    pub mask: &'a Mask,
}

impl<'a> Shot<'a> {
    /// Tight bounding box of the subject, in the *working* image's coordinates.
    ///
    /// `None` for an empty mask — a classification can come back with nothing marked, and a route
    /// asking "where is the subject" deserves an answer that says "nowhere" rather than a box
    /// covering the whole frame.
    pub fn subject_bbox(&self) -> Option<Rect> {
        let (mut x0, mut y0) = (u32::MAX, u32::MAX);
        let (mut x1, mut y1) = (0u32, 0u32);
        for y in 0..self.mask.height {
            for x in 0..self.mask.width {
                if self.mask.get(x, y) {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        (x0 != u32::MAX).then(|| Rect::from_bounds(x0, y0, x1, y1))
    }

    /// The same box in `prep.original`'s coordinates.
    ///
    /// The preprocessor only ever scales down and keeps the aspect ratio, so one factor per axis
    /// is the whole of the mapping.
    pub fn subject_bbox_in(&self, prep: &Prepared) -> Option<Rect> {
        let fx = prep.original.width() as f32 / prep.working.width().max(1) as f32;
        let fy = prep.original.height() as f32 / prep.working.height().max(1) as f32;
        self.subject_bbox().map(|b| b.scaled(fx, fy))
    }
}

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
pub fn dispatch(prep: &Prepared, shot: Option<Shot<'_>>) -> RgbImage {
    match (Route::select(shot.map(|s| s.class)), shot) {
        (Route::CenterAndStretch, Some(shot)) => center_and_stretch::apply(prep, shot),
        (Route::CropSquare, Some(shot)) => crop_square::apply(prep, shot),
        // R3, and nothing else can reach here: `select` returns a non-fallback route only for
        // `Some`, so a missing shot always lands on the fallback. R1 and R2 therefore take a
        // plain `Shot` rather than an `Option`, and cannot be handed a state they can't serve.
        _ => fallback::apply(prep),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::shot_classifier::Edge;

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

    fn mask_with(w: u32, h: u32, subject: Rect) -> Mask {
        let mut m = Mask { width: w, height: h, data: vec![0u8; (w * h) as usize] };
        for y in subject.y..subject.y + subject.h {
            for x in subject.x..subject.x + subject.w {
                m.data[(y * w + x) as usize] = 255;
            }
        }
        m
    }

    #[test]
    fn subject_bbox_is_tight_around_the_mask() {
        let mask = mask_with(100, 100, Rect { x: 20, y: 30, w: 40, h: 20 });
        let class = verdict(&[]);
        let got = Shot { class: &class, mask: &mask }.subject_bbox().expect("mask is not empty");

        // Tight, not the frame: the whole point of deriving it from the mask rather than taking
        // `Instance::bbox`, which BiRefNet fills with the full image.
        assert_eq!((got.x, got.y, got.w, got.h), (20, 30, 40, 20));
    }

    #[test]
    fn an_empty_mask_has_no_subject_bbox() {
        let mask = Mask { width: 50, height: 50, data: vec![0u8; 2500] };
        let class = verdict(&[]);
        assert!(
            Shot { class: &class, mask: &mask }.subject_bbox().is_none(),
            "an empty mask must report no subject, not a box covering the frame"
        );
    }

    #[test]
    fn subject_bbox_scales_from_working_to_original() {
        let mask = mask_with(100, 100, Rect { x: 20, y: 30, w: 40, h: 20 });
        let class = verdict(&[]);
        let prep = Prepared {
            original: RgbImage::new(200, 200),
            working: RgbImage::new(100, 100),
        };

        let got = Shot { class: &class, mask: &mask }
            .subject_bbox_in(&prep)
            .expect("mask is not empty");

        // 2x on both axes. Routes compose from `original`, so handing them a working-resolution
        // box would misplace every crop by the downscale factor.
        assert_eq!((got.x, got.y, got.w, got.h), (40, 60, 80, 40));
    }
}
