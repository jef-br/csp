//! Backdrop trim: the step between the colour optimiser and BiRefNet.
//!
//! BiRefNet at 384px sometimes returns nothing for a product that sits small in a large empty frame,
//! and finds it once the frame is tightened around it. This crops the empty backdrop away before the
//! model runs, in three steps, on the optimised image:
//!
//! 1. **Corner trim.** For each of the four corner colours, trim in from every side while the rows and
//!    columns match that colour. This removes false edges, such as white side bars added to a coloured
//!    backdrop.
//! 2. **Edge-colour trim.** Take every colour on the edge of the step-1 result and trim again, with a
//!    tolerance for noise and soft lighting. This peels the real backdrop, also where it fades.
//! 3. **Margin.** Grow the box back out by a share of its own size (more when more was trimmed), never
//!    past the image, so the product's edge verdict (EIX) cannot change.
//!
//! The model runs on the crop and its mask is pasted back into the full frame (zero outside the
//! crop), so every stage after it sees an ordinary full-frame mask.
//!
//! **When the trim is wrong.** Where the product is nearly the colour of the backdrop (a white garment
//! on white) the trim can cut into it. [`cut_sides`] spots that from the mask: a mask running into a
//! crop edge that is not the image edge may continue beyond it, and [`grow`] widens that side. If it
//! still does, or the trim leaves too little, the caller uses the untrimmed image.

use crate::core::config::{
    TRIM_CORNER_TOL, TRIM_CUT_BAND_PX, TRIM_CUT_FRACTION, TRIM_EDGE_SPREAD, TRIM_EDGE_TOL_BUCKETS,
    TRIM_GROW, TRIM_MARGIN_MAX, TRIM_MARGIN_MIN, TRIM_MIN_AREA, TRIM_MIN_SIDE, TRIM_OUTLIER,
};
use crate::core::shot_classifier::geometry::Rect;
use crate::core::shot_classifier::Mask;
use image::{imageops, RgbImage};

/// `(x0, y0, x1, y1)`, `x1`/`y1` exclusive. A box with no area has `x1 <= x0` or `y1 <= y0`.
type Bounds = (u32, u32, u32, u32);

/// The crop to run the model on, or `None` when the whole frame should be used: nothing worth
/// trimming, or a trim that leaves too little to be trusted.
pub fn plan(img: &RgbImage) -> Option<Rect> {
    let (w, h) = img.dimensions();
    let usable = |b: Bounds| {
        let (bw, bh) = (b.2.saturating_sub(b.0), b.3.saturating_sub(b.1));
        bw >= TRIM_MIN_SIDE && bh >= TRIM_MIN_SIDE && (bw as f32 * bh as f32) >= TRIM_MIN_AREA * (w as f32 * h as f32)
    };
    let after_corners = corner_trim(img);
    // Step 2 learns the colours on the box's edge as backdrop. When the product fills the box (a
    // rectangular product), that edge is the product itself and step 2 would trim it all away: keep
    // step 1's box then rather than lose the trim.
    let after_edges = edge_trim(img, after_corners);
    let (x0, y0, x1, y1) = if usable(after_edges) { after_edges } else { after_corners };
    if !usable((x0, y0, x1, y1)) {
        return None;
    }
    let (bw, bh) = (x1 - x0, y1 - y0);

    let trimmed_share = 1.0 - (bw as f32 * bh as f32) / (w as f32 * h as f32);
    let m = margin(trimmed_share);
    let (mx, my) = ((m * bw as f32).round() as u32, (m * bh as f32).round() as u32);
    let (gx0, gy0) = (x0.saturating_sub(mx), y0.saturating_sub(my));
    let (gx1, gy1) = ((x1 + mx).min(w), (y1 + my).min(h));
    let crop = Rect { x: gx0, y: gy0, w: gx1 - gx0, h: gy1 - gy0 };
    (crop != Rect { x: 0, y: 0, w, h }).then_some(crop)
}

/// Step 3's margin as a share of the trimmed box: `TRIM_MARGIN_MIN` when nothing was trimmed, rising
/// to `TRIM_MARGIN_MAX` as the trimmed share of the frame approaches 1.
fn margin(trimmed_share: f32) -> f32 {
    TRIM_MARGIN_MIN + (TRIM_MARGIN_MAX - TRIM_MARGIN_MIN) * trimmed_share.clamp(0.0, 1.0)
}

/// Step 1: one trim per corner colour, each starting from what the last left.
fn corner_trim(img: &RgbImage) -> Bounds {
    let (w, h) = img.dimensions();
    let k = 3.min(w).min(h);
    let corner = |x0: u32, y0: u32| {
        let mut px: Vec<[u8; 3]> = Vec::with_capacity((k * k) as usize);
        for y in y0..y0 + k {
            for x in x0..x0 + k {
                px.push(img.get_pixel(x, y).0);
            }
        }
        median(&px)
    };
    let corners = [corner(0, 0), corner(w - k, 0), corner(w - k, h - k), corner(0, h - k)];

    let mut b = (0, 0, w, h);
    for c in corners {
        b = peel(|x, y| distance(img.get_pixel(x, y).0, c) <= TRIM_CORNER_TOL, b);
        if b.2 <= b.0 + 1 || b.3 <= b.1 + 1 {
            break;
        }
    }
    b
}

/// Step 2: trim again, treating every colour found on the edge of `b` as backdrop (within a
/// tolerance). Edge pixels far from the median edge colour are left out, so a product touching the
/// edge is not learned as backdrop.
fn edge_trim(img: &RgbImage, b: Bounds) -> Bounds {
    let (x0, y0, x1, y1) = b;
    if x1 < x0 + 4 || y1 < y0 + 4 {
        return b;
    }
    let mut ring: Vec<[u8; 3]> = Vec::with_capacity(2 * ((x1 - x0) + (y1 - y0)) as usize);
    for x in x0..x1 {
        ring.push(img.get_pixel(x, y0).0);
        ring.push(img.get_pixel(x, y1 - 1).0);
    }
    for y in y0..y1 {
        ring.push(img.get_pixel(x0, y).0);
        ring.push(img.get_pixel(x1 - 1, y).0);
    }
    let typical = median(&ring);

    // A 64x64x64 table of 4-level colour buckets; every bucket within the tolerance of an edge colour is backdrop.
    let bucket = |c: [u8; 3]| (usize::from(c[0] >> 2) * 64 + usize::from(c[1] >> 2)) * 64 + usize::from(c[2] >> 2);
    let mut backdrop = vec![false; 64 * 64 * 64];
    let r = TRIM_EDGE_TOL_BUCKETS;
    for c in ring.iter().filter(|c| distance(**c, typical) <= TRIM_EDGE_SPREAD) {
        let q = [i32::from(c[0] >> 2), i32::from(c[1] >> 2), i32::from(c[2] >> 2)];
        for dr in -r..=r {
            for dg in -r..=r {
                for db in -r..=r {
                    let (a, g, bl) = ((q[0] + dr).clamp(0, 63), (q[1] + dg).clamp(0, 63), (q[2] + db).clamp(0, 63));
                    backdrop[((a * 64 + g) * 64 + bl) as usize] = true;
                }
            }
        }
    }
    peel(|x, y| backdrop[bucket(img.get_pixel(x, y).0)], b)
}

/// Trim rows and columns off the sides of `b` while they are backdrop (all but `TRIM_OUTLIER` of
/// them), until every side holds something else. Returns an empty box when all of `b` is backdrop.
fn peel(is_backdrop: impl Fn(u32, u32) -> bool, mut b: Bounds) -> Bounds {
    loop {
        let (x0, y0, x1, y1) = b;
        if x1 <= x0 || y1 <= y0 {
            return (x0, y0, x0, y0);
        }
        let (bw, bh) = ((x1 - x0) as usize, (y1 - y0) as usize);
        let mut row_hits = vec![0u32; bh];
        let mut col_hits = vec![0u32; bw];
        for y in y0..y1 {
            for x in x0..x1 {
                if is_backdrop(x, y) {
                    row_hits[(y - y0) as usize] += 1;
                    col_hits[(x - x0) as usize] += 1;
                }
            }
        }
        let need_row = ((1.0 - TRIM_OUTLIER) * bw as f32).ceil() as u32;
        let need_col = ((1.0 - TRIM_OUTLIER) * bh as f32).ceil() as u32;
        let row_is_backdrop: Vec<bool> = row_hits.iter().map(|&n| n >= need_row).collect();
        let col_is_backdrop: Vec<bool> = col_hits.iter().map(|&n| n >= need_col).collect();
        let (Some(top), Some(left)) =
            (row_is_backdrop.iter().position(|&v| !v), col_is_backdrop.iter().position(|&v| !v))
        else {
            return (x0, y0, x0, y0);
        };
        let bottom = row_is_backdrop.iter().rev().position(|&v| !v).unwrap_or(0);
        let right = col_is_backdrop.iter().rev().position(|&v| !v).unwrap_or(0);
        let next = (x0 + left as u32, y0 + top as u32, x1 - right as u32, y1 - bottom as u32);
        if next == b {
            return b;
        }
        b = next;
    }
}

fn distance(a: [u8; 3], b: [u8; 3]) -> i32 {
    (0..3).map(|c| (i32::from(a[c]) - i32::from(b[c])).abs()).max().unwrap_or(0)
}

/// Per-channel median.
fn median(px: &[[u8; 3]]) -> [u8; 3] {
    let mut out = [0u8; 3];
    for (c, slot) in out.iter_mut().enumerate() {
        let mut v: Vec<u8> = px.iter().map(|p| p[c]).collect();
        v.sort_unstable();
        *slot = v[v.len() / 2];
    }
    out
}

/// The part of `img` the model should see.
pub fn crop_view(img: &RgbImage, crop: Rect) -> RgbImage {
    imageops::crop_imm(img, crop.x, crop.y, crop.w, crop.h).to_image()
}

/// Which sides of `crop` the mask runs into, as `[left, top, right, bottom]`. `mask` is the crop's own
/// mask (same size as the crop) and `canvas` the full frame: a side lying on the frame edge is never
/// reported, because the subject ending there is the true edge, not a cut.
pub fn cut_sides(mask: &Mask, canvas: (u32, u32), crop: Rect) -> [bool; 4] {
    let (w, h) = (mask.width, mask.height);
    let band = TRIM_CUT_BAND_PX;
    let column = |x: u32| (0..h).filter(|&y| mask.get(x, y)).count() as f32 / h.max(1) as f32;
    let row = |y: u32| (0..w).filter(|&x| mask.get(x, y)).count() as f32 / w.max(1) as f32;
    let runs_into = |share: f32| share >= TRIM_CUT_FRACTION;
    [
        crop.x > 0 && (0..band.min(w)).any(|x| runs_into(column(x))),
        crop.y > 0 && (0..band.min(h)).any(|y| runs_into(row(y))),
        crop.x + crop.w < canvas.0 && (w.saturating_sub(band)..w).any(|x| runs_into(column(x))),
        crop.y + crop.h < canvas.1 && (h.saturating_sub(band)..h).any(|y| runs_into(row(y))),
    ]
}

/// `crop` with each side in `cut` pushed outwards by `TRIM_GROW` of the crop's size, within `canvas`.
pub fn grow(crop: Rect, cut: [bool; 4], canvas: (u32, u32)) -> Rect {
    let (gx, gy) = ((TRIM_GROW * crop.w as f32) as u32, (TRIM_GROW * crop.h as f32) as u32);
    let x0 = if cut[0] { crop.x.saturating_sub(gx) } else { crop.x };
    let y0 = if cut[1] { crop.y.saturating_sub(gy) } else { crop.y };
    let x1 = if cut[2] { (crop.x + crop.w + gx).min(canvas.0) } else { crop.x + crop.w };
    let y1 = if cut[3] { (crop.y + crop.h + gy).min(canvas.1) } else { crop.y + crop.h };
    Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
}

/// Put the crop's mask back where it came from in a `w` x `h` frame; everything outside is background.
pub fn paste_back(mask: &Mask, crop: Rect, w: u32, h: u32) -> Mask {
    let mut data = vec![0u8; w as usize * h as usize];
    for y in 0..mask.height.min(h.saturating_sub(crop.y)) {
        for x in 0..mask.width.min(w.saturating_sub(crop.x)) {
            if mask.get(x, y) {
                data[((y + crop.y) * w + x + crop.x) as usize] = 255;
            }
        }
    }
    Mask { width: w, height: h, data }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    const WHITE: Rgb<u8> = Rgb([245, 245, 245]);
    const DARK: Rgb<u8> = Rgb([30, 40, 90]);

    /// `w` x `h` backdrop with a filled `product` rectangle (`x, y, w, h`).
    fn scene(w: u32, h: u32, backdrop: Rgb<u8>, product: (u32, u32, u32, u32)) -> RgbImage {
        let mut img = RgbImage::from_pixel(w, h, backdrop);
        for y in product.1..product.1 + product.3 {
            for x in product.0..product.0 + product.2 {
                img.put_pixel(x, y, DARK);
            }
        }
        img
    }

    /// The box around a lone product on plain paper contains it, with room, and is far smaller than the frame.
    #[test]
    fn a_product_on_plain_paper_is_cropped_to_itself_plus_a_margin() {
        let crop = plan(&scene(400, 600, WHITE, (150, 250, 100, 100))).expect("there is backdrop to trim");
        assert!(crop.x <= 150 && crop.y <= 250 && crop.x + crop.w >= 250 && crop.y + crop.h >= 350, "{crop:?}");
        assert!(crop.w < 200 && crop.h < 200, "{crop:?}");
    }

    /// Step 3 grows the box by 10-20% of its size per side, never past the image.
    #[test]
    fn the_margin_is_ten_to_twenty_percent_of_the_box() {
        assert!((margin(0.0) - 0.10).abs() < 1e-6);
        assert!((margin(1.0) - 0.20).abs() < 1e-6);
        assert!(margin(0.5) > margin(0.1));
        // 200x200 product on 1000x1000 paper trims 96% of the area, so ~19.6% each side: 200 + 2*39.
        let crop = plan(&scene(1000, 1000, WHITE, (400, 400, 200, 200))).unwrap();
        assert!((crop.w as f32 - 278.0).abs() <= 2.0, "{crop:?}");
    }

    /// A product touching the left edge keeps that side of the frame: nothing to trim there.
    #[test]
    fn a_product_touching_an_edge_is_not_trimmed_on_that_side() {
        let crop = plan(&scene(400, 600, WHITE, (0, 250, 100, 100))).expect("the other sides trim");
        assert_eq!(crop.x, 0, "{crop:?}");
        assert!(crop.y > 0 && crop.x + crop.w < 400, "{crop:?}");
    }

    /// Step 1 removes white side bars first, so step 2 then sees the true (blue) backdrop on its edge.
    #[test]
    fn false_side_bars_are_removed_before_the_real_backdrop() {
        let mut img = RgbImage::from_pixel(400, 300, Rgb([20, 120, 200]));
        for y in 0..300 {
            for x in (0..60).chain(340..400) {
                img.put_pixel(x, y, Rgb([255, 255, 255]));
            }
        }
        for y in 120..180 {
            for x in 180..220 {
                img.put_pixel(x, y, DARK);
            }
        }
        let crop = plan(&img).expect("bars and backdrop both trim");
        assert!(crop.x > 60 && crop.x + crop.w < 340, "the bars must be gone: {crop:?}");
        assert!(crop.x <= 180 && crop.x + crop.w >= 220 && crop.y <= 120 && crop.y + crop.h >= 180, "{crop:?}");
    }

    /// A backdrop that fades (soft lighting) is still trimmed: step 2 learns every colour on the edge.
    #[test]
    fn a_fading_backdrop_is_trimmed() {
        let mut img = RgbImage::new(400, 300);
        for y in 0..300 {
            for x in 0..400 {
                let v = (235 + x * 15 / 400) as u8; // 235..250 left to right
                img.put_pixel(x, y, Rgb([v, v, v]));
            }
        }
        for y in 100..200 {
            for x in 150..250 {
                img.put_pixel(x, y, DARK);
            }
        }
        let crop = plan(&img).expect("the fade must not defeat the trim");
        assert!(crop.w < 250 && crop.h < 200, "{crop:?}");
    }

    /// A uniform frame has no product to keep: use the whole frame.
    #[test]
    fn a_blank_frame_is_left_alone() {
        assert_eq!(plan(&RgbImage::from_pixel(200, 200, WHITE)), None);
    }

    /// A product that fills the frame leaves nothing to trim.
    #[test]
    fn a_full_frame_product_is_left_alone() {
        assert_eq!(plan(&scene(100, 100, WHITE, (0, 0, 100, 100))), None);
    }

    /// A trim that would leave a sliver is not trusted.
    #[test]
    fn a_trim_that_leaves_almost_nothing_is_not_trusted() {
        assert_eq!(plan(&scene(400, 400, WHITE, (200, 200, 6, 6))), None);
    }

    fn mask_with(w: u32, h: u32, on: impl Fn(u32, u32) -> bool) -> Mask {
        let mut data = vec![0u8; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                if on(x, y) {
                    data[(y * w + x) as usize] = 255;
                }
            }
        }
        Mask { width: w, height: h, data }
    }

    /// A mask running off the bottom of the crop is a cut when the crop's bottom is inside the frame...
    #[test]
    fn a_mask_running_into_an_inner_crop_edge_is_a_cut() {
        let crop = Rect { x: 50, y: 50, w: 100, h: 100 };
        let mask = mask_with(100, 100, |x, y| (30..70).contains(&x) && y >= 40);
        assert_eq!(cut_sides(&mask, (300, 300), crop), [false, false, false, true]);
    }

    /// ...and not when that edge is the frame's own edge: the subject really ends there.
    #[test]
    fn a_mask_running_into_the_frame_edge_is_not_a_cut() {
        let crop = Rect { x: 50, y: 200, w: 100, h: 100 };
        let mask = mask_with(100, 100, |x, y| (30..70).contains(&x) && y >= 40);
        assert_eq!(cut_sides(&mask, (300, 300), crop), [false, false, false, false]);
    }

    /// A mask well inside the crop is no cut, and a stray pixel on the edge does not count.
    #[test]
    fn a_mask_inside_the_crop_is_no_cut() {
        let crop = Rect { x: 50, y: 50, w: 100, h: 100 };
        let mut mask = mask_with(100, 100, |x, y| (30..70).contains(&x) && (30..70).contains(&y));
        mask.data[99 * 100 + 50] = 255;
        assert_eq!(cut_sides(&mask, (300, 300), crop), [false; 4]);
    }

    /// Growing pushes only the cut sides outwards, and stays inside the frame.
    #[test]
    fn growing_moves_only_the_cut_side_and_stays_in_the_frame() {
        let crop = Rect { x: 50, y: 50, w: 100, h: 100 };
        let g = grow(crop, [false, false, false, true], (300, 300));
        assert_eq!((g.x, g.y, g.w), (50, 50, 100));
        assert_eq!(g.h, 125);
        let g = grow(crop, [true, true, true, true], (160, 160));
        assert_eq!((g.x, g.y, g.x + g.w, g.y + g.h), (25, 25, 160, 160));
    }

    /// The crop's mask lands where the crop came from; the rest of the frame is background.
    #[test]
    fn the_mask_is_pasted_back_where_the_crop_came_from() {
        let crop = Rect { x: 3, y: 2, w: 4, h: 4 };
        let mask = mask_with(4, 4, |x, y| x == 1 && y == 2);
        let full = paste_back(&mask, crop, 10, 8);
        assert_eq!((full.width, full.height), (10, 8));
        assert!(full.get(4, 4));
        assert_eq!(full.data.iter().filter(|&&v| v > 0).count(), 1);
    }
}
