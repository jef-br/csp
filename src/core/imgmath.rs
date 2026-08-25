//! Low-level numeric helpers for detection: Lab conversion, integral-image box statistics,
//! robust statistics, and a least-squares plane fit. All operate on flat f32 planes.

/// A single-channel f32 plane in row-major order.
pub struct Plane {
    pub w: usize,
    pub h: usize,
    pub data: Vec<f32>,
}

impl Plane {
    pub fn new(w: usize, h: usize) -> Self {
        Plane { w, h, data: vec![0.0; w * h] }
    }

    #[inline]
    pub fn at(&self, x: usize, y: usize) -> f32 {
        self.data[y * self.w + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, v: f32) {
        self.data[y * self.w + x] = v;
    }
}

/// Convert an interleaved RGB8 buffer to three Lab planes (L, a, b) in CIELAB, D65.
/// a and b are returned centered on 0 (not offset by 128).
pub fn rgb_to_lab(rgb: &[u8], w: usize, h: usize) -> (Plane, Plane, Plane) {
    // sRGB->linear has only 256 possible inputs; a LUT removes millions of powf calls. The Lab
    // cube-root is replaced by a 4096-entry table over the linear domain (nearest lookup).
    let lut = srgb_linear_lut();
    let flut = lab_f_lut();
    let mut l = Plane::new(w, h);
    let mut a = Plane::new(w, h);
    let mut b = Plane::new(w, h);
    for i in 0..(w * h) {
        let rf = lut[rgb[i * 3] as usize];
        let gf = lut[rgb[i * 3 + 1] as usize];
        let bf = lut[rgb[i * 3 + 2] as usize];
        let x = rf * 0.4124 + gf * 0.3576 + bf * 0.1805;
        let y = rf * 0.2126 + gf * 0.7152 + bf * 0.0722;
        let z = rf * 0.0193 + gf * 0.1192 + bf * 0.9505;
        let fx = flut_lookup(&flut, x / 0.95047);
        let fy = flut_lookup(&flut, y / 1.00000);
        let fz = flut_lookup(&flut, z / 1.08883);
        l.data[i] = (116.0 * fy - 16.0) as f32;
        a.data[i] = (500.0 * (fx - fy)) as f32;
        b.data[i] = (200.0 * (fy - fz)) as f32;
    }
    (l, a, b)
}

const FLUT_N: usize = 4096;
const FLUT_MAX: f64 = 1.2;

fn lab_f_lut() -> [f64; FLUT_N] {
    let mut lut = [0.0f64; FLUT_N];
    for (i, v) in lut.iter_mut().enumerate() {
        *v = lab_f(i as f64 / (FLUT_N - 1) as f64 * FLUT_MAX);
    }
    lut
}

#[inline]
fn flut_lookup(lut: &[f64; FLUT_N], t: f64) -> f64 {
    let idx = ((t / FLUT_MAX) * (FLUT_N - 1) as f64).round();
    if idx < 0.0 {
        lut[0]
    } else if idx as usize >= FLUT_N {
        lab_f(t)
    } else {
        lut[idx as usize]
    }
}

fn srgb_linear_lut() -> [f64; 256] {
    let mut lut = [0.0f64; 256];
    for (i, v) in lut.iter_mut().enumerate() {
        *v = srgb_to_linear(i as u8);
    }
    lut
}

fn srgb_to_linear(c: u8) -> f64 {
    let cs = c as f64 / 255.0;
    if cs <= 0.04045 {
        cs / 12.92
    } else {
        ((cs + 0.055) / 1.055).powf(2.4)
    }
}

fn lab_f(t: f64) -> f64 {
    const DELTA: f64 = 6.0 / 29.0;
    if t > DELTA * DELTA * DELTA {
        t.cbrt()
    } else {
        t / (3.0 * DELTA * DELTA) + 4.0 / 29.0
    }
}

/// Separable Gaussian blur of a plane with the given sigma. Edges use clamp-to-border.
pub fn gaussian_blur(p: &Plane, sigma: f64) -> Plane {
    if sigma <= 0.0 {
        return Plane { w: p.w, h: p.h, data: p.data.clone() };
    }
    let radius = (sigma * 3.0).ceil() as i32;
    let mut kernel = vec![0.0f64; (radius * 2 + 1) as usize];
    let mut sum = 0.0;
    for (i, k) in kernel.iter_mut().enumerate() {
        let d = i as i32 - radius;
        *k = (-(d * d) as f64 / (2.0 * sigma * sigma)).exp();
        sum += *k;
    }
    for k in kernel.iter_mut() {
        *k /= sum;
    }

    let (w, h) = (p.w, p.h);
    let mut tmp = Plane::new(w, h);
    // Horizontal pass.
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0f64;
            for (i, &k) in kernel.iter().enumerate() {
                let sx = (x as i32 + i as i32 - radius).clamp(0, w as i32 - 1) as usize;
                acc += k * p.at(sx, y) as f64;
            }
            tmp.set(x, y, acc as f32);
        }
    }
    // Vertical pass.
    let mut out = Plane::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0f64;
            for (i, &k) in kernel.iter().enumerate() {
                let sy = (y as i32 + i as i32 - radius).clamp(0, h as i32 - 1) as usize;
                acc += k * tmp.at(x, sy) as f64;
            }
            out.set(x, y, acc as f32);
        }
    }
    out
}

/// Edge-preserving bilateral filter on a plane: smooths noise while keeping real edges (weights a
/// neighbour by both spatial closeness and value similarity). Radius in pixels; sigmas in the same
/// units as the plane values / pixels.
pub fn bilateral(p: &Plane, spatial_sigma: f64, range_sigma: f64, radius: i32) -> Plane {
    let (w, h) = (p.w, p.h);
    // Precompute the spatial kernel.
    let mut spatial = vec![0.0f64; ((2 * radius + 1) * (2 * radius + 1)) as usize];
    let k = 2 * radius + 1;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let s = (-((dx * dx + dy * dy) as f64) / (2.0 * spatial_sigma * spatial_sigma)).exp();
            spatial[((dy + radius) * k + (dx + radius)) as usize] = s;
        }
    }
    let inv_2r2 = 1.0 / (2.0 * range_sigma * range_sigma);
    let mut out = Plane::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let center = p.at(x, y) as f64;
            let mut acc = 0.0f64;
            let mut wsum = 0.0f64;
            for dy in -radius..=radius {
                let ny = y as i32 + dy;
                if ny < 0 || ny >= h as i32 {
                    continue;
                }
                for dx in -radius..=radius {
                    let nx = x as i32 + dx;
                    if nx < 0 || nx >= w as i32 {
                        continue;
                    }
                    let v = p.at(nx as usize, ny as usize) as f64;
                    let sr = spatial[((dy + radius) * k + (dx + radius)) as usize];
                    let dv = v - center;
                    let wgt = sr * (-(dv * dv) * inv_2r2).exp();
                    acc += wgt * v;
                    wsum += wgt;
                }
            }
            out.set(x, y, (acc / wsum.max(1e-9)) as f32);
        }
    }
    out
}

/// Otsu threshold that best splits a plane's values into two clusters. `lo`/`hi` bound the value
/// range; returns the threshold in that range. Used to separate a bright zone from shadows.
pub fn otsu(p: &Plane, lo: f32, hi: f32) -> f32 {
    const BINS: usize = 256;
    let mut hist = [0u32; BINS];
    let span = (hi - lo).max(1e-6);
    for &v in p.data.iter() {
        let idx = (((v - lo) / span) * (BINS as f32 - 1.0)).round().clamp(0.0, BINS as f32 - 1.0) as usize;
        hist[idx] += 1;
    }
    let total: u32 = hist.iter().sum();
    if total == 0 {
        return (lo + hi) * 0.5;
    }
    let sum_all: f64 = (0..BINS).map(|i| i as f64 * hist[i] as f64).sum();
    let (mut w_b, mut sum_b) = (0.0f64, 0.0f64);
    let (mut best_var, mut best_t) = (-1.0f64, 0usize);
    for t in 0..BINS {
        w_b += hist[t] as f64;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total as f64 - w_b;
        if w_f == 0.0 {
            break;
        }
        sum_b += t as f64 * hist[t] as f64;
        let m_b = sum_b / w_b;
        let m_f = (sum_all - sum_b) / w_f;
        let between = w_b * w_f * (m_b - m_f) * (m_b - m_f);
        if between > best_var {
            best_var = between;
            best_t = t;
        }
    }
    lo + (best_t as f32 / (BINS as f32 - 1.0)) * span
}

/// Fast Gaussian approximation: three box-blur passes (Wells' method), O(n) in the image size and
/// independent of sigma. Good enough for the detector's high-pass; far cheaper than a wide kernel.
pub fn box_blur_gaussian(p: &Plane, sigma: f64) -> Plane {
    if sigma <= 0.0 {
        return Plane { w: p.w, h: p.h, data: p.data.clone() };
    }
    // Box width matching the target sigma across 3 passes.
    let ideal = (12.0 * sigma * sigma / 3.0 + 1.0).sqrt();
    let mut wl = ideal.floor() as i32;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let radius = (wl.max(1) / 2).max(1);
    let mut cur = Plane { w: p.w, h: p.h, data: p.data.clone() };
    for _ in 0..3 {
        cur = box_pass(&cur, radius);
    }
    cur
}

fn box_pass(p: &Plane, radius: i32) -> Plane {
    let integral = Integral::build(p);
    let mut out = Plane::new(p.w, p.h);
    let k = radius * 2 + 1;
    for y in 0..p.h {
        for x in 0..p.w {
            out.set(x, y, integral.box_mean(x, y, k) as f32);
        }
    }
    out
}

/// Integral image (summed-area table) of a plane, sized (w+1)*(h+1), f64 for precision.
pub struct Integral {
    w: usize,
    h: usize,
    sum: Vec<f64>,
}

impl Integral {
    pub fn build(p: &Plane) -> Self {
        let (w, h) = (p.w, p.h);
        let sw = w + 1;
        let mut sum = vec![0.0f64; sw * (h + 1)];
        for y in 0..h {
            let mut row_acc = 0.0f64;
            for x in 0..w {
                row_acc += p.at(x, y) as f64;
                sum[(y + 1) * sw + (x + 1)] = sum[y * sw + (x + 1)] + row_acc;
            }
        }
        Integral { w, h, sum }
    }

    /// Mean over the square window of side `k` centered at (x,y), clamped to image bounds.
    pub fn box_mean(&self, x: usize, y: usize, k: i32) -> f64 {
        let half = k / 2;
        let x0 = (x as i32 - half).max(0) as usize;
        let y0 = (y as i32 - half).max(0) as usize;
        let x1 = ((x as i32 + half + 1).min(self.w as i32)) as usize;
        let y1 = ((y as i32 + half + 1).min(self.h as i32)) as usize;
        let sw = self.w + 1;
        let s = self.sum[y1 * sw + x1] - self.sum[y0 * sw + x1] - self.sum[y1 * sw + x0]
            + self.sum[y0 * sw + x0];
        let area = ((x1 - x0) * (y1 - y0)) as f64;
        if area <= 0.0 {
            0.0
        } else {
            s / area
        }
    }
}

/// Median of a slice (sorts a copy). Empty -> 0.
pub fn median(values: &[f32]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v: Vec<f32> = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2] as f64
}

/// Median absolute deviation scaled to compare with a standard deviation.
pub fn robust_spread(values: &[f32]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let med = median(values) as f32;
    let dev: Vec<f32> = values.iter().map(|&x| (x - med).abs()).collect();
    1.4826 * median(&dev)
}

/// Least-squares fit of channel ~ c0 + c1*xn + c2*yn over the given sample coords/values,
/// with xn,yn normalized to [-1,1]. Returns (c0,c1,c2). Falls back to the median if degenerate.
pub fn fit_plane(coords: &[(f32, f32)], values: &[f32]) -> (f64, f64, f64) {
    if values.len() < 500 {
        return (median(values), 0.0, 0.0);
    }
    let (mut n, mut sx, mut sy) = (0.0f64, 0.0, 0.0);
    let (mut sxx, mut sxy, mut syy) = (0.0f64, 0.0, 0.0);
    let (mut sv, mut sxv, mut syv) = (0.0f64, 0.0, 0.0);
    for (&(xn, yn), &v) in coords.iter().zip(values.iter()) {
        let (xn, yn, v) = (xn as f64, yn as f64, v as f64);
        n += 1.0;
        sx += xn;
        sy += yn;
        sxx += xn * xn;
        sxy += xn * yn;
        syy += yn * yn;
        sv += v;
        sxv += xn * v;
        syv += yn * v;
    }
    solve3(n, sx, sy, sxx, sxy, syy, sv, sxv, syv)
        .unwrap_or((median(values), 0.0, 0.0))
}

// Solve the 3x3 normal-equation system via Cramer's rule.
fn solve3(
    n: f64, sx: f64, sy: f64, sxx: f64, sxy: f64, syy: f64, sv: f64, sxv: f64, syv: f64,
) -> Option<(f64, f64, f64)> {
    let m = [[n, sx, sy], [sx, sxx, sxy], [sy, sxy, syy]];
    let d = det3(&m);
    if d.abs() < 1e-9 {
        return None;
    }
    let mut mx = m;
    mx[0][0] = sv;
    mx[1][0] = sxv;
    mx[2][0] = syv;
    let mut my = m;
    my[0][1] = sv;
    my[1][1] = sxv;
    my[2][1] = syv;
    let mut mz = m;
    mz[0][2] = sv;
    mz[1][2] = sxv;
    mz[2][2] = syv;
    Some((det3(&mx) / d, det3(&my) / d, det3(&mz) / d))
}

fn det3(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}
