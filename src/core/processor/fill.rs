//! Background fill: grow a crop to a square by stretching only the background bands. The product
//! band is never scaled. Mirrors the reference nine-slice background stretch (background-only).

use super::super::config::LOW_BACKGROUND_FRACTION;
use super::super::types::Box as GBox;
use super::geometry::Layout;
use image::{imageops, RgbImage};

/// Crop the original per `layout` and return a `side`×`side` square (native product pixels).
pub fn render(original: &RgbImage, layout: &Layout) -> RgbImage {
    let c = layout.crop;
    let crop = imageops::crop_imm(original, c.x as u32, c.y as u32, c.w as u32, c.h as u32).to_image();

    if layout.already_square {
        return ensure_square(crop, layout.side);
    }

    // Horizontal pass, then vertical pass (via rotate) — one implementation covers both axes.
    let widened = expand_width(&crop, layout.protected, layout.side);

    let prot_v = GBox::new(layout.protected.y, layout.protected.x, layout.protected.h, layout.protected.w);
    let rotated = imageops::rotate90(&widened);
    let rot_expanded = expand_width(&rotated, prot_v, layout.side);
    let out = imageops::rotate270(&rot_expanded);

    ensure_square(out, layout.side)
}

// Grow `img` horizontally to `target` width by stretching the background bands left/right of the
// protected product band. The product columns pass through untouched.
fn expand_width(img: &RgbImage, protected: GBox, target: i32) -> RgbImage {
    let cw = img.width() as i32;
    let ch = img.height();
    let extra = target - cw;
    if extra <= 0 {
        return img.clone();
    }

    let keep_start = protected.x.clamp(0, cw);
    let keep_end = (protected.x + protected.w).clamp(0, cw);
    let left_width = keep_start.max(0);
    let right_width = (cw - keep_end).max(0);

    // Almost no background either side: replicate the outer edge instead of chewing product.
    if (left_width + right_width) < (LOW_BACKGROUND_FRACTION * cw as f64) as i32 {
        let left = extra / 2;
        return pad_replicate_horizontal(img, left, extra - left);
    }

    let total_bg = (left_width + right_width) as f64;
    let left_extra = ((extra as f64) * (left_width as f64 / total_bg)).round() as i32;
    let right_extra = extra - left_extra;

    let mut out: RgbImage = RgbImage::new(target as u32, ch);
    let mut cursor = 0i32;

    if left_width > 0 {
        let band = imageops::crop_imm(img, 0, 0, left_width as u32, ch).to_image();
        let grown = imageops::resize(&band, (left_width + left_extra) as u32, ch, imageops::FilterType::Triangle);
        blit(&mut out, &grown, cursor);
        cursor += grown.width() as i32;
    }

    // Middle (product band) — copied verbatim.
    let mid_w = keep_end - keep_start;
    if mid_w > 0 {
        let mid = imageops::crop_imm(img, keep_start as u32, 0, mid_w as u32, ch).to_image();
        blit(&mut out, &mid, cursor);
        cursor += mid_w;
    }

    if right_width > 0 {
        let band = imageops::crop_imm(img, keep_end as u32, 0, right_width as u32, ch).to_image();
        let grown = imageops::resize(&band, (right_width + right_extra) as u32, ch, imageops::FilterType::Triangle);
        blit(&mut out, &grown, cursor);
    }

    out
}

fn pad_replicate_horizontal(img: &RgbImage, left: i32, right: i32) -> RgbImage {
    let (w, h) = (img.width() as i32, img.height());
    let mut out = RgbImage::new((w + left + right) as u32, h);
    for y in 0..h {
        let first = *img.get_pixel(0, y);
        let last = *img.get_pixel((w - 1) as u32, y);
        for x in 0..left {
            out.put_pixel(x as u32, y, first);
        }
        for x in 0..w {
            out.put_pixel((left + x) as u32, y, *img.get_pixel(x as u32, y));
        }
        for x in 0..right {
            out.put_pixel((left + w + x) as u32, y, last);
        }
    }
    out
}

fn blit(dst: &mut RgbImage, src: &RgbImage, x_off: i32) {
    let h = dst.height().min(src.height());
    for y in 0..h {
        for x in 0..src.width() {
            let dx = x_off + x as i32;
            if dx >= 0 && (dx as u32) < dst.width() {
                dst.put_pixel(dx as u32, y, *src.get_pixel(x, y));
            }
        }
    }
}

// Guarantee an exact side×side output (pads/crops by at most a pixel of rounding slack).
fn ensure_square(img: RgbImage, side: i32) -> RgbImage {
    if img.width() as i32 == side && img.height() as i32 == side {
        return img;
    }
    let mut out = RgbImage::new(side as u32, side as u32);
    let copy_w = img.width().min(side as u32);
    let copy_h = img.height().min(side as u32);
    for y in 0..copy_h {
        for x in 0..copy_w {
            out.put_pixel(x, y, *img.get_pixel(x, y));
        }
    }
    // Fill any rounding remainder by replicating the last real row/col.
    for y in 0..side as u32 {
        for x in 0..side as u32 {
            if x >= copy_w || y >= copy_h {
                let sx = x.min(copy_w - 1);
                let sy = y.min(copy_h - 1);
                let p = *out.get_pixel(sx, sy);
                out.put_pixel(x, y, p);
            }
        }
    }
    out
}
