//! Background color sampling for one edge.
//!
//! One function, not four: per-edge behaviour comes entirely from
//! [`EdgeView`]'s coordinate transform, so this runs once per edge with
//! no edge-specific branching here.

use crate::geometry::{Edge, EdgeView, Grid};
use crate::segmentation::Mask;
use image::{Rgb, RgbImage};

#[derive(Debug, Clone, Copy)]
pub struct ColorStats {
    pub mean: [f32; 3],
    pub sample_count: u32,
}

impl ColorStats {
    /// Squared Euclidean distance in RGB space. Cheap and sufficient for
    /// a foreground/background decision; swap for a perceptual distance
    /// (e.g. CIEDE2000 in Lab) later if the classifier proves sensitive
    /// to it on hard cases.
    pub fn distance_sq(&self, px: Rgb<u8>) -> f32 {
        let dr = px[0] as f32 - self.mean[0];
        let dg = px[1] as f32 - self.mean[1];
        let db = px[2] as f32 - self.mean[2];
        dr * dr + dg * dg + db * db
    }
}

/// Thin adapter so `RgbImage` satisfies [`Grid`] without CSP's image
/// pipeline needing to depend on this crate's trait directly.
pub(crate) struct ImageGrid<'a>(pub &'a RgbImage);

impl<'a> Grid for ImageGrid<'a> {
    type Item = Rgb<u8>;
    fn width(&self) -> u32 {
        self.0.width()
    }
    fn height(&self) -> u32 {
        self.0.height()
    }
    fn get(&self, x: u32, y: u32) -> Rgb<u8> {
        *self.0.get_pixel(x, y)
    }
}

/// Samples the background color near one edge: a `strip_px`-wide band
/// starting at the image border, with the mask — plus a `safety_px`
/// margin around it — excluded, so no subject-edge pixels leak into the
/// background statistics.
pub fn sample_background_color(
    image: &RgbImage,
    mask: &Mask,
    edge: Edge,
    strip_px: u32,
    safety_px: u32,
) -> Option<ColorStats> {
    let img_grid = ImageGrid(image);
    let img_view = EdgeView::new(&img_grid, edge);
    let mask_view = EdgeView::new(mask, edge);
    let strip_h = strip_px.min(img_view.height());

    let mut sum = [0f64; 3];
    let mut count = 0u32;
    for cy in 0..strip_h {
        for cx in 0..img_view.width() {
            if mask_is_near(&mask_view, cx, cy, safety_px) {
                continue;
            }
            let Rgb(px) = img_view.get(cx, cy);
            sum[0] += px[0] as f64;
            sum[1] += px[1] as f64;
            sum[2] += px[2] as f64;
            count += 1;
        }
    }

    if count == 0 {
        return None;
    }
    Some(ColorStats {
        mean: [
            (sum[0] / count as f64) as f32,
            (sum[1] / count as f64) as f32,
            (sum[2] / count as f64) as f32,
        ],
        sample_count: count,
    })
}

/// True if any mask pixel within `margin` (Chebyshev distance) of
/// `(cx, cy)`, in the edge-canonical coordinate space. Used to carve the
/// subject-plus-safety-margin out of the background sampling strip.
///
/// Brute-force over a `margin`-sized window; fine at the scope this
/// runs at (a `strip_px`-wide band, only for pass-2-flagged instances).
/// Swap for a distance transform first if this ever shows up in a
/// profile.
fn mask_is_near(mask_view: &EdgeView<Mask>, cx: u32, cy: u32, margin: u32) -> bool {
    let x0 = cx.saturating_sub(margin);
    let y0 = cy.saturating_sub(margin);
    let x1 = (cx + margin).min(mask_view.width().saturating_sub(1));
    let y1 = (cy + margin).min(mask_view.height().saturating_sub(1));
    (y0..=y1).any(|y| (x0..=x1).any(|x| mask_view.get(x, y)))
}
