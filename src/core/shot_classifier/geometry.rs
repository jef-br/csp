//! Edge-relative coordinate handling.
//!
//! The one idea this module exists for: every per-edge algorithm in this
//! crate (border-band scanning, background-color sampling, the trimap
//! ring) is written exactly once, in "top edge, depth increasing
//! downward" terms, against the [`Grid`] trait. [`EdgeView`] then presents
//! any of the other three edges as if they were the top edge, by
//! transposing/flipping coordinates. Nothing edge-specific lives outside
//! this file.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    pub const ALL: [Edge; 4] = [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right];
}

/// A read-only 2D grid of values addressed as `(x, y)`, `(0, 0)` at the
/// top-left. Implemented for [`super::segmentation::Mask`] directly and
/// for `image::RgbImage` via a thin adapter, so both can be run through
/// the same edge-relative algorithms.
pub trait Grid {
    type Item: Copy;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn get(&self, x: u32, y: u32) -> Self::Item;
}

/// Presents `inner` rotated/flipped so that `edge` reads as the top edge.
pub struct EdgeView<'a, G: Grid> {
    inner: &'a G,
    edge: Edge,
}

impl<'a, G: Grid> EdgeView<'a, G> {
    pub fn new(inner: &'a G, edge: Edge) -> Self {
        Self { inner, edge }
    }

    pub fn edge(&self) -> Edge {
        self.edge
    }

    /// Maps a canonical (top-edge-relative) coordinate back to the
    /// underlying grid's own coordinate space.
    pub fn to_original(&self, cx: u32, cy: u32) -> (u32, u32) {
        let (w, h) = (self.inner.width(), self.inner.height());
        match self.edge {
            Edge::Top => (cx, cy),
            Edge::Bottom => (cx, h.saturating_sub(1).saturating_sub(cy)),
            Edge::Left => (cy, cx),
            Edge::Right => (w.saturating_sub(1).saturating_sub(cy), cx),
        }
    }
}

impl<'a, G: Grid> Grid for EdgeView<'a, G> {
    type Item = G::Item;

    fn width(&self) -> u32 {
        match self.edge {
            Edge::Top | Edge::Bottom => self.inner.width(),
            Edge::Left | Edge::Right => self.inner.height(),
        }
    }

    fn height(&self) -> u32 {
        match self.edge {
            Edge::Top | Edge::Bottom => self.inner.height(),
            Edge::Left | Edge::Right => self.inner.width(),
        }
    }

    fn get(&self, cx: u32, cy: u32) -> Self::Item {
        let (ox, oy) = self.to_original(cx, cy);
        self.inner.get(ox, oy)
    }
}

/// An axis-aligned pixel rectangle, `[x, x+w) x [y, y+h)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn from_bounds(x0: u32, y0: u32, x1: u32, y1: u32) -> Self {
        Rect {
            x: x0,
            y: y0,
            w: x1.saturating_sub(x0) + 1,
            h: y1.saturating_sub(y0) + 1,
        }
    }

    /// Grows the rect by `px` on every side, clamped to `[0, bound_w) x [0, bound_h)`.
    pub fn expanded(&self, px: u32, bound_w: u32, bound_h: u32) -> Rect {
        let x0 = self.x.saturating_sub(px);
        let y0 = self.y.saturating_sub(px);
        let x1 = (self.x + self.w + px).min(bound_w);
        let y1 = (self.y + self.h + px).min(bound_h);
        Rect {
            x: x0,
            y: y0,
            w: x1.saturating_sub(x0),
            h: y1.saturating_sub(y0),
        }
    }

    /// Rescales from one coordinate space to another (e.g. working
    /// resolution to full original resolution) by independent x/y factors.
    pub fn scaled(&self, fx: f32, fy: f32) -> Rect {
        Rect {
            x: (self.x as f32 * fx).floor() as u32,
            y: (self.y as f32 * fy).floor() as u32,
            w: ((self.w as f32 * fx).ceil() as u32).max(1),
            h: ((self.h as f32 * fy).ceil() as u32).max(1),
        }
    }
}
