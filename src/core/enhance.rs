//! Low-contrast detection rescue (throwaway, detection-only).
//!
//! When the normal segmentation misses, boost a copy of the lightness so a faint product silhouette
//! becomes a real colour step: bilateral-denoise, split the bright zone from shadows (Otsu), measure
//! the zone's own tonal statistics, stretch that narrow band to full range within the zone only, and
//! feather it back so no artificial seam appears. Measuring μ/σ on the bright zone alone (not the
//! whole frame) keeps σ tight even when the image also contains dark shadows — that tightness is what
//! makes the stretch aggressive enough to separate white-on-white.

use super::config::*;
use super::imgmath::{self, Plane};

/// Return a contrast-boosted copy of the lightness plane (0..100). Chroma is handled by the caller.
pub fn low_contrast_boost(l: &Plane) -> Plane {
    let (w, h) = (l.w, l.h);

    // 1. Edge-preserving denoise so we stretch signal, not noise.
    let denoised = imgmath::bilateral(l, LC_BILATERAL_SPATIAL_SIGMA, LC_BILATERAL_RANGE_SIGMA, LC_BILATERAL_RADIUS);

    // 2. Split the bright zone (background + product) from the shadows. If there is no real dark
    //    cluster, the whole frame is the zone.
    let t = imgmath::otsu(&denoised, 0.0, 100.0);
    let dark_frac = denoised.data.iter().filter(|&&v| v <= t).count() as f64 / (w * h) as f64;
    let zone: Vec<bool> = if dark_frac < LC_DARK_FRACTION_MIN {
        vec![true; w * h]
    } else {
        denoised.data.iter().map(|&v| v > t).collect()
    };

    // 3. Zone tonal statistics (per-image, no fixed brightness constants).
    let (mut sum, mut sq, mut n) = (0.0f64, 0.0f64, 0u64);
    for i in 0..(w * h) {
        if zone[i] {
            let v = denoised.data[i] as f64;
            sum += v;
            sq += v * v;
            n += 1;
        }
    }
    if n == 0 {
        return Plane { w, h, data: l.data.clone() };
    }
    let mu = sum / n as f64;
    let var = (sq / n as f64 - mu * mu).max(0.0);
    let sigma = var.sqrt().max(1e-3);

    // 4. Endpoints: a couple of standard deviations either side of the zone mean. The stretch
    //    aggression auto-scales as 1/σ, so a tight (white-on-white) band is amplified hard while a
    //    wide band is barely touched.
    let black = (mu - LC_SIGMA_K * sigma) as f32;
    let white = (mu + LC_SIGMA_K * sigma) as f32;
    let range = (white - black).max(1e-3);

    // 5. Stretch inside the zone; feather the zone mask so the boost fades smoothly at its edge and
    //    injects no artificial boundary into the throwaway image.
    let mut alpha = Plane::new(w, h);
    for i in 0..(w * h) {
        alpha.data[i] = if zone[i] { 1.0 } else { 0.0 };
    }
    let alpha = imgmath::box_blur_gaussian(&alpha, LC_FEATHER_SIGMA);

    let mut out = Plane::new(w, h);
    for i in 0..(w * h) {
        let stretched = (((denoised.data[i] - black) / range).clamp(0.0, 1.0)) * 100.0;
        let a = alpha.data[i].clamp(0.0, 1.0);
        out.data[i] = a * stretched + (1.0 - a) * l.data[i];
    }
    out
}
