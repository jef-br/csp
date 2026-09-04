//! Contrast-Limited Adaptive Histogram Equalization on a single 0..255 lightness plane.
//!
//! Used only inside the detector's throwaway preprocessing to lift white-on-white weave clear of
//! the noise floor. Tiled clipped-CDF with bilinear interpolation between tile centers.

use super::imgmath::Plane;

const BINS: usize = 256;

/// Apply CLAHE to a plane whose values are in [0,255]. Returns a new equalized plane.
pub fn apply(src: &Plane, clip_limit: f64, tiles: u32) -> Plane {
    let (w, h) = (src.w, src.h);
    let tiles = tiles.max(1) as usize;
    let tw = (w + tiles - 1) / tiles;
    let th = (h + tiles - 1) / tiles;

    // Per-tile clipped CDF lookup tables.
    let mut luts = vec![[0u8; BINS]; tiles * tiles];
    for ty in 0..tiles {
        for tx in 0..tiles {
            let x0 = tx * tw;
            let y0 = ty * th;
            let x1 = (x0 + tw).min(w);
            let y1 = (y0 + th).min(h);
            luts[ty * tiles + tx] = tile_lut(src, x0, y0, x1, y1, clip_limit);
        }
    }

    // Bilinear interpolation of the four surrounding tile LUTs per pixel.
    let mut out = Plane::new(w, h);
    for y in 0..h {
        let gy = ((y as f64 + 0.5) / th as f64 - 0.5).clamp(0.0, tiles as f64 - 1.0);
        let ty0 = gy.floor() as usize;
        let ty1 = (ty0 + 1).min(tiles - 1);
        let fy = gy - ty0 as f64;
        for x in 0..w {
            let gx = ((x as f64 + 0.5) / tw as f64 - 0.5).clamp(0.0, tiles as f64 - 1.0);
            let tx0 = gx.floor() as usize;
            let tx1 = (tx0 + 1).min(tiles - 1);
            let fx = gx - tx0 as f64;

            let v = src.at(x, y).clamp(0.0, 255.0) as usize;
            let v00 = luts[ty0 * tiles + tx0][v] as f64;
            let v01 = luts[ty0 * tiles + tx1][v] as f64;
            let v10 = luts[ty1 * tiles + tx0][v] as f64;
            let v11 = luts[ty1 * tiles + tx1][v] as f64;
            let top = v00 * (1.0 - fx) + v01 * fx;
            let bot = v10 * (1.0 - fx) + v11 * fx;
            out.set(x, y, (top * (1.0 - fy) + bot * fy) as f32);
        }
    }
    out
}

fn tile_lut(src: &Plane, x0: usize, y0: usize, x1: usize, y1: usize, clip_limit: f64) -> [u8; BINS] {
    let mut hist = [0u32; BINS];
    let mut count = 0u32;
    for y in y0..y1 {
        for x in x0..x1 {
            let v = src.at(x, y).clamp(0.0, 255.0) as usize;
            hist[v] += 1;
            count += 1;
        }
    }
    if count == 0 {
        // Identity LUT.
        let mut lut = [0u8; BINS];
        for (i, l) in lut.iter_mut().enumerate() {
            *l = i as u8;
        }
        return lut;
    }

    // Clip histogram at clip_limit * mean, redistribute the excess uniformly.
    let mean = count as f64 / BINS as f64;
    let clip = (clip_limit * mean).max(1.0);
    let mut excess = 0.0f64;
    for c in hist.iter_mut() {
        if (*c as f64) > clip {
            excess += *c as f64 - clip;
            *c = clip as u32;
        }
    }
    let redistribute = excess / BINS as f64;

    let mut lut = [0u8; BINS];
    let mut cdf = 0.0f64;
    let total = count as f64;
    for i in 0..BINS {
        cdf += hist[i] as f64 + redistribute;
        lut[i] = ((cdf / total) * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    lut
}
