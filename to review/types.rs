//! Shared geometry + detection types for the imaging core.

/// Axis-aligned integer box in pixel space of a specific image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Box {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Box {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Box { x, y, w, h }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn longest_side(&self) -> i32 {
        self.w.max(self.h)
    }

    pub fn area(&self) -> i64 {
        self.w as i64 * self.h as i64
    }

    pub fn center_x(&self) -> f64 {
        self.x as f64 + self.w as f64 / 2.0
    }

    pub fn center_y(&self) -> f64 {
        self.y as f64 + self.h as f64 / 2.0
    }
}

/// Which canvas edges the subject runs off. Drives the repositioning route.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EdgeIntersects {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

impl EdgeIntersects {
    pub fn count(&self) -> u32 {
        self.top as u32 + self.bottom as u32 + self.left as u32 + self.right as u32
    }
}

/// How the detector arrived at the box — used for routing + evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectionKind {
    /// A real subject box was isolated by the chroma/texture pass.
    Subject,
    /// No usable box; the whole frame was returned (routes to a centered crop).
    WholeFrame,
    /// Product bleeds off-canvas / fills the frame; the largest salient square is used.
    SalientSquare,
}

/// Result of subject detection on one image, in that image's own pixel space.
#[derive(Clone, Copy, Debug)]
pub struct Detection {
    pub box_: Box,
    pub intersects: EdgeIntersects,
    pub kind: DetectionKind,
    /// Confidence that `box_` covers the true product [0,1].
    pub confidence: f64,
    /// Fraction of the frame stripped as hard-shadow edge (shadow-present evidence).
    pub hard_shadow_fraction: f64,
}
