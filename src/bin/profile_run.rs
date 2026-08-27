//! Dev-only instrumented runner: times each pipeline step per image, sequentially.
//! Usage: profile_run <input_dir> <output_dir> <csv_path>

use csp::core;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let csv_path = PathBuf::from(&args[2]);
    let _ = std::fs::create_dir_all(&output);

    const SUPPORTED: &[&str] = &["jpg", "jpeg", "png", "bmp", "tif", "tiff", "webp"];
    let mut files: Vec<PathBuf> = std::fs::read_dir(&input)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| SUPPORTED.contains(&e.to_ascii_lowercase().as_str()))
                    .unwrap_or(false)
        })
        .collect();
    files.sort();

    let mut rows = vec!["file,width,height,load_ms,detect_ms,geometry_ms,fill_ms,resize_ms,save_ms,total_ms,status,detail".to_string()];

    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let t_total = Instant::now();

        let t0 = Instant::now();
        let loaded = match core::load::load_image(path) {
            Ok(l) => l,
            Err(e) => {
                rows.push(format!("{name},,,,,,,,{:.2},error,\"load: {e}\"", t_total.elapsed().as_secs_f64() * 1000.0));
                continue;
            }
        };
        let load_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let (w, h) = (loaded.rgb.width(), loaded.rgb.height());

        let t1 = Instant::now();
        let det = core::detect::detect(&loaded.rgb, loaded.alpha.as_ref());
        let detect_ms = t1.elapsed().as_secs_f64() * 1000.0;

        let t2 = Instant::now();
        let mut layout = core::geometry::plan(&det, w as i32, h as i32);
        let min_fill = core::resize::min_fill_side();
        if !layout.already_square && layout.side < min_fill {
            layout.side = min_fill;
        }
        let geometry_ms = t2.elapsed().as_secs_f64() * 1000.0;

        let t3 = Instant::now();
        let square = core::fill::render(&loaded.rgb, &layout);
        let fill_ms = t3.elapsed().as_secs_f64() * 1000.0;

        let t4 = Instant::now();
        let resized = core::resize::resize_to_spec(&square);
        let resize_ms = t4.elapsed().as_secs_f64() * 1000.0;

        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
        let out_path = output.join(format!("{stem}.jpg"));
        let t5 = Instant::now();
        let save_result = core::save::save_jpeg_srgb(&resized, &out_path, core::config::JPEG_QUALITY);
        let save_ms = t5.elapsed().as_secs_f64() * 1000.0;

        let total_ms = t_total.elapsed().as_secs_f64() * 1000.0;

        let detail = format!(
            "kind={:?} edges={}(t{} b{} l{} r{}) conf={:.2} shadow={:.2} side={} out={} already_square={}",
            det.kind,
            det.intersects.count(),
            det.intersects.top as u8,
            det.intersects.bottom as u8,
            det.intersects.left as u8,
            det.intersects.right as u8,
            det.confidence,
            det.hard_shadow_fraction,
            layout.side,
            resized.width(),
            layout.already_square
        );

        match save_result {
            Ok(()) => rows.push(format!(
                "{name},{w},{h},{load_ms:.2},{detect_ms:.2},{geometry_ms:.2},{fill_ms:.2},{resize_ms:.2},{save_ms:.2},{total_ms:.2},ok,\"{detail}\""
            )),
            Err(e) => rows.push(format!(
                "{name},{w},{h},{load_ms:.2},{detect_ms:.2},{geometry_ms:.2},{fill_ms:.2},{resize_ms:.2},{save_ms:.2},{total_ms:.2},error,\"save: {e}\""
            )),
        }

        println!("{name}: {total_ms:.1}ms total ({w}x{h})");
    }

    std::fs::write(&csv_path, rows.join("\n")).unwrap();
    println!("\nwrote {} rows to {}", rows.len() - 1, csv_path.display());
}
