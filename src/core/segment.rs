//! Figure/ground segmentation via a border-connected background prior.
//!
//! Over-segment the image into superpixels, build a region-adjacency graph weighted by colour
//! distance, then compute each superpixel's geodesic distance to the frame border (Dijkstra from
//! all border superpixels, edge cost = colour step above a small clip). Background = reachable from
//! the border through small steps (a wall, a floor — both touch the border and are locally uniform).
//! Foreground = walled off from the border by a strong colour boundary (the product), captured whole
//! regardless of internal darkness or texture.

use super::config::*;
use super::enhance;
use super::imgmath;
use super::superpixel::{self, Superpixels};
use image::{GrayImage, RgbImage};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Foreground mask (0/255) at the working segmentation resolution. With `enhance`, a low-contrast
/// zone-stretch is applied to the lightness first (the fallback path when the normal pass missed).
pub fn foreground_mask(rgb_small: &RgbImage, enhance: bool) -> GrayImage {
    let (w, h) = (rgb_small.width() as usize, rgb_small.height() as usize);
    let (mut l, a, b) = imgmath::rgb_to_lab(rgb_small.as_raw(), w, h);

    if enhance {
        l = enhance::low_contrast_boost(&l);
    }

    let sp = superpixel::slic(&l, &a, &b, SEG_K, SEG_COMPACT, SEG_ITERS);
    let geo = geodesic_to_border(&sp);
    let mut mask = GrayImage::new(w as u32, h as u32);
    for i in 0..(w * h) {
        let ci = sp.labels[i] as usize;
        mask.as_mut()[i] = if geo[ci] > GEO_THRESHOLD { 255 } else { 0 };
    }
    mask
}

// Shortest-path colour distance from any border superpixel to each superpixel.
fn geodesic_to_border(sp: &Superpixels) -> Vec<f32> {
    let adj = adjacency(sp);
    let n = sp.count;
    let mut dist = vec![f32::MAX; n];
    let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();

    for ci in 0..n {
        if sp.border[ci] {
            dist[ci] = 0.0;
            heap.push(Reverse((0, ci as u32)));
        }
    }

    while let Some(Reverse((d_scaled, u))) = heap.pop() {
        let u = u as usize;
        let d = d_scaled as f32 / 1000.0;
        if d > dist[u] {
            continue;
        }
        for &(v, weight) in &adj[u] {
            let nd = d + weight;
            if nd < dist[v as usize] {
                dist[v as usize] = nd;
                heap.push(Reverse(((nd * 1000.0) as u64, v)));
            }
        }
    }

    for d in dist.iter_mut() {
        if *d == f32::MAX {
            *d = 0.0; // unreachable (isolated) — treat as background, harmless
        }
    }
    dist
}

// Region adjacency graph: an undirected edge between neighbouring superpixels, weighted by the
// colour step between their mean Lab (clipped so movement within a smooth region is nearly free).
fn adjacency(sp: &Superpixels) -> Vec<Vec<(u32, f32)>> {
    let (w, h) = (sp.w, sp.h);
    let mut seen: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
    let mut adj: Vec<Vec<(u32, f32)>> = vec![Vec::new(); sp.count];

    let mut add = |x: u32, y: u32, seen: &mut std::collections::HashSet<(u32, u32)>, adj: &mut Vec<Vec<(u32, f32)>>| {
        let (a, b) = (x.min(y), x.max(y));
        if a == b || !seen.insert((a, b)) {
            return;
        }
        let weight = lab_dist(&sp.mean_lab[a as usize], &sp.mean_lab[b as usize]);
        let weight = (weight - GEO_CLIP).max(0.0);
        adj[a as usize].push((b, weight));
        adj[b as usize].push((a, weight));
    };

    for y in 0..h {
        for x in 0..w {
            let cur = sp.labels[y * w + x];
            if x + 1 < w {
                add(cur, sp.labels[y * w + x + 1], &mut seen, &mut adj);
            }
            if y + 1 < h {
                add(cur, sp.labels[(y + 1) * w + x], &mut seen, &mut adj);
            }
        }
    }
    adj
}

fn lab_dist(p: &[f32; 3], q: &[f32; 3]) -> f32 {
    let dl = p[0] - q[0];
    let da = p[1] - q[1];
    let db = p[2] - q[2];
    (dl * dl + da * da + db * db).sqrt()
}
