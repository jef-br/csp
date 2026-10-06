//! Shot classifier: is the subject touching an image border?
//!
//! A bounding box can't answer this reliably — the box can touch an
//! edge while the actual subject silhouette doesn't (or vice versa for
//! a diagonal pose). This module answers it with segmentation instead,
//! in two passes:
//!
//! - **Pass 1** ([`gate`]): a cheap scan of the segmentation mask's
//!   border-adjacent band, per edge, to decide which edges are even
//!   worth checking precisely. This is the filter that keeps the
//!   expensive path off the common case (subject nowhere near an edge).
//! - **Pass 2** ([`refine`]): for edges pass 1 flagged, intersects the
//!   mask with the border band, rescales that region to full original
//!   resolution, crops it, and refines the boundary there via a
//!   trimap + color-distance matting step — because the segmentation
//!   mask is upsampled from a lower internal resolution and isn't
//!   pixel-accurate at the boundary. Only the refined, full-resolution
//!   result decides touching or not.
//!
//! [`segmentation::SegmentationModel`] abstracts the actual BiRefNet
//! inference call, so the gate/refine logic can be exercised against a
//! stand-in model. [`super::classify`] wires the real one up and feeds
//! [`classify_instance`] the preprocessor's two resolutions.

pub mod birefnet;
mod cipher;
pub mod gate;
pub mod geometry;
pub mod refine;
pub mod sampling;
pub mod segmentation;
pub mod shotcode;

pub use birefnet::{BiRefNetConfig, BiRefNetModel};
pub use geometry::Edge;
pub use refine::{EdgeRefinement, RefineParams, RefinementInput};
pub use segmentation::{Instance, Mask, SegmentationModel};

/// Full verdict for one detected instance: which edges, if any, it
/// actually touches, and whether the mask behind that answer is worth
/// believing at all.
#[derive(Debug, Clone, Default)]
pub struct ShotClassification {
    pub touches_edges: Vec<Edge>,
    /// The mask covers too little of the frame to be a subject — see [`shotcode::mask_too_small`].
    ///
    /// Measured here, next to the mask it is measured from, and consumed in two places: routing
    /// sends it to the route that crops rather than the one that frames, and the debug tag reports
    /// it. Neither recomputes it, so the tag can never disagree with the route.
    pub mask_too_small: bool,
    /// Per-edge refinement detail, for edges pass 1 flagged as worth
    /// checking — useful for logging or visual export even when the
    /// verdict came back "not touching".
    pub refinements: Vec<EdgeRefinement>,
}

/// Did the primary model find no subject, so the fallback model should be tried?
///
/// "No subject" is the existing [`shotcode::mask_too_small`] test — the mask covers under
/// [`MASK_MIN_COVERAGE`](super::config::MASK_MIN_COVERAGE) of the frame — not a new rule. The
/// failure this exists for (the 384 int8 model on striped knitwear) leaves every pixel below the
/// threshold, so the mask is empty and sits well under that line. It also fires for the few
/// close-ups where the primary latched onto a speck; the fallback gets a look at those too.
pub fn needs_fallback(working: &image::RgbImage, mask: &Mask) -> bool {
    shotcode::mask_too_small(working, mask)
}

/// Runs both passes for one instance: gates on all four edges, then
/// refines only the ones the gate flagged.
pub fn classify_instance(input: &RefinementInput, gate_margin_px: u32, params: RefineParams) -> ShotClassification {
    let proximity = gate::gate(&input.instance.mask, gate_margin_px);

    let mut touches_edges = Vec::new();
    let mut refinements = Vec::with_capacity(proximity.near.len());
    for edge in proximity.near {
        let result = refine::refine_edge(input, edge, params);
        if result.touching {
            touches_edges.push(edge);
        }
        refinements.push(result);
    }

    ShotClassification {
        touches_edges,
        mask_too_small: shotcode::mask_too_small(input.working_image, &input.instance.mask),
        refinements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;

    fn mask_with(fg_rows: u32) -> (RgbImage, Mask) {
        let (w, h) = (100, 100);
        let mut data = vec![0u8; (w * h) as usize];
        data[..(w * fg_rows) as usize].fill(255);
        (RgbImage::new(w, h), Mask { width: w, height: h, data })
    }

    #[test]
    fn empty_mask_needs_fallback() {
        let (img, mask) = mask_with(0);
        assert!(needs_fallback(&img, &mask));
    }

    #[test]
    fn mask_just_under_the_line_needs_fallback_and_at_it_does_not() {
        // 100x100 frame: 4 rows = 4.0% (< 5%), 5 rows = 5.0% (not < 5%).
        let (img, mask) = mask_with(4);
        assert!(needs_fallback(&img, &mask));
        let (img, mask) = mask_with(5);
        assert!(!needs_fallback(&img, &mask));
    }

    #[test]
    fn full_mask_skips_fallback() {
        let (img, mask) = mask_with(100);
        assert!(!needs_fallback(&img, &mask));
    }
}
