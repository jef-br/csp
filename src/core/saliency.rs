//! Spectral-residual saliency (Hou & Zhang) + largest-salient-square placement.
//!
//! Used only to place a crop window on a detail shot that bleeds off the canvas — there is no
//! product outline to crop to, so the biggest square that fits over the busiest content is taken.

use super::config::CENTER_PRIOR_FALLOFF;
use super::types::Box;
use image::RgbImage;
use rustfft::{num_complex::Complex, FftPlanner};

const WORKING_SIZE: usize = 192;

/// Largest square (scaled by `zoom` <= 1) placed over the most salient content.
pub fn most_salient_square(rgb: &RgbImage, zoom: f64) -> Box {
    let (w, h) = (rgb.width() as i32, rgb.height() as i32);
    let side = ((w.min(h) as f64 * zoom).round() as i32).clamp(1, w.min(h));
    if w == h && zoom >= 1.0 {
        return Box::new(0, 0, side, side);
    }

    let sal = center_primed_saliency(rgb);
    // Integral image of the saliency at full resolution for O(1) window sums.
    let (sw, sh) = (w as usize, h as usize);
    let mut integral = vec![0.0f64; (sw + 1) * (sh + 1)];
    for y in 0..sh {
        let mut row = 0.0;
        for x in 0..sw {
            row += sal[y * sw + x] as f64;
            integral[(y + 1) * (sw + 1) + (x + 1)] = integral[y * (sw + 1) + (x + 1)] + row;
        }
    }
    let win_sum = |x0: usize, y0: usize| -> f64 {
        let x1 = x0 + side as usize;
        let y1 = y0 + side as usize;
        integral[y1 * (sw + 1) + x1] - integral[y0 * (sw + 1) + x1] - integral[y1 * (sw + 1) + x0]
            + integral[y0 * (sw + 1) + x0]
    };

    // Search the busier (longer) axis by saliency; center the square on the other axis (which,
    // once zoomed below the full short side, also has slack).
    let horizontal = w >= h;
    let (search_limit, fixed_limit) = if horizontal { (w - side, h - side) } else { (h - side, w - side) };
    let fixed = (fixed_limit / 2).max(0);
    let step = (search_limit / 400).max(1);

    let mut offsets: Vec<i32> = (0..=search_limit).step_by(step as usize).collect();
    if *offsets.last().unwrap_or(&-1) != search_limit {
        offsets.push(search_limit);
    }

    let (mut best_off, mut best_score) = (0i32, -1.0f64);
    for off in offsets {
        let (x0, y0) = if horizontal { (off as usize, fixed as usize) } else { (fixed as usize, off as usize) };
        let s = win_sum(x0, y0);
        if s > best_score {
            best_score = s;
            best_off = off;
        }
    }

    if horizontal {
        Box::new(best_off, fixed, side, side)
    } else {
        Box::new(fixed, best_off, side, side)
    }
}

// Spectral-residual saliency at WORKING_SIZE, resampled to full frame, with a mild center prior.
fn center_primed_saliency(rgb: &RgbImage) -> Vec<f32> {
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let scale = WORKING_SIZE as f64 / w.max(h) as f64;
    let sw = ((w as f64 * scale).round() as usize).max(16);
    let sh = ((h as f64 * scale).round() as usize).max(16);
    let small = image::imageops::resize(
        rgb,
        sw as u32,
        sh as u32,
        image::imageops::FilterType::Triangle,
    );

    // Grayscale float buffer.
    let mut buf: Vec<Complex<f32>> = Vec::with_capacity(sw * sh);
    for p in small.pixels() {
        let g = 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
        buf.push(Complex::new(g, 0.0));
    }

    fft2d(&mut buf, sw, sh, false);
    let log_amp: Vec<f32> = buf.iter().map(|c| (c.norm() + 1e-8).ln()).collect();
    let phase: Vec<f32> = buf.iter().map(|c| c.arg()).collect();
    let smooth = box_blur(&log_amp, sw, sh, 3);

    // Reconstruct from spectral residual + original phase.
    let mut recon: Vec<Complex<f32>> = (0..sw * sh)
        .map(|i| {
            let residual = log_amp[i] - smooth[i];
            let mag = residual.exp();
            Complex::new(mag * phase[i].cos(), mag * phase[i].sin())
        })
        .collect();
    fft2d(&mut recon, sw, sh, true);

    let mut sal: Vec<f32> = recon.iter().map(|c| c.norm_sqr()).collect();
    let sal = box_blur(&sal_normalize(&mut sal), sw, sh, 3);

    // Upsample saliency to full frame (nearest is fine — it only places a window).
    let mut full = vec![0.0f32; w * h];
    for y in 0..h {
        let sy = ((y as f64 * scale).floor() as usize).min(sh - 1);
        for x in 0..w {
            let sx = ((x as f64 * scale).floor() as usize).min(sw - 1);
            full[y * w + x] = sal[sy * sw + sx];
        }
    }
    apply_center_prior(&mut full, w, h);
    full
}

fn apply_center_prior(sal: &mut [f32], w: usize, h: usize) {
    let strength = 0.35f32;
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 - cx) / cx;
            let dy = (y as f32 - cy) / cy;
            let dist = dx * dx + dy * dy;
            let prior = (-dist / (2.0 * CENTER_PRIOR_FALLOFF * CENTER_PRIOR_FALLOFF)).exp();
            sal[y * w + x] *= (1.0 - strength) + strength * prior;
        }
    }
}

fn sal_normalize(sal: &mut [f32]) -> Vec<f32> {
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for &v in sal.iter() {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let range = (hi - lo).max(1e-8);
    sal.iter().map(|&v| (v - lo) / range).collect()
}

fn box_blur(src: &[f32], w: usize, h: usize, k: usize) -> Vec<f32> {
    let r = (k / 2) as i32;
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0f32;
            let mut cnt = 0.0f32;
            for dy in -r..=r {
                for dx in -r..=r {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                        acc += src[ny as usize * w + nx as usize];
                        cnt += 1.0;
                    }
                }
            }
            out[y * w + x] = acc / cnt;
        }
    }
    out
}

// In-place 2D FFT (row-column decomposition). inverse=true does the inverse transform.
fn fft2d(buf: &mut [Complex<f32>], w: usize, h: usize, inverse: bool) {
    let mut planner = FftPlanner::new();
    let fft_row = if inverse {
        planner.plan_fft_inverse(w)
    } else {
        planner.plan_fft_forward(w)
    };
    for y in 0..h {
        fft_row.process(&mut buf[y * w..y * w + w]);
    }
    // Columns: gather, transform, scatter.
    let fft_col = if inverse {
        planner.plan_fft_inverse(h)
    } else {
        planner.plan_fft_forward(h)
    };
    let mut col = vec![Complex::new(0.0f32, 0.0); h];
    for x in 0..w {
        for y in 0..h {
            col[y] = buf[y * w + x];
        }
        fft_col.process(&mut col);
        for y in 0..h {
            buf[y * w + x] = col[y];
        }
    }
    if inverse {
        let n = (w * h) as f32;
        for c in buf.iter_mut() {
            *c /= n;
        }
    }
}
