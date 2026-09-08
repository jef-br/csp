//! Pass 1: the cheap gate.
//!
//! Scans only a `margin_px`-wide band along each edge of the mask — not
//! the whole mask, not the image — so this stays fast across a whole
//! batch. A positive result means "worth running pass 2 refinement for
//! this edge", not "this instance touches the border": the mask is
//! still working-resolution and upsampled, so it isn't trusted for the
//! actual yes/no decision.

use crate::geometry::{Edge, EdgeView, Grid};
use crate::segmentation::Mask;

#[derive(Debug, Clone, Default)]
pub struct EdgeProximity {
    pub near: Vec<Edge>,
}

impl EdgeProximity {
    pub fn any(&self) -> bool {
        !self.near.is_empty()
    }
}

pub fn gate(mask: &Mask, margin_px: u32) -> EdgeProximity {
    let mut near = Vec::new();
    for edge in Edge::ALL {
        let view = EdgeView::new(mask, edge);
        let band_h = margin_px.min(view.height());
        let found = (0..band_h).any(|cy| (0..view.width()).any(|cx| view.get(cx, cy)));
        if found {
            near.push(edge);
        }
    }
    EdgeProximity { near }
}
