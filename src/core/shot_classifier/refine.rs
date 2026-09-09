//! Pass 2: pixel-perfect edge refinement.
//!
//! Only call [`refine_edge`] for edges [`super::gate::gate`] already
//! flagged — this does the real work pass 1 exists to avoid paying for
//! on every instance:
//!
//! 1. Intersect the working-resolution mask with the border band — not
//!    the whole mask, just the slice near this edge — to find where
//!    along the edge the subject actually reaches.
//! 2. Map that region back to the original image and crop it at full
//!    resolution. This is an in-memory crop; the region is also reported
//!    on [`EdgeRefinement::crop_region`] for callers that want to export
//!    or visualize it.
//! 3. Build a trimap from the upsampled working-resolution mask:
//!    interior = foreground, exterior = background, a ring around the
//!    boundary = unknown (this ring is exactly where the upsampling
//!    made the mask inaccurate).
//! 4. Classify each unknown pixel by color distance to sampled
//!    foreground vs. background color, at full resolution.
//! 5. The refined mask, not the raw BiRefNet mask, says whether the
//!    boundary reaches the border at all.
//! 6. A contact that reaches the border still has to earn the verdict:
//!    [`is_graze`] rejects the ones where the silhouette runs along the
//!    edge rather than off it.

use super::geometry::{Edge, EdgeView, Grid, Rect};
use super::sampling::{sample_background_color, ColorStats};
use super::segmentation::{Instance, Mask};
use image::{Rgb, RgbImage};

/// Everything pass 2 needs for one instance.
pub struct RefinementInput<'a> {
    /// The preprocessor's working-resolution image — what the
    /// segmentation model actually ran on.
    pub working_image: &'a RgbImage,
    /// The full original-resolution image, prior to the preprocessor's
    /// resize.
    pub original_image: &'a RgbImage,
    /// Detected instance; `mask`/`bbox` are in `working_image`'s
    /// coordinate space.
    pub instance: &'a Instance,
}

/// Tuning knobs, expressed in working-resolution pixels so they stay
/// meaningful regardless of the original image's size.
#[derive(Debug, Clone, Copy)]
pub struct RefineParams {
    /// Width of the border band pass 1 already gated on.
    pub band_px: u32,
    /// Safety margin excluded around the mask when sampling background color.
    pub safety_px: u32,
    /// Extra context grown around the band-intersection region before
    /// cropping, so the samplers have enough pixels to work with.
    pub context_px: u32,
    /// Narrow band of the graze test, compared against `band_px`.
    pub narrow_band_px: u32,
    /// Below this narrow-to-wide coverage ratio, the contact is a graze
    /// and the edge reports as not touching.
    pub graze_ratio_max: f32,
}

impl Default for RefineParams {
    fn default() -> Self {
        RefineParams {
            band_px: 20,
            safety_px: 10,
            context_px: 30,
            narrow_band_px: 5,
            graze_ratio_max: 0.80,
        }
    }
}

/// Result of refining one edge for one instance.
#[derive(Debug, Clone)]
pub struct EdgeRefinement {
    pub edge: Edge,
    /// Pixel-perfect verdict: does the refined subject boundary reach
    /// the actual image border on this edge?
    pub touching: bool,
    /// The region refinement worked in, in original-image coordinates.
    /// Not used in `touching`; exposed for callers that export or
    /// visualize the refined area.
    pub crop_region: Rect,
}

pub fn refine_edge(input: &RefinementInput, edge: Edge, params: RefineParams) -> EdgeRefinement {
    let mask = &input.instance.mask;
    let scale_x = input.original_image.width() as f32 / input.working_image.width() as f32;
    let scale_y = input.original_image.height() as f32 / input.working_image.height() as f32;
    let scale = scale_x.max(scale_y);

    // Step 1: intersect the mask with the border band, in edge-canonical
    // working-resolution space.
    let mask_view = EdgeView::new(mask, edge);
    let band_h = params.band_px.min(mask_view.height());
    let mut min_cx: Option<u32> = None;
    let mut max_cx = 0u32;
    let mut max_cy = 0u32;
    for cy in 0..band_h {
        for cx in 0..mask_view.width() {
            if mask_view.get(cx, cy) {
                min_cx = Some(min_cx.map_or(cx, |m| m.min(cx)));
                max_cx = max_cx.max(cx);
                max_cy = max_cy.max(cy);
            }
        }
    }

    let Some(min_cx) = min_cx else {
        // Pass 1 flagged the mask's bbox as near this edge, but the mask
        // itself doesn't actually reach the band — nothing to refine.
        return EdgeRefinement {
            edge,
            touching: false,
            crop_region: Rect { x: 0, y: 0, w: 0, h: 0 },
        };
    };

    let roi_canonical = Rect::from_bounds(min_cx, 0, max_cx, max_cy);

    // Step 2a: map back to working-image coordinates.
    let (ox0, oy0) = mask_view.to_original(roi_canonical.x, roi_canonical.y);
    let (ox1, oy1) = mask_view.to_original(
        roi_canonical.x + roi_canonical.w.saturating_sub(1),
        roi_canonical.y + roi_canonical.h.saturating_sub(1),
    );
    let working_roi = Rect::from_bounds(ox0.min(ox1), oy0.min(oy1), ox0.max(ox1), oy0.max(oy1))
        .expanded(params.context_px, input.working_image.width(), input.working_image.height());

    // Step 2b: rescale to full original resolution and crop there.
    let original_roi = working_roi.scaled(scale_x, scale_y).expanded(
        0,
        input.original_image.width(),
        input.original_image.height(),
    );
    let crop = crop_image(input.original_image, original_roi);

    // Step 3: trimap from the working-resolution mask, upsampled into
    // crop coordinates.
    let upsampled_mask = mask.upsample_region_nearest(working_roi, crop.width(), crop.height());
    let ring_px = ((params.safety_px as f32) * scale).round().max(2.0) as u32;
    let trimap = Trimap::from_mask(&upsampled_mask, ring_px);

    // Step 4: sample fg/bg color, then classify the unknown ring.
    let band_px_full = ((params.band_px as f32) * scale).round().max(1.0) as u32;
    let safety_px_full = ((params.safety_px as f32) * scale).round().max(1.0) as u32;
    let bg = sample_background_color(&crop, &upsampled_mask, edge, band_px_full, safety_px_full);
    let fg = sample_foreground_color(&crop, &trimap);

    let refined = match (fg, bg) {
        (Some(fg), Some(bg)) => trimap.resolve(&crop, fg, bg),
        // Not enough samples to refine confidently — fall back to the
        // upsampled mask rather than guessing at a verdict.
        _ => upsampled_mask,
    };

    // Step 5: the refined mask says whether the boundary reaches the
    // actual border (canonical row/col 0) anywhere along this edge...
    let refined_view = EdgeView::new(&refined, edge);
    let reaches_border = (0..refined_view.width()).any(|cx| refined_view.get(cx, 0));

    // ...and step 6 asks whether that contact is a real bleed-off or only
    // a graze. Reaching the border is necessary but not sufficient: a hem
    // curving down to kiss the bottom edge reaches it just as surely as a
    // subject the frame cuts in half, and only the second one gives R2 an
    // edge to anchor a crop against.
    let narrow_px_full = ((params.narrow_band_px as f32) * scale).round().max(1.0) as u32;
    let touching = reaches_border
        && !is_graze(&refined, edge, band_px_full, narrow_px_full, params.graze_ratio_max);

    EdgeRefinement { edge, touching, crop_region: original_roi }
}

/// Does the subject merely graze this edge rather than bleed off it?
///
/// A subject the frame truncates holds the same coverage at every depth —
/// its silhouette meets the border head-on and simply carries on past it.
/// One that grazes runs *along* the border and curves away, so coverage
/// grows the deeper you look. Comparing mean coverage in a narrow band
/// against a wide one turns that into a single scale-free number: about
/// 0.61 for a graze, 0.93 and up for a real bleed-off.
///
/// Measured on the *refined* mask, in crop coordinates, so `wide` and
/// `narrow` arrive already scaled to full resolution. It has to be the
/// refined mask: where matting moved the boundary, the raw mask's profile
/// describes a silhouette that is no longer the one being judged — a raw
/// mask stopping short of a border its subject really reaches has zero
/// coverage in the narrow band and would read as a graze every time.
///
/// The ratio does not care how wide a column range it is taken over — the
/// width cancels — so the crop spanning only the contact region rather
/// than the whole edge leaves it unchanged.
fn is_graze(mask: &Mask, edge: Edge, wide_px: u32, narrow_px: u32, ratio_max: f32) -> bool {
    let view = EdgeView::new(mask, edge);
    let wide = wide_px.min(view.height());
    let narrow = narrow_px.min(wide);
    // Degenerate bands can't express a ratio; say "not a graze" so the
    // border contact stands on its own, as it did before this test.
    if narrow == 0 || narrow == wide {
        return false;
    }

    let per_row: Vec<f32> = (0..wide)
        .map(|cy| (0..view.width()).filter(|&cx| view.get(cx, cy)).count() as f32)
        .collect();
    let mean = |rows: &[f32]| rows.iter().sum::<f32>() / rows.len() as f32;

    let wide_mean = mean(&per_row);
    if wide_mean <= 0.0 {
        return false;
    }
    mean(&per_row[..narrow as usize]) / wide_mean < ratio_max
}

fn crop_image(image: &RgbImage, rect: Rect) -> RgbImage {
    image::imageops::crop_imm(image, rect.x, rect.y, rect.w.max(1), rect.h.max(1)).to_image()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrimapLabel {
    Foreground,
    Background,
    Unknown,
}

struct Trimap {
    width: u32,
    height: u32,
    labels: Vec<TrimapLabel>,
}

impl Trimap {
    /// `ring_px` is the width of the "unknown" ring straddling the mask
    /// boundary, in crop-resolution pixels — this ring is exactly where
    /// the working-resolution mask's upsampling made it inaccurate.
    fn from_mask(mask: &Mask, ring_px: u32) -> Trimap {
        let (w, h) = (mask.width, mask.height);
        let mut labels = vec![TrimapLabel::Background; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let inside = mask.get(x, y);
                let near_boundary = (0..=ring_px).any(|d| {
                    [
                        x.checked_sub(d).map(|nx| mask.get(nx, y)),
                        Some(mask.get((x + d).min(w - 1), y)),
                        y.checked_sub(d).map(|ny| mask.get(x, ny)),
                        Some(mask.get(x, (y + d).min(h - 1))),
                    ]
                    .into_iter()
                    .flatten()
                    .any(|v| v != inside)
                });
                labels[(y * w + x) as usize] = if near_boundary {
                    TrimapLabel::Unknown
                } else if inside {
                    TrimapLabel::Foreground
                } else {
                    TrimapLabel::Background
                };
            }
        }
        Trimap { width: w, height: h, labels }
    }

    fn get(&self, x: u32, y: u32) -> TrimapLabel {
        self.labels[(y * self.width + x) as usize]
    }

    /// Classifies every "unknown" pixel by color distance to the sampled
    /// foreground/background distributions, producing a refined binary
    /// mask at the trimap's (crop) resolution.
    fn resolve(&self, image: &RgbImage, fg: ColorStats, bg: ColorStats) -> Mask {
        let mut data = vec![0u8; (self.width * self.height) as usize];
        for y in 0..self.height {
            for x in 0..self.width {
                let set = match self.get(x, y) {
                    TrimapLabel::Foreground => true,
                    TrimapLabel::Background => false,
                    TrimapLabel::Unknown => {
                        let px = *image.get_pixel(x, y);
                        fg.distance_sq(px) <= bg.distance_sq(px)
                    }
                };
                if set {
                    data[(y * self.width + x) as usize] = 255;
                }
            }
        }
        Mask { width: self.width, height: self.height, data }
    }
}

/// Mirrors `sample_background_color`'s approach but samples the mask's
/// confident interior instead (the trimap's `Foreground` label already
/// excludes the boundary ring, so no extra margin needed here).
fn sample_foreground_color(image: &RgbImage, trimap: &Trimap) -> Option<ColorStats> {
    let mut sum = [0f64; 3];
    let mut count = 0u32;
    for y in 0..trimap.height {
        for x in 0..trimap.width {
            if trimap.get(x, y) == TrimapLabel::Foreground {
                let Rgb(px) = *image.get_pixel(x, y);
                sum[0] += px[0] as f64;
                sum[1] += px[1] as f64;
                sum[2] += px[2] as f64;
                count += 1;
            }
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
