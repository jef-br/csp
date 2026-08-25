//! Subject detection — the combined "best of both" detector.
//!
//! Keys on chroma + texture, never lightness (a cast shadow is a near-pure lightness change).
//! White-on-white is caught by the texture signal; hard-shadow edges are stripped by shape; the
//! background colour is fitted as a plane over the border ring; a Canny border flood-fill
//! corroborates but never introduces a region on its own. Non-flat backgrounds escalate to a
//! higher-resolution pass; a product that bleeds off-canvas falls back to the largest salient
//! square (see `saliency`).

use super::config::*;
use super::imgmath::{self, Integral, Plane};
use super::saliency;
use super::types::*;
use super::{clahe, imgutil};
use image::{GrayImage, Luma, RgbImage};
use imageproc::morphology::{close, open};
use imageproc::region_labelling::{connected_components, Connectivity};

/// Debug helper: return the studio-pass foreground mask upscaled to the source size.
pub fn debug_mask(rgb: &RgbImage) -> GrayImage {
    let (w, h) = (rgb.width(), rgb.height());
    let scale = (ANALYSIS_SIZE as f64 / w.max(h) as f64).min(1.0);
    let small = imgutil::downscale(rgb, scale);
    let (mask, _, _, _) = build_foreground_mask(&small, true, SWEEP_SPECKLE_KERNEL);
    image::imageops::resize(&mask, w, h, image::imageops::FilterType::Nearest)
}

/// Detect the subject in a full-resolution flattened RGB image (plus optional alpha).
pub fn detect(rgb: &RgbImage, alpha: Option<&GrayImage>) -> Detection {
    let (w, h) = (rgb.width() as i32, rgb.height() as i32);

    if let Some(a) = alpha {
        if let Some(det) = detect_from_alpha(a, w, h) {
            return det;
        }
    }

    // Studio pass at the analysis cap.
    let scale = (ANALYSIS_SIZE as f64 / w.max(h) as f64).min(1.0);
    let small = imgutil::downscale(rgb, scale);
    let pass = run_pass(&small, true, MIN_COMPONENT_AREA_RATIO, SWEEP_SPECKLE_KERNEL);

    // Escalate to a higher-res, stricter pass when the border ring says the background isn't a
    // flat sweep (real-life background).
    let pass = if pass.ring_residual > REALLIFE_RESIDUAL_THRESHOLD {
        let scale2 = (REALLIFE_ANALYSIS_SIZE as f64 / w.max(h) as f64).min(1.0);
        let small2 = imgutil::downscale(rgb, scale2);
        let mut p = run_pass(&small2, true, REALLIFE_MIN_COMPONENT_AREA_RATIO, 0);
        p.scale = scale2;
        p
    } else {
        let mut p = pass;
        p.scale = scale;
        p
    };

    // Detail shot: the product bleeds off the canvas / fills the frame. There's no clean box to
    // crop to, so take the largest salient square instead.
    let bleeds = pass.intersects.count() >= BLEED_EDGES || pass.ring_texture > SWEEP_TEXTURE_LIMIT;
    if bleeds {
        let sq = saliency::most_salient_square(rgb);
        return Detection {
            box_: sq,
            intersects: pass.intersects,
            kind: DetectionKind::SalientSquare,
            confidence: 1.0,
            hard_shadow_fraction: pass.hard_shadow_fraction,
        };
    }

    match pass.box_small {
        None => whole_frame(w, h, pass.intersects, pass.hard_shadow_fraction, 0.2),
        Some(b) => {
            let box_ = rescale_box(b, pass.scale, w, h);
            if box_.area() as f64 >= WHOLE_FRAME_FRACTION * (w as f64 * h as f64) {
                whole_frame(w, h, pass.intersects, pass.hard_shadow_fraction, 0.2)
            } else {
                Detection {
                    box_,
                    intersects: pass.intersects,
                    kind: DetectionKind::Subject,
                    confidence: pass.confidence.clamp(0.1, 1.0),
                    hard_shadow_fraction: pass.hard_shadow_fraction,
                }
            }
        }
    }
}

fn detect_from_alpha(alpha: &GrayImage, w: i32, h: i32) -> Option<Detection> {
    let (mut minx, mut miny, mut maxx, mut maxy) = (w, h, -1i32, -1i32);
    for y in 0..h {
        for x in 0..w {
            if alpha.get_pixel(x as u32, y as u32)[0] > 8 {
                minx = minx.min(x);
                miny = miny.min(y);
                maxx = maxx.max(x);
                maxy = maxy.max(y);
            }
        }
    }
    if maxx < 0 {
        return None;
    }
    let box_ = Box::new(minx, miny, maxx - minx + 1, maxy - miny + 1);
    let mask = alpha_mask(alpha);
    let intersects = edge_intersects(&mask);
    if box_.area() as f64 >= WHOLE_FRAME_FRACTION * (w as f64 * h as f64) {
        return Some(whole_frame(w, h, intersects, 0.0, 0.2));
    }
    Some(Detection {
        box_,
        intersects,
        kind: DetectionKind::Subject,
        confidence: 1.0,
        hard_shadow_fraction: 0.0,
    })
}

fn alpha_mask(alpha: &GrayImage) -> GrayImage {
    let mut m = GrayImage::new(alpha.width(), alpha.height());
    for (dst, src) in m.pixels_mut().zip(alpha.pixels()) {
        dst[0] = if src[0] > 8 { 255 } else { 0 };
    }
    m
}

struct Pass {
    box_small: Option<Box>,
    intersects: EdgeIntersects,
    confidence: f64,
    hard_shadow_fraction: f64,
    ring_residual: f64,
    ring_texture: f64,
    scale: f64,
}

fn run_pass(small: &RgbImage, use_clahe: bool, min_component_ratio: f64, speckle_kernel: i32) -> Pass {
    let (w, h) = (small.width() as usize, small.height() as usize);
    let (mask, hard_shadow_fraction, ring_residual, ring_texture) =
        build_foreground_mask(small, use_clahe, speckle_kernel);

    let box_small = significant_components_box(&mask, min_component_ratio);
    let intersects = edge_intersects(&mask);
    let confidence = match box_small {
        Some(b) => box_coverage(&mask, b),
        None => 0.0,
    };
    let _ = (w, h);
    Pass {
        box_small,
        intersects,
        confidence,
        hard_shadow_fraction,
        ring_residual,
        ring_texture,
        scale: 1.0,
    }
}

// Returns (mask 0/255, hard-shadow fraction, ring chroma residual, ring texture median).
fn build_foreground_mask(
    small: &RgbImage,
    use_clahe: bool,
    speckle_kernel: i32,
) -> (GrayImage, f64, f64, f64) {
    let (w, h) = (small.width() as usize, small.height() as usize);
    let (l, a, b) = imgmath::rgb_to_lab(small.as_raw(), w, h);

    // Texture: high-pass the (optionally CLAHE'd) lightness, then local std-dev via integral image.
    let l_det = if use_clahe {
        clahe::apply(&scale_to_255(&l), CLAHE_CLIP_LIMIT, CLAHE_TILE_SIZE)
    } else {
        scale_to_255(&l)
    };
    let blurred = imgmath::box_blur_gaussian(&l_det, TEXTURE_DETAIL_SIGMA);
    let mut detail = Plane::new(w, h);
    let mut detail_sq = Plane::new(w, h);
    for i in 0..(w * h) {
        let d = l_det.data[i] - blurred.data[i];
        detail.data[i] = d;
        detail_sq.data[i] = d * d;
    }
    let int_detail = Integral::build(&detail);
    let int_detail_sq = Integral::build(&detail_sq);
    let mut texture = Plane::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mean = int_detail.box_mean(x, y, TEXTURE_WINDOW);
            let mean_sq = int_detail_sq.box_mean(x, y, TEXTURE_WINDOW);
            texture.set(x, y, (mean_sq - mean * mean).max(0.0).sqrt() as f32);
        }
    }

    // Background chroma fitted as a plane over the border ring.
    let ring = ring_indices(w, h);
    let (bg_a, ring_a_vals, ring_coords) = fit_channel_over_ring(&a, &ring, w, h);
    let (bg_b, _ring_b_vals, _) = fit_channel_over_ring(&b, &ring, w, h);
    let _ = (ring_a_vals, ring_coords);

    let mut chroma_dist = Plane::new(w, h);
    for i in 0..(w * h) {
        let da = a.data[i] - bg_a.data[i];
        let db = b.data[i] - bg_b.data[i];
        chroma_dist.data[i] = (da * da + db * db).sqrt();
    }

    let ring_chroma: Vec<f32> = ring.iter().map(|&i| chroma_dist.data[i]).collect();
    let ring_tex: Vec<f32> = ring.iter().map(|&i| texture.data[i]).collect();
    let ring_residual = imgmath::median(&ring_chroma); // robust "typical" residual over the ring
    let ring_texture_med = imgmath::median(&ring_tex);

    let chroma_limit =
        CHROMA_FLOOR.max(OUTLIER_SPREAD_MULTIPLIER * imgmath::robust_spread(&ring_chroma));
    let texture_limit = TEXTURE_FLOOR
        .max(ring_texture_med + OUTLIER_SPREAD_MULTIPLIER * imgmath::robust_spread(&ring_tex));

    // Chroma mask, and texture-only pixels (texture without chroma support = candidate shadow edge).
    let mut chroma_mask = GrayImage::new(w as u32, h as u32);
    let mut texture_only = GrayImage::new(w as u32, h as u32);
    for i in 0..(w * h) {
        let is_chroma = chroma_dist.data[i] as f64 > chroma_limit;
        let is_texture = texture.data[i] as f64 > texture_limit;
        chroma_mask.as_mut()[i] = if is_chroma { 255 } else { 0 };
        texture_only.as_mut()[i] = if is_texture && !is_chroma { 255 } else { 0 };
    }

    // Strip the thin edge of a hard shadow: real texture fills a 2D area and survives an open;
    // a shadow's silhouette is a thin line that does not.
    let opened_texture = open(&texture_only, imageproc::distance_transform::Norm::LInf, kradius(SHADOW_EDGE_KERNEL));
    let hard_shadow_fraction = stripped_fraction(&texture_only, &opened_texture);

    let mut mask = GrayImage::new(w as u32, h as u32);
    for i in 0..(w * h) {
        mask.as_mut()[i] = if chroma_mask.as_ref()[i] > 0 || opened_texture.as_ref()[i] > 0 {
            255
        } else {
            0
        };
    }

    // Hysteresis: grow the strong mask along connected weaker-but-real pixels. This recovers thin,
    // low-contrast appendages (a bag strap, a hanger) that sit above the hard threshold's floor but
    // are continuously connected to the confidently-detected product.
    let weak = weak_mask(&chroma_dist, &texture, chroma_limit, texture_limit, w, h);
    hysteresis_grow(&mut mask, &weak);

    // Canny border flood-fill corroboration: only extend regions the chroma/texture already flagged.
    let enclosed = canny_enclosed_region(small);
    corroborate(&mut mask, &enclosed);

    // Kill speckle, then bridge separately-detected parts into one region.
    if speckle_kernel > 0 {
        mask = open(&mask, imageproc::distance_transform::Norm::LInf, kradius(speckle_kernel));
    }
    let bridge = ((0.02 * w.min(h) as f64) as i32).max(9) | 1;
    mask = open(&mask, imageproc::distance_transform::Norm::LInf, 2);
    mask = close(&mask, imageproc::distance_transform::Norm::LInf, kradius(bridge));
    mask = close(&mask, imageproc::distance_transform::Norm::LInf, kradius(bridge));

    (mask, hard_shadow_fraction, ring_residual, ring_texture_med)
}

fn scale_to_255(l: &Plane) -> Plane {
    // Lab L is already 0..100; map to 0..255 for CLAHE/high-pass.
    let mut out = Plane::new(l.w, l.h);
    for (o, &v) in out.data.iter_mut().zip(l.data.iter()) {
        *o = (v * 2.55).clamp(0.0, 255.0);
    }
    out
}

fn kradius(k: i32) -> u8 {
    (k / 2).clamp(1, 255) as u8
}

fn ring_indices(w: usize, h: usize) -> Vec<usize> {
    let band_y = ((h as f64 * BORDER_RING_FRACTION) as usize).max(2);
    let band_x = ((w as f64 * BORDER_RING_FRACTION) as usize).max(2);
    let mut out = Vec::new();
    for y in 0..h {
        let edge_row = y < band_y || y >= h - band_y;
        for x in 0..w {
            if edge_row || x < band_x || x >= w - band_x {
                out.push(y * w + x);
            }
        }
    }
    out
}

fn fit_channel_over_ring(
    ch: &Plane,
    ring: &[usize],
    w: usize,
    h: usize,
) -> (Plane, Vec<f32>, Vec<(f32, f32)>) {
    let hw = w as f32 / 2.0;
    let hh = h as f32 / 2.0;
    let coords: Vec<(f32, f32)> = ring
        .iter()
        .map(|&i| {
            let x = (i % w) as f32;
            let y = (i / w) as f32;
            ((x - hw) / hw, (y - hh) / hh)
        })
        .collect();
    let vals: Vec<f32> = ring.iter().map(|&i| ch.data[i]).collect();
    let (c0, c1, c2) = imgmath::fit_plane(&coords, &vals);

    let mut bg = Plane::new(w, h);
    for y in 0..h {
        let yn = (y as f32 - hh) / hh;
        for x in 0..w {
            let xn = (x as f32 - hw) / hw;
            bg.set(x, y, (c0 + c1 * xn as f64 + c2 * yn as f64) as f32);
        }
    }
    (bg, vals, coords)
}

// Pixels an edge boundary walls off from the frame border — candidate product. Corroboration only,
// so it runs at a coarse resolution (<=512px) and is upsampled back to the mask size.
const CANNY_ANALYSIS_CAP: u32 = 512;

fn canny_enclosed_region(small: &RgbImage) -> GrayImage {
    let (fw, fh) = (small.width(), small.height());
    let cap_scale = (CANNY_ANALYSIS_CAP as f64 / fw.max(fh) as f64).min(1.0);
    let coarse = imgutil::downscale(small, cap_scale);
    let enclosed = canny_enclosed_coarse(&coarse);
    if cap_scale >= 1.0 {
        enclosed
    } else {
        image::imageops::resize(&enclosed, fw, fh, image::imageops::FilterType::Nearest)
    }
}

fn canny_enclosed_coarse(small: &RgbImage) -> GrayImage {
    let gray = image::imageops::grayscale(small);
    let med = imgutil::median_u8(&gray);
    let low = ((1.0 - CANNY_SIGMA) * med).max(0.0) as f32;
    let high = ((1.0 + CANNY_SIGMA) * med).min(255.0) as f32;
    let edges = imageproc::edges::canny(&gray, low, high);
    let closed = close(&edges, imageproc::distance_transform::Norm::LInf, kradius(CANNY_CLOSE_KERNEL));

    // Free space = non-edge; flood from the border via connected components.
    let mut free = GrayImage::new(closed.width(), closed.height());
    for (d, s) in free.pixels_mut().zip(closed.pixels()) {
        d[0] = if s[0] == 0 { 255 } else { 0 };
    }
    let labels = connected_components(&free, Connectivity::Eight, Luma([0u8]));
    let (w, h) = (labels.width(), labels.height());
    let mut border = std::collections::HashSet::new();
    for x in 0..w {
        border.insert(labels.get_pixel(x, 0)[0]);
        border.insert(labels.get_pixel(x, h - 1)[0]);
    }
    for y in 0..h {
        border.insert(labels.get_pixel(0, y)[0]);
        border.insert(labels.get_pixel(w - 1, y)[0]);
    }
    border.remove(&0);

    let mut enclosed = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let lbl = labels.get_pixel(x, y)[0];
            // Enclosed = not reachable free-space from the border (includes the edge pixels).
            let is_free_border = free.get_pixel(x, y)[0] > 0 && border.contains(&lbl);
            enclosed.put_pixel(x, y, Luma([if is_free_border { 0 } else { 255 }]));
        }
    }
    enclosed
}

// Weak-threshold mask: pixels above a fraction of the strong chroma/texture limits.
fn weak_mask(chroma_dist: &Plane, texture: &Plane, chroma_limit: f64, texture_limit: f64, w: usize, h: usize) -> GrayImage {
    let cw = chroma_limit * HYSTERESIS_WEAK_FRACTION;
    let tw = texture_limit * HYSTERESIS_WEAK_FRACTION;
    let mut weak = GrayImage::new(w as u32, h as u32);
    for i in 0..(w * h) {
        let hit = chroma_dist.data[i] as f64 > cw || texture.data[i] as f64 > tw;
        weak.as_mut()[i] = if hit { 255 } else { 0 };
    }
    weak
}

// Absorb a weak component only when it is seeded by the strong mask AND does not balloon far
// beyond that seed — the flood guard keeps a faint appendage while rejecting a background flood.
fn hysteresis_grow(mask: &mut GrayImage, weak: &GrayImage) {
    let labels = connected_components(weak, Connectivity::Eight, Luma([0u8]));
    let (w, h) = (labels.width(), labels.height());

    let mut total_area: std::collections::HashMap<u32, i64> = std::collections::HashMap::new();
    let mut seed_area: std::collections::HashMap<u32, i64> = std::collections::HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let lbl = labels.get_pixel(x, y)[0];
            if lbl == 0 {
                continue;
            }
            *total_area.entry(lbl).or_insert(0) += 1;
            if mask.get_pixel(x, y)[0] > 0 {
                *seed_area.entry(lbl).or_insert(0) += 1;
            }
        }
    }

    let mut absorb = std::collections::HashSet::new();
    for (&lbl, &seed) in &seed_area {
        let total = total_area[&lbl] as f64;
        // Extra (non-seed) area this component would add, relative to its seed.
        let extra = total - seed as f64;
        if extra <= HYSTERESIS_FLOOD_CAP * seed as f64 {
            absorb.insert(lbl);
        }
    }

    for y in 0..h {
        for x in 0..w {
            let lbl = labels.get_pixel(x, y)[0];
            if lbl != 0 && absorb.contains(&lbl) {
                mask.put_pixel(x, y, Luma([255]));
            }
        }
    }
}

// Fold in enclosed components that touch an already-flagged pixel.
fn corroborate(mask: &mut GrayImage, enclosed: &GrayImage) {
    let labels = connected_components(enclosed, Connectivity::Eight, Luma([0u8]));
    let (w, h) = (labels.width(), labels.height());
    let mut touching = std::collections::HashSet::new();
    for y in 0..h {
        for x in 0..w {
            if mask.get_pixel(x, y)[0] > 0 {
                let lbl = labels.get_pixel(x, y)[0];
                if lbl != 0 {
                    touching.insert(lbl);
                }
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            let lbl = labels.get_pixel(x, y)[0];
            if lbl != 0 && touching.contains(&lbl) {
                mask.put_pixel(x, y, Luma([255]));
            }
        }
    }
}

fn significant_components_box(mask: &GrayImage, min_component_ratio: f64) -> Option<Box> {
    let labels = connected_components(mask, Connectivity::Eight, Luma([0u8]));
    let (w, h) = (labels.width() as usize, labels.height() as usize);
    let mut areas: std::collections::HashMap<u32, i64> = std::collections::HashMap::new();
    let mut bounds: std::collections::HashMap<u32, (i32, i32, i32, i32)> =
        std::collections::HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let lbl = labels.get_pixel(x as u32, y as u32)[0];
            if lbl == 0 {
                continue;
            }
            *areas.entry(lbl).or_insert(0) += 1;
            let e = bounds.entry(lbl).or_insert((x as i32, y as i32, x as i32, y as i32));
            e.0 = e.0.min(x as i32);
            e.1 = e.1.min(y as i32);
            e.2 = e.2.max(x as i32);
            e.3 = e.3.max(y as i32);
        }
    }
    if areas.is_empty() {
        return None;
    }
    let largest_area = *areas.values().max().unwrap() as f64;

    // Keep the main mass (>= ratio of the largest) plus any component big enough on its own
    // (absolute + fraction floors). This admits thin appendages — a bag strap, a hanger — that
    // fall under the ratio but are real product, while dropping speckle.
    let main_threshold = min_component_ratio * largest_area;
    let keep_floor = (MIN_COMPONENT_AREA_FRACTION * (w * h) as f64).max(MIN_COMPONENT_AREA_PIXELS);

    let mut main: Vec<u32> = Vec::new();
    let mut candidates: Vec<u32> = Vec::new();
    for (&lbl, &area) in &areas {
        let a = area as f64;
        if a >= main_threshold {
            main.push(lbl);
        } else if a >= keep_floor {
            candidates.push(lbl);
        }
    }
    if main.is_empty() {
        return None;
    }

    // Seed the union with the main components, then absorb nearby candidates (strap etc.) whose
    // bbox sits within a small gap of the growing union. Iterate to a fixed point.
    let gap = (0.03 * w.min(h) as f64) as i32;
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for lbl in &main {
        let &(bx0, by0, bx1, by1) = &bounds[lbl];
        x0 = x0.min(bx0);
        y0 = y0.min(by0);
        x1 = x1.max(bx1);
        y1 = y1.max(by1);
    }
    let mut changed = true;
    while changed {
        changed = false;
        candidates.retain(|lbl| {
            let &(bx0, by0, bx1, by1) = &bounds[lbl];
            let near_x = bx0 <= x1 + gap && bx1 >= x0 - gap;
            let near_y = by0 <= y1 + gap && by1 >= y0 - gap;
            if near_x && near_y {
                x0 = x0.min(bx0);
                y0 = y0.min(by0);
                x1 = x1.max(bx1);
                y1 = y1.max(by1);
                changed = true;
                false // absorbed; drop from candidates
            } else {
                true
            }
        });
    }
    Some(Box::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

fn edge_intersects(mask: &GrayImage) -> EdgeIntersects {
    let (w, h) = (mask.width(), mask.height());
    let cov_row = |y: u32| -> f64 {
        let mut c = 0;
        for x in 0..w {
            if mask.get_pixel(x, y)[0] > 0 {
                c += 1;
            }
        }
        c as f64 / w as f64
    };
    let cov_col = |x: u32| -> f64 {
        let mut c = 0;
        for y in 0..h {
            if mask.get_pixel(x, y)[0] > 0 {
                c += 1;
            }
        }
        c as f64 / h as f64
    };
    EdgeIntersects {
        top: cov_row(0) >= BLEED_CONTACT,
        bottom: cov_row(h - 1) >= BLEED_CONTACT,
        left: cov_col(0) >= BLEED_CONTACT,
        right: cov_col(w - 1) >= BLEED_CONTACT,
    }
}

fn box_coverage(mask: &GrayImage, b: Box) -> f64 {
    let mut count = 0i64;
    for y in b.y..b.bottom() {
        for x in b.x..b.right() {
            if mask.get_pixel(x as u32, y as u32)[0] > 0 {
                count += 1;
            }
        }
    }
    count as f64 / b.area().max(1) as f64
}

fn stripped_fraction(before: &GrayImage, after: &GrayImage) -> f64 {
    let area = (before.width() * before.height()) as f64;
    if area <= 0.0 {
        return 0.0;
    }
    let mut stripped = 0i64;
    for (b, a) in before.pixels().zip(after.pixels()) {
        if b[0] > 0 && a[0] == 0 {
            stripped += 1;
        }
    }
    stripped as f64 / area
}

fn rescale_box(b: Box, scale: f64, w: i32, h: i32) -> Box {
    if scale >= 1.0 {
        return clamp_box(b, w, h);
    }
    let x = ((b.x as f64 / scale).floor() as i32 - 1).max(0);
    let y = ((b.y as f64 / scale).floor() as i32 - 1).max(0);
    let bw = ((b.w as f64 / scale).ceil() as i32 + 2).min(w - x);
    let bh = ((b.h as f64 / scale).ceil() as i32 + 2).min(h - y);
    Box::new(x, y, bw.max(1), bh.max(1))
}

fn clamp_box(b: Box, w: i32, h: i32) -> Box {
    let x = b.x.clamp(0, w - 1);
    let y = b.y.clamp(0, h - 1);
    Box::new(x, y, b.w.min(w - x).max(1), b.h.min(h - y).max(1))
}

fn whole_frame(w: i32, h: i32, intersects: EdgeIntersects, shadow: f64, conf: f64) -> Detection {
    Detection {
        box_: Box::new(0, 0, w, h),
        intersects,
        kind: DetectionKind::WholeFrame,
        confidence: conf,
        hard_shadow_fraction: shadow,
    }
}
