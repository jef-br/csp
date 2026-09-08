//! Batch-run the shot classifier (BiRefNet backend) over a folder of
//! images. For every input, these files are written into `<output_dir>`:
//!
//!   <stem>--EIX=<trbl>[--BGC=<hex>][--FGC=<hex>].<ext>
//!                          the input image, copied verbatim, renamed   (step 0)
//!                          with the shot code derived so far
//!   <stem>_1_segmask.png   BiRefNet foreground mask                    (step 1)
//!   <stem>_2_refine.png    white canvas the size of the segmask with   (step 2)
//!                          each gated edge's refined region pasted back
//!                          (only if an edge was gated)
//!
//! plus `zzz_results.json` (named to sort last). The `_<n>_` infix keeps
//! generated files sorted in pipeline order next to their source.
//!
//! Shot code (partial — BG type comes later):
//!   EIX  edge intersection, 4-bit TRBL (top-right-bottom-left). `1000`
//!        = top only, `0110` = right+bottom, `0000` = no intersection.
//!        Always the classifier's real verdict — a `full_bleed` flag
//!        (probable full-bleed close-up: tiny mask, no uniform background)
//!        is reported separately (console/JSON) and never overrides it.
//!   BGC  background colour: dominant colour of the inverse BiRefNet
//!        mask, emitted only when it covers >97.5% of that region.
//!   FGC  foreground colour: weighted-median colour of the largest colour
//!        blob inside the mask. Omitted if the subject is too small to
//!        sample.
//!   `--` separates the stem from the tags; `=` stands in for the spec's
//!   `:` (a colon is illegal in Windows filenames).
//!
//! The shot code is derived by `csp::core::shot_classifier::shotcode`, the
//! same code the pipeline exporter uses for its `CSP_DEBUG_TAGS` output.
//!
//! Usage:
//!   cargo run --release --example run_dir -- <input> [output_dir]
//!   <input> is a folder of images or a single image file.
//!
//! Paths (override via env):
//!   ORT_DYLIB_PATH   ONNX Runtime shared library  (default: ./onnxruntime.dll)
//!   BIREFNET_ONNX    BiRefNet model               (default: ./birefnet_lite_512.onnx)

use csp::core::shot_classifier::geometry::Rect;
use csp::core::shot_classifier::refine::{RefineParams, RefinementInput};
use csp::core::shot_classifier::shotcode::{self, ShotCode};
use csp::core::shot_classifier::{classify_instance, BiRefNetConfig, BiRefNetModel, Edge, SegmentationModel};
use image::{Rgb, RgbImage};
use std::path::{Path, PathBuf};

const DEFAULT_OUTPUT: &str = "test data/OUTPUT";

fn main() {
    let mut args = std::env::args().skip(1);
    let input = PathBuf::from(args.next().unwrap_or_else(|| {
        eprintln!("usage: run_dir <input> [output_dir]   (<input> = image folder or single image file)");
        std::process::exit(2);
    }));
    let output_dir = PathBuf::from(args.next().unwrap_or_else(|| DEFAULT_OUTPUT.into()));

    let ort_lib = std::env::var("ORT_DYLIB_PATH").unwrap_or_else(|_| "onnxruntime.dll".into());
    let model_path = std::env::var("BIREFNET_ONNX").unwrap_or_else(|_| "birefnet_lite_512.onnx".into());

    let meta = std::fs::metadata(&input)
        .unwrap_or_else(|e| panic!("cannot open {}: {e}", input.display()));
    let mut entries: Vec<PathBuf> = if meta.is_dir() {
        std::fs::read_dir(&input)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", input.display()))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| is_image(p))
            .collect()
    } else {
        vec![input.clone()]
    };
    entries.sort();

    if entries.is_empty() {
        eprintln!("no images found in {}", input.display());
        std::process::exit(1);
    }

    std::fs::create_dir_all(&output_dir).expect("could not create output dir");
    // Only wipe OUTPUT for a full-folder run; a single-file run just
    // overwrites that image's own outputs.
    if meta.is_dir() {
        let cleared = clear_output_files(&input, &output_dir);
        if cleared > 0 {
            eprintln!("cleared {cleared} file(s) from {}", output_dir.display());
        }
    }

    eprintln!("loading BiRefNet: {model_path}");
    eprintln!("onnx runtime:    {ort_lib}");
    let model = BiRefNetModel::load(&model_path, &ort_lib, BiRefNetConfig::default())
        .expect("failed to load BiRefNet model");

    let mut json_rows: Vec<String> = Vec::new();
    println!("\n{:<44}  {:>10}  {}", "image", "size", "shot code / touches");
    println!("{}", "-".repeat(96));

    for path in &entries {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("img");

        let image = match image::open(path) {
            Ok(img) => img.to_rgb8(),
            Err(e) => {
                println!("{file_name:<44}  {:>10}  <open failed: {e}>", "-");
                continue;
            }
        };

        let instances = match model.segment(&image) {
            Ok(instances) => instances,
            Err(e) => {
                println!("{file_name:<44}  {:>10}  <segment failed: {e}>", "-");
                continue;
            }
        };
        let inst = &instances[0]; // BiRefNet returns exactly one whole-image instance

        let fg = inst.mask.data.iter().filter(|&&v| v > 0).count();
        let total = (image.width() * image.height()) as usize;

        let input = RefinementInput { working_image: &image, original_image: &image, instance: inst };
        let result = classify_instance(&input, 20, RefineParams::default());

        // --- shot code: BGC/FGC from the mask, EIX from the verdict (shared with the pipeline) ---
        let sc = shotcode::derive(&image, &inst.mask, &result);
        let ShotCode { eix, bgc, fgc, full_bleed } = &sc;
        let fg_ratio = fg as f64 / total as f64;

        let edges: Vec<&str> = result.touches_edges.iter().map(edge_name).collect();
        let verdict = if edges.is_empty() { "none".to_string() } else { edges.join(", ") };
        let code = format!(
            "EIX={eix}{}{}",
            bgc.as_deref().map(|c| format!(" BGC={c}")).unwrap_or_default(),
            fgc.as_deref().map(|c| format!(" FGC={c}")).unwrap_or_default(),
        );
        println!(
            "{file_name:<44}  {:>10}  {code}   [{verdict}{}]",
            format!("{}x{}", image.width(), image.height()),
            if *full_bleed { " · full-bleed" } else { "" },
        );
        for r in &result.refinements {
            println!("    gate flagged {:<6} -> refined: touching={}", edge_name(&r.edge), r.touching);
        }

        // step 0: the input image, verbatim, renamed with the shot code.
        let tagged_name = format!("{stem}{}.{ext}", sc.tags());
        let _ = std::fs::copy(path, output_dir.join(&tagged_name));

        // step 1: BiRefNet foreground mask, full frame.
        let mut m = image::GrayImage::new(inst.mask.width, inst.mask.height);
        for (i, px) in m.pixels_mut().enumerate() {
            px.0[0] = inst.mask.data[i];
        }
        let _ = m.save(output_dir.join(format!("{stem}_1_segmask.png")));

        // step 2: one canvas the size of the segmask, white except for
        // the refined region of each gated edge, pasted back where it
        // sits in the frame (top region up top, bottom down low, etc.).
        let regions: Vec<Rect> = result
            .refinements
            .iter()
            .map(|r| r.crop_region)
            .filter(|r| r.w > 0 && r.h > 0)
            .collect();
        if !regions.is_empty() {
            let mut canvas =
                RgbImage::from_pixel(inst.mask.width, inst.mask.height, Rgb([255, 255, 255]));
            for reg in &regions {
                let patch = crop_region(&image, *reg);
                image::imageops::overlay(&mut canvas, &patch, reg.x as i64, reg.y as i64);
            }
            let _ = canvas.save(output_dir.join(format!("{stem}_2_refine.png")));
        }

        let gate_json: Vec<String> = result
            .refinements
            .iter()
            .map(|r| format!("{{\"edge\":\"{}\",\"touching\":{}}}", edge_name(&r.edge), r.touching))
            .collect();
        let opt_str = |o: &Option<String>| o.as_deref().map(|s| format!("{s:?}")).unwrap_or_else(|| "null".into());
        json_rows.push(format!(
            "  {{\"image\":{:?},\"width\":{},\"height\":{},\"foreground_ratio\":{:.4},\
             \"eix\":{:?},\"full_bleed\":{},\"bgc\":{},\"fgc\":{},\"touches_edges\":[{}],\"gate\":[{}]}}",
            file_name,
            image.width(),
            image.height(),
            fg_ratio,
            eix,
            full_bleed,
            opt_str(bgc),
            opt_str(fgc),
            edges.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>().join(","),
            gate_json.join(",")
        ));
    }

    let json = format!("[\n{}\n]\n", json_rows.join(",\n"));
    // Folder run -> the aggregate `zzz_results.json`; single-file run ->
    // `<stem>_results.json`, so it never clobbers a folder run's report.
    let json_name = if meta.is_dir() {
        "zzz_results.json".to_string()
    } else {
        format!("{}_results.json", input.file_stem().unwrap().to_string_lossy())
    };
    let json_path = output_dir.join(json_name);
    std::fs::write(&json_path, json).expect("could not write results json");

    println!("\nwrote {}", json_path.display());
    println!("images written to {}", output_dir.display());
}

fn crop_region(image: &RgbImage, r: Rect) -> RgbImage {
    image::imageops::crop_imm(image, r.x, r.y, r.w.max(1), r.h.max(1)).to_image()
}

fn is_image(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("jpg" | "jpeg" | "png" | "webp" | "bmp")
    )
}

/// Deletes regular files sitting directly in `output_dir` so a re-run
/// doesn't leave stale crops from a previous verdict. Subdirectories are
/// left untouched, and it refuses if output and input are the same folder.
fn clear_output_files(input_dir: &Path, output_dir: &Path) -> usize {
    let same = std::fs::canonicalize(input_dir).ok() == std::fs::canonicalize(output_dir).ok();
    if same {
        return 0;
    }
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(output_dir) {
        for entry in rd.flatten() {
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) && std::fs::remove_file(entry.path()).is_ok() {
                n += 1;
            }
        }
    }
    n
}

fn edge_name(e: &Edge) -> &'static str {
    match e {
        Edge::Top => "top",
        Edge::Bottom => "bottom",
        Edge::Left => "left",
        Edge::Right => "right",
    }
}
