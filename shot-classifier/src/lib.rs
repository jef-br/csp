//! Shot classifier: is the subject touching an image border?
//!
//! A bounding box can't answer this reliably — the box can touch an
//! edge while the actual subject silhouette doesn't (or vice versa for
//! a diagonal pose). This crate answers it with segmentation instead,
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
//! inference call; wire up CSP's model loader to implement it. Feed its
//! output, together with the preprocessor's working-resolution image and
//! the original-resolution image, into [`classify_instance`].

#[cfg(feature = "birefnet")]
pub mod birefnet;
pub mod gate;
pub mod geometry;
pub mod refine;
pub mod sampling;
pub mod segmentation;

#[cfg(feature = "birefnet")]
pub use birefnet::{BiRefNetConfig, BiRefNetModel};
pub use geometry::Edge;
pub use refine::{EdgeRefinement, RefineParams, RefinementInput};
pub use segmentation::{Instance, Mask, SegmentationModel};

/// Full verdict for one detected instance: which edges, if any, it
/// actually touches.
#[derive(Debug, Clone, Default)]
pub struct ShotClassification {
    pub touches_edges: Vec<Edge>,
    /// Per-edge refinement detail, for edges pass 1 flagged as worth
    /// checking — useful for logging or visual export even when the
    /// verdict came back "not touching".
    pub refinements: Vec<EdgeRefinement>,
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

    ShotClassification { touches_edges, refinements }
}
