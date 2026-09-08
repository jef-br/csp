//! Diagnostic: what EIX does the pipeline derive, versus the known-good harness?
//!
//! Runs the real preprocessor and the real gate/refine passes over a stand-in
//! segmentation model, then prints both verdicts side by side. Not part of the product.

use csp::core::config;
use csp::core::preprocessor;
use csp::core::processor::routes::Route;
use csp::core::shot_classifier::geometry::Rect;
use csp::core::shot_classifier::segmentation::{Instance, Mask};
use csp::core::shot_classifier::{classify_instance, Edge, RefineParams, RefinementInput};
use image::RgbImage;

/// Stand-in for BiRefNet: foreground = pixels far from the image's corner colour.
/// On a full-frame close-up there is no distinct corner colour, so almost nothing
/// clears the threshold — the same near-empty mask BiRefNet returns there.
fn segment(img: &RgbImage) -> Mask {
    let bg = *img.get_pixel(0, 0);
    let (w, h) = (img.width(), img.height());
    let mut data = vec![0u8; (w * h) as usize];
    let mut uniform_corner = 0usize;
    for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
        let p = *img.get_pixel(x, y);
        if dist2(p.0, bg.0) < 900 {
            uniform_corner += 1;
        }
    }
    // No agreed background across the corners -> the segmenter has nothing to
    // separate against and returns almost nothing.
    if uniform_corner < 3 {
        return Mask { width: w, height: h, data };
    }
    for y in 0..h {
        for x in 0..w {
            if dist2(img.get_pixel(x, y).0, bg.0) > 900 {
                data[(y * w + x) as usize] = 255;
            }
        }
    }
    Mask { width: w, height: h, data }
}

fn dist2(a: [u8; 3], b: [u8; 3]) -> i32 {
    (0..3).map(|i| { let d = a[i] as i32 - b[i] as i32; d * d }).sum()
}

fn eix_bits(edges: &[Edge]) -> String {
    let bit = |e: Edge| if edges.contains(&e) { '1' } else { '0' };
    [bit(Edge::Top), bit(Edge::Right), bit(Edge::Bottom), bit(Edge::Left)].into_iter().collect()
}

/// The harness's threshold, from `examples/run_dir.rs`.
const FULL_BLEED_MAX_FG: f64 = 0.05;

fn main() {
    let dir = std::env::args().nth(1).expect("usage: eix_probe <image dir>");
    let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
    paths.sort();

    println!("{:<26} {:>10} {:>8}  {:<10} {:<10} {}", "image", "working", "fg%", "EIX(pipe)", "EIX(harness)", "route");
    println!("{}", "-".repeat(92));

    for path in paths {
        let prep = preprocessor::prepare(&path).unwrap();
        let mask = segment(&prep.working);
        let fg = mask.data.iter().filter(|&&v| v > 0).count();
        let total = (mask.width * mask.height) as usize;
        let fg_ratio = fg as f64 / total as f64;

        let inst = Instance {
            bbox: Rect::from_bounds(0, 0, mask.width - 1, mask.height - 1),
            mask,
            class_id: 0,
            confidence: 1.0,
        };
        let input = RefinementInput {
            working_image: &prep.working,
            original_image: &prep.original,
            instance: &inst,
        };
        let params = RefineParams {
            band_px: config::REFINE_BAND_PX,
            safety_px: config::REFINE_SAFETY_PX,
            context_px: config::REFINE_CONTEXT_PX,
        };
        let class = classify_instance(&input, config::GATE_MARGIN_PX, params);

        // What the pipeline acts on today.
        let eix_pipeline = eix_bits(&class.touches_edges);
        let route = Route::select(Some(&class));

        // What the known-good harness reported: same bits, plus the full-bleed override.
        let bgc_missing = fg_ratio < FULL_BLEED_MAX_FG;
        let eix_harness = if fg_ratio < FULL_BLEED_MAX_FG && bgc_missing {
            "1111".to_string()
        } else {
            eix_pipeline.clone()
        };

        println!(
            "{:<26} {:>10} {:>7.1}%  {:<10} {:<12} {:?}",
            path.file_name().unwrap().to_string_lossy(),
            format!("{}x{}", prep.working.width(), prep.working.height()),
            fg_ratio * 100.0,
            eix_pipeline,
            eix_harness,
            route
        );
    }
}
