//! Colour optimiser: the preprocessor's last step before BiRefNet sees the image.
//!
//! The segmentation model finds a subject more reliably when the working copy uses its full tonal
//! range and its midtones sit where the subject/background contrast is easiest to read. This pushes
//! each channel there, in three steps the spec lays out:
//!
//! 1. **Full-range stretch.** Per channel, map the channel's own darkest value to 0 and brightest
//!    to 255, so every channel has at least one pixel at each extreme.
//! 2. **±2.5σ expansion.** Measure the stretched channel's mean and standard deviation, then remap
//!    the `[μ − 2.5σ, μ + 2.5σ]` window (clamped to 0-255) onto the full range, clipping the tails.
//!    This throws away the thin outlier tails and spends the whole range on the bulk.
//! 3. **Midtone (Z) nudge.** Step 2 shifts the bulk's brightness — a bright-skewed channel gets
//!    pulled down, a dark one pushed up. Nudge the midtone *the opposite way* to soften that shift
//!    (and the noise it would otherwise amplify), with a gamma curve that leaves 0 and 255 pinned.
//!
//! Everything is per channel, so the result is an "enhanced colour mode", not a tone curve on
//! luminance — the colour balance is allowed to move. It runs on the working copy only; the
//! full-resolution `original` the exporter composes from stays true-colour.
//!
//! Each channel reduces to one 256-entry lookup table built from that channel's own statistics, so
//! the per-pixel cost is a single table read.

use image::RgbImage;

/// Half-width of the tonal window step 2 keeps, in standard deviations.
const SIGMA_CLIP: f32 = 2.5;

/// The midtone gamma ratio Z is clamped into this range. A high-contrast or bimodal channel can
/// make the raw `255 / inward` magnitude large; clamping keeps the midtone nudge to "somewhat", as
/// the spec asks. Both are tuning knobs — this module is a starting point to iterate on.
const Z_MIN: f32 = 0.5;
const Z_MAX: f32 = 2.0;

/// Enhance `img` for the segmentation model. Returns a new image; the caller keeps the original.
pub fn optimize(img: &RgbImage) -> RgbImage {
    let mut out = img.clone();
    for c in 0..3 {
        if let Some(lut) = channel_lut(img, c) {
            for px in out.pixels_mut() {
                px[c] = lut[px[c] as usize];
            }
        }
    }
    out
}

/// Build the 0-255 → 0-255 map for one channel, or `None` when there is nothing to do (an empty or
/// flat channel) so the caller leaves that channel untouched.
fn channel_lut(img: &RgbImage, c: usize) -> Option<[u8; 256]> {
    let mut hist = [0u64; 256];
    for px in img.pixels() {
        hist[px[c] as usize] += 1;
    }
    let total: u64 = hist.iter().sum();
    if total == 0 {
        return None;
    }

    // Step 1: full-range stretch over this channel's own extremes.
    let cmin = hist.iter().position(|&n| n > 0)? as f32;
    let cmax = hist.iter().rposition(|&n| n > 0)? as f32;
    if cmax <= cmin {
        return None; // one distinct value: nothing to stretch, leave it alone.
    }
    let stretch = |v: f32| (v - cmin) / (cmax - cmin) * 255.0;

    // Mean and std of the *stretched* channel, read straight from the histogram.
    let mut mean = 0.0f32;
    for (v, &n) in hist.iter().enumerate() {
        mean += stretch(v as f32) * n as f32;
    }
    mean /= total as f32;
    let mut var = 0.0f32;
    for (v, &n) in hist.iter().enumerate() {
        let d = stretch(v as f32) - mean;
        var += d * d * n as f32;
    }
    let std = (var / total as f32).sqrt();

    // Step 2: keep the ±2.5σ window and spread it over the full range.
    let lo = (mean - SIGMA_CLIP * std).clamp(0.0, 255.0);
    let hi = (mean + SIGMA_CLIP * std).clamp(0.0, 255.0);
    if hi <= lo {
        return None; // ~zero spread: the stretched channel already is the window.
    }
    let remap = |v: f32| ((v - lo) / (hi - lo) * 255.0).clamp(0.0, 255.0);

    // Step 3: nudge the midtone opposite to the brightness shift step 2 caused.
    // `mean` lands at `mean_after` once the window fills the range. If the bulk dropped (the image
    // darkened) we brighten the mids; if it rose, we darken them.
    let mean_after = (mean - lo) / (hi - lo) * 255.0;
    let shift = mean_after - mean;
    let z = if shift.abs() < 1.0 {
        // No real shift — including the case where the ±2.5σ window already fills 0-255, which is
        // exactly when `lo + (255 - hi)` would be zero. Leave the midtone alone; never divide by 0.
        1.0
    } else {
        // How far, together, the two ends moved inward; in (0, 255] whenever there is a shift.
        let inward = lo + (255.0 - hi);
        let z_mag = 255.0 / inward.max(1.0); // >= 1
        if shift < 0.0 {
            z_mag // darkened -> brighten the mids
        } else {
            1.0 / z_mag // lightened -> darken the mids
        }
    }
    .clamp(Z_MIN, Z_MAX);

    // Gamma that bends the midtone while pinning 0 and 255. Z > 1 brightens, Z < 1 darkens.
    let gamma = 1.0 / z;

    let mut lut = [0u8; 256];
    for (v, slot) in lut.iter_mut().enumerate() {
        let toned = remap(stretch(v as f32));
        *slot = (255.0 * (toned / 255.0).powf(gamma)).round().clamp(0.0, 255.0) as u8;
    }
    Some(lut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    /// A channel filled with one value has nothing to stretch, so the LUT is skipped.
    #[test]
    fn a_flat_channel_is_left_untouched() {
        let img = RgbImage::from_pixel(8, 8, Rgb([40, 40, 40]));
        let out = optimize(&img);
        assert_eq!(out, img);
    }

    /// Endpoints are always pinned: the darkest pixel stays 0-ish, the brightest 255-ish, whatever
    /// the midtone does in between.
    #[test]
    fn extremes_map_to_extremes() {
        let mut img = RgbImage::from_pixel(16, 16, Rgb([120, 120, 120]));
        img.put_pixel(0, 0, Rgb([10, 10, 10]));
        img.put_pixel(1, 0, Rgb([240, 240, 240]));
        let out = optimize(&img);
        assert_eq!(out.get_pixel(0, 0)[0], 0);
        assert_eq!(out.get_pixel(1, 0)[0], 255);
    }

    /// A full-range channel (±2.5σ already spans 0-255) must not divide by zero, and must leave the
    /// midtone alone: the remap is the identity, so only the stretch acts and a mid value is
    /// unchanged.
    #[test]
    fn full_range_channel_does_not_blow_up() {
        // Half black, half white: huge σ, so lo clamps to 0 and hi to 255.
        let mut img = RgbImage::new(10, 10);
        for (i, px) in img.pixels_mut().enumerate() {
            *px = if i % 2 == 0 { Rgb([0, 0, 0]) } else { Rgb([255, 255, 255]) };
        }
        let out = optimize(&img); // must not panic
        assert_eq!(out, img);
    }
}
