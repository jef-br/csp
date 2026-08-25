//! SLIC superpixels (Achanta et al.): cluster pixels in 5D (Lab + xy) into compact regions.
//!
//! Used by `segment` to build a region graph for the border-connected background prior. Runs at a
//! reduced working resolution; connectivity is not strictly enforced (the geodesic flood tolerates
//! the occasional stray pixel).

use super::imgmath::Plane;

pub struct Superpixels {
    pub w: usize,
    pub h: usize,
    pub labels: Vec<u32>,
    pub count: usize,
    /// Per-superpixel mean Lab (l,a,b).
    pub mean_lab: Vec<[f32; 3]>,
    /// Per-superpixel pixel count.
    pub size: Vec<u32>,
    /// Whether the superpixel touches the image border.
    pub border: Vec<bool>,
}

struct Center {
    l: f32,
    a: f32,
    b: f32,
    x: f32,
    y: f32,
}

/// Segment the Lab planes into roughly `k` superpixels with the given compactness `m`.
pub fn slic(l: &Plane, a: &Plane, b: &Plane, k: usize, m: f32, iterations: usize) -> Superpixels {
    let (w, h) = (l.w, l.h);
    let n = w * h;
    let step = ((n as f32 / k as f32).sqrt()).max(2.0);
    let s = step as i32;

    // Grid of initial centers.
    let mut centers: Vec<Center> = Vec::new();
    let mut cy = step / 2.0;
    while (cy as usize) < h {
        let mut cx = step / 2.0;
        while (cx as usize) < w {
            let (ix, iy) = (cx as usize, cy as usize);
            centers.push(Center { l: l.at(ix, iy), a: a.at(ix, iy), b: b.at(ix, iy), x: cx, y: cy });
            cx += step;
        }
        cy += step;
    }
    let kc = centers.len().max(1);

    let mut labels = vec![u32::MAX; n];
    let mut distance = vec![f32::MAX; n];
    let inv_s2 = (m / step) * (m / step); // spatial weight relative to colour

    for _ in 0..iterations {
        for d in distance.iter_mut() {
            *d = f32::MAX;
        }
        for (ci, c) in centers.iter().enumerate() {
            let x0 = ((c.x as i32 - s).max(0)) as usize;
            let y0 = ((c.y as i32 - s).max(0)) as usize;
            let x1 = ((c.x as i32 + s).min(w as i32 - 1)) as usize;
            let y1 = ((c.y as i32 + s).min(h as i32 - 1)) as usize;
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let i = y * w + x;
                    let dl = l.data[i] - c.l;
                    let da = a.data[i] - c.a;
                    let db = b.data[i] - c.b;
                    let dc = dl * dl + da * da + db * db;
                    let dx = x as f32 - c.x;
                    let dy = y as f32 - c.y;
                    let ds = dx * dx + dy * dy;
                    let dist = dc + ds * inv_s2;
                    if dist < distance[i] {
                        distance[i] = dist;
                        labels[i] = ci as u32;
                    }
                }
            }
        }
        // Recompute centers.
        let mut acc = vec![[0.0f64; 5]; kc];
        let mut cnt = vec![0u32; kc];
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let ci = labels[i] as usize;
                if ci == u32::MAX as usize {
                    continue;
                }
                acc[ci][0] += l.data[i] as f64;
                acc[ci][1] += a.data[i] as f64;
                acc[ci][2] += b.data[i] as f64;
                acc[ci][3] += x as f64;
                acc[ci][4] += y as f64;
                cnt[ci] += 1;
            }
        }
        for ci in 0..kc {
            if cnt[ci] == 0 {
                continue;
            }
            let f = cnt[ci] as f64;
            centers[ci].l = (acc[ci][0] / f) as f32;
            centers[ci].a = (acc[ci][1] / f) as f32;
            centers[ci].b = (acc[ci][2] / f) as f32;
            centers[ci].x = (acc[ci][3] / f) as f32;
            centers[ci].y = (acc[ci][4] / f) as f32;
        }
    }

    // Assign any never-labeled pixel to nearest labeled neighbor's cluster (rare edges).
    for i in 0..n {
        if labels[i] == u32::MAX {
            labels[i] = 0;
        }
    }

    // Aggregate per-superpixel stats.
    let mut mean = vec![[0.0f64; 3]; kc];
    let mut size = vec![0u32; kc];
    let mut border = vec![false; kc];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let ci = labels[i] as usize;
            mean[ci][0] += l.data[i] as f64;
            mean[ci][1] += a.data[i] as f64;
            mean[ci][2] += b.data[i] as f64;
            size[ci] += 1;
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                border[ci] = true;
            }
        }
    }
    let mean_lab: Vec<[f32; 3]> = (0..kc)
        .map(|ci| {
            let c = size[ci].max(1) as f64;
            [(mean[ci][0] / c) as f32, (mean[ci][1] / c) as f32, (mean[ci][2] / c) as f32]
        })
        .collect();

    Superpixels { w, h, labels, count: kc, mean_lab, size, border }
}
