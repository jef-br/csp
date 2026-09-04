//! Layout planning: turn a detection into a crop rectangle, a protected (never-scaled) product
//! rectangle, and a target square side. The routing tree mirrors PRISM's `Tx_DetailCropper` +
//! `Tx_CenterAndStretch`; the crop-to-real-pixels math mirrors `process_images.py`.

use super::super::config::*;
use super::super::types::*;

/// A planned layout in the original image's pixel space.
pub struct Layout {
    /// Region of the original to crop (already clamped to the frame — real pixels only).
    pub crop: Box,
    /// The product band inside the crop (crop-local coords). Never scaled by the fill.
    pub protected: Box,
    /// Target square side; the crop is grown with background to reach this.
    pub side: i32,
    /// True when the crop is already square and needs no background fill.
    pub already_square: bool,
}

pub fn plan(det: &Detection, img_w: i32, img_h: i32) -> Layout {
    match det.kind {
        DetectionKind::SalientSquare => Layout {
            crop: det.box_,
            protected: Box::new(0, 0, det.box_.w, det.box_.h),
            side: det.box_.longest_side(),
            already_square: true,
        },
        _ => {
            if det.intersects.count() == 0 {
                center_and_stretch(det, img_w, img_h)
            } else {
                detail_crop(det, img_w, img_h)
            }
        }
    }
}

// No edge intersect: crop box+margin to a square using real pixels, background fills the rest.
fn center_and_stretch(det: &Detection, img_w: i32, img_h: i32) -> Layout {
    // Shadow is excluded from the box at detection time (see detect::suppress_shadow), so no
    // blind post-hoc box shrink here — that used to eat real product on low-contrast subjects.
    let box_ = det.box_;
    let margin = ((box_.longest_side() as f64) * MARGIN_FRACTION).round() as i32;
    let (crop, side) = compute_square_crop(img_w, img_h, box_, margin);

    let protected = Box::new(
        (box_.x - margin - crop.x).max(0),
        (box_.y - margin - crop.y).max(0),
        (box_.w + 2 * margin).min(crop.w),
        (box_.h + 2 * margin).min(crop.h),
    );
    Layout { crop, protected, side, already_square: false }
}

// Ideal square around box+margin, slid back inside the frame then clipped to real pixels.
fn compute_square_crop(img_w: i32, img_h: i32, box_: Box, margin: i32) -> (Box, i32) {
    let dx0 = box_.x - margin;
    let dy0 = box_.y - margin;
    let dx1 = box_.right() + margin;
    let dy1 = box_.bottom() + margin;

    let side = (dx1 - dx0).max(dy1 - dy0);
    let cx = (dx0 + dx1) as f64 / 2.0;
    let cy = (dy0 + dy1) as f64 / 2.0;

    let mut sx0 = (cx - side as f64 / 2.0).round() as i32;
    let mut sy0 = (cy - side as f64 / 2.0).round() as i32;
    // Slide back inside the frame to use real pixels wherever possible.
    sx0 = sx0.clamp(0.min(img_w - side), (img_w - side).max(0)).min(img_w - side).max(0.min(img_w - side));
    sy0 = sy0.clamp(0.min(img_h - side), (img_h - side).max(0)).min(img_h - side).max(0.min(img_h - side));
    // The clamp above keeps sx0 in [min(0,img-side), max(0,img-side)]; normalize for both cases.
    sx0 = slide_inside(sx0, side, img_w);
    sy0 = slide_inside(sy0, side, img_h);

    let cx0 = sx0.max(0);
    let cy0 = sy0.max(0);
    let cx1 = (sx0 + side).min(img_w);
    let cy1 = (sy0 + side).min(img_h);
    (Box::new(cx0, cy0, cx1 - cx0, cy1 - cy0), side)
}

fn slide_inside(origin: i32, side: i32, extent: i32) -> i32 {
    // When side <= extent: clamp origin into [0, extent-side].
    // When side  > extent: clamp toward (extent-side) which is negative — still maximizes real pixels.
    let lo = (extent - side).min(0);
    let hi = (extent - side).max(0);
    origin.clamp(lo, hi)
}

// Edge-intersect routes: anchor touched edges, center free axes, extend background where needed.
fn detail_crop(det: &Detection, img_w: i32, img_h: i32) -> Layout {
    let e = det.intersects;
    let b = det.box_;
    match e.count() {
        1 => one_edge(b, img_w, img_h, e),
        2 if e.top && e.bottom => two_opposing(b, img_w, img_h, true),
        2 if e.left && e.right => two_opposing(b, img_w, img_h, false),
        2 => two_adjacent(b, img_w, img_h, e),
        3 => three_edges(b, img_w, img_h, e),
        _ => four_edges(b, img_w, img_h),
    }
}

// 1 edge: touched edge stays flush; margin added on the far side; free axis centered.
fn one_edge(b: Box, img_w: i32, img_h: i32, e: EdgeIntersects) -> Layout {
    let margin = (b.longest_side() as f64 * MARGIN_FRACTION).round() as i32;
    let (side, crop, protected);
    if e.left || e.right {
        // Horizontal axis is anchored.
        let target_w = b.w + margin;
        side = target_w;
        let x = if e.left { b.x } else { (b.right() - target_w).max(0) };
        let cx0 = x.max(0);
        let cx1 = (x + target_w).min(img_w);
        // Free (vertical) axis centered on the box at `side`.
        let cy0 = (b.center_y() - side as f64 / 2.0).round() as i32;
        let (cy0, cy1) = clamp_span(cy0, side, img_h);
        crop = Box::new(cx0, cy0, cx1 - cx0, cy1 - cy0);
        protected = Box::new((b.x - crop.x).max(0), (b.y - crop.y).max(0), b.w.min(crop.w), b.h.min(crop.h));
    } else {
        let target_h = b.h + margin;
        side = target_h;
        let y = if e.top { b.y } else { (b.bottom() - target_h).max(0) };
        let cy0 = y.max(0);
        let cy1 = (y + target_h).min(img_h);
        let cx0 = (b.center_x() - side as f64 / 2.0).round() as i32;
        let (cx0, cx1) = clamp_span(cx0, side, img_w);
        crop = Box::new(cx0, cy0, cx1 - cx0, cy1 - cy0);
        protected = Box::new((b.x - crop.x).max(0), (b.y - crop.y).max(0), b.w.min(crop.w), b.h.min(crop.h));
    }
    Layout { crop, protected, side, already_square: false }
}

// 2 opposite: pinned axis takes the full frame extent; free axis centered on the box.
fn two_opposing(b: Box, img_w: i32, img_h: i32, vertical_pinned: bool) -> Layout {
    let side = if vertical_pinned { img_h } else { img_w };
    let (crop, protected);
    if vertical_pinned {
        let cx0 = (b.center_x() - side as f64 / 2.0).round() as i32;
        let (cx0, cx1) = clamp_span(cx0, side, img_w);
        crop = Box::new(cx0, 0, cx1 - cx0, img_h);
    } else {
        let cy0 = (b.center_y() - side as f64 / 2.0).round() as i32;
        let (cy0, cy1) = clamp_span(cy0, side, img_h);
        crop = Box::new(0, cy0, img_w, cy1 - cy0);
    }
    protected = Box::new((b.x - crop.x).max(0), (b.y - crop.y).max(0), b.w.min(crop.w), b.h.min(crop.h));
    Layout { crop, protected, side, already_square: false }
}

// 2 adjacent: flush at the shared corner; side from the box's own extents.
fn two_adjacent(b: Box, img_w: i32, img_h: i32, e: EdgeIntersects) -> Layout {
    let side = b.w.max(b.h);
    let x = if e.left { b.x } else { (b.right() - side).max(0) };
    let y = if e.top { b.y } else { (b.bottom() - side).max(0) };
    let cx0 = x.max(0);
    let cy0 = y.max(0);
    let cx1 = (x + side).min(img_w);
    let cy1 = (y + side).min(img_h);
    let crop = Box::new(cx0, cy0, cx1 - cx0, cy1 - cy0);
    let protected = Box::new((b.x - crop.x).max(0), (b.y - crop.y).max(0), b.w.min(crop.w), b.h.min(crop.h));
    Layout { crop, protected, side, already_square: false }
}

// 3 edges: both-touched axis pinned to full frame; open axis anchored flush, no margin.
fn three_edges(b: Box, img_w: i32, img_h: i32, e: EdgeIntersects) -> Layout {
    let vertical_pinned = e.top && e.bottom;
    let side = if vertical_pinned { img_h } else { img_w };
    let (crop, protected);
    if vertical_pinned {
        let x = if e.left { b.x } else { (b.right() - side).max(0) };
        let cx0 = x.max(0);
        let cx1 = (x + side).min(img_w);
        crop = Box::new(cx0, 0, cx1 - cx0, img_h);
    } else {
        let y = if e.top { b.y } else { (b.bottom() - side).max(0) };
        let cy0 = y.max(0);
        let cy1 = (y + side).min(img_h);
        crop = Box::new(0, cy0, img_w, cy1 - cy0);
    }
    protected = Box::new((b.x - crop.x).max(0), (b.y - crop.y).max(0), b.w.min(crop.w), b.h.min(crop.h));
    Layout { crop, protected, side, already_square: false }
}

// 4 edges: centered square crop on the box center, no fill.
fn four_edges(b: Box, img_w: i32, img_h: i32) -> Layout {
    let side = img_w.min(img_h);
    let cx0 = (b.center_x() - side as f64 / 2.0).round() as i32;
    let cy0 = (b.center_y() - side as f64 / 2.0).round() as i32;
    let (cx0, _) = clamp_span(cx0, side, img_w);
    let (cy0, _) = clamp_span(cy0, side, img_h);
    let crop = Box::new(cx0, cy0, side, side);
    Layout { crop, protected: Box::new(0, 0, side, side), side, already_square: true }
}

// Clamp a span of length `side` starting at `start` to sit inside [0, extent]; returns (start,end).
fn clamp_span(start: i32, side: i32, extent: i32) -> (i32, i32) {
    let s = slide_inside(start, side, extent).max(0);
    let end = (s + side).min(extent);
    (s, end)
}
