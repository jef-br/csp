//! R1 step 2 — build the square.
//!
//! Two passes, horizontal then vertical. The first widens every real row to the square's width;
//! the second stretches the result to the square's height. Splitting it this way is what fills the
//! *corners*: a corner sits outside the image on both axes at once, so no single pass can source
//! it from real pixels — but after the horizontal pass, the top rows are already the full square
//! width, and the vertical pass reads them like any other row.
//!
//! Both passes are the same function over one line — a row for the first, a column for the second.
//! Crop and stretch are not separate branches in it: a side whose square edge lands inside the
//! image resamples a source segment onto a target of exactly the same length, which is a copy.

use image::{Rgb, RgbImage};

use super::plan::SquarePlan;
use crate::core::shot_classifier::geometry::Rect;

/// Compose the square from `src`, stretching background outward wherever the square reaches past a
/// border. `subject` is the subject's box in `src`'s coordinates; `safety` is how far outside it
/// the background is considered trustworthy.
pub fn compose(src: &RgbImage, plan: &SquarePlan, subject: Rect, safety: u32) -> RgbImage {
    let (w, h) = (src.width(), src.height());
    let Some(real) = plan.real_region(w, h) else {
        return RgbImage::new(plan.side, plan.side);
    };
    let side = plan.side as usize;
    let safety = safety as i64;

    // Pass 1: every real row becomes a full-width square row.
    let mut mid = RgbImage::new(plan.side, real.h);
    let mut line: Vec<Rgb<u8>> = vec![Rgb([0, 0, 0]); side];
    for ry in 0..real.h {
        let y = real.y + ry;
        let row: Vec<Rgb<u8>> = (0..w).map(|x| *src.get_pixel(x, y)).collect();
        fill_line(
            &row,
            &mut line,
            plan.x,
            subject.x as i64 - safety,
            (subject.x + subject.w) as i64 + safety,
        );
        for (x, px) in line.iter().enumerate() {
            mid.put_pixel(x as u32, ry, *px);
        }
    }

    // Pass 2: those rows become the square's full height. `mid` row 0 is original row `real.y`, so
    // the square's origin sits at `plan.y - real.y` in this pass's coordinates.
    let mut out = RgbImage::new(plan.side, plan.side);
    for x in 0..plan.side {
        let col: Vec<Rgb<u8>> = (0..real.h).map(|y| *mid.get_pixel(x, y)).collect();
        fill_line(
            &col,
            &mut line,
            plan.y - real.y as i64,
            subject.y as i64 - safety - real.y as i64,
            (subject.y + subject.h) as i64 + safety - real.y as i64,
        );
        for (y, px) in line.iter().enumerate() {
            out.put_pixel(x, y as u32, *px);
        }
    }
    out
}

/// Write one output line of `out.len()` samples, taken from `src`.
///
/// `start` is the source index the output's first sample maps to — negative when the square
/// begins outside the image. `inner_lo`/`inner_hi` bound the subject plus its safety margin: the
/// segment between them is copied untouched, because that is where the product lives and the
/// product is never resampled. Everything outside them is background, and background is what
/// absorbs the difference between what the source has and what the square needs.
fn fill_line(src: &[Rgb<u8>], out: &mut [Rgb<u8>], start: i64, inner_lo: i64, inner_hi: i64) {
    let len = src.len() as i64;
    let side = out.len() as i64;
    if len == 0 || side == 0 {
        return;
    }

    // The source range the square actually covers, and the product segment inside it.
    let lo_src = start.max(0);
    let hi_src = (start + side).min(len);
    let inner_lo = inner_lo.clamp(lo_src, hi_src);
    let inner_hi = inner_hi.clamp(inner_lo, hi_src);

    // The same two boundaries in output coordinates.
    let a = (inner_lo - start).clamp(0, side) as usize;
    let b = (inner_hi - start).clamp(0, side) as usize;

    resample(&src[lo_src as usize..inner_lo as usize], &mut out[..a]);
    for (k, i) in (inner_lo..inner_hi).enumerate() {
        out[a + k] = src[i as usize];
    }
    resample(&src[inner_hi as usize..hi_src as usize], &mut out[b..]);
}

/// Linear resample of `src` across `out`, with both ends pinned: `out[0]` is `src[0]` and the last
/// sample of each is the other's. Pinning matters at the outer end — that sample lands on the
/// square's own border, so anything else would shift the background by a fraction of a pixel and
/// leave a seam against the untouched product band.
///
/// An empty source with a non-empty target cannot happen from `fill_line`'s clamping unless the
/// subject runs to the border with no background left at all; that line is left black rather than
/// invented, so the case is visible instead of silently plausible.
fn resample(src: &[Rgb<u8>], out: &mut [Rgb<u8>]) {
    if out.is_empty() || src.is_empty() {
        return;
    }
    let n = src.len();
    let m = out.len();
    if n == m {
        out.copy_from_slice(src);
        return;
    }
    let span = (n - 1) as f32;
    let steps = (m - 1).max(1) as f32;
    for (k, dst) in out.iter_mut().enumerate() {
        let pos = k as f32 * span / steps;
        let i = pos.floor() as usize;
        let t = pos - i as f32;
        let a = src[i.min(n - 1)];
        let b = src[(i + 1).min(n - 1)];
        *dst = Rgb([
            (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t).round() as u8,
            (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t).round() as u8,
            (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t).round() as u8,
        ]);
    }
}
