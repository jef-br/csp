//! CSP-Analyzer: dev-only replay harness. Calls the same Core functions the real pipeline calls
//! (unmodified) and writes the intermediate image after each internal decision point, for insight
//! into how the algorithm behaves. Not part of the app's normal run and does not go through
//! Exporter — output goes to its own directory, source files are never touched or deleted.
//!
//! Usage: csp_analyzer <input_file_or_dir> <output_dir>
//!   Single file or directory: same per-file logic either way, snapshots land flat in
//!   <output_dir> as "<stem>___<moduleindex>_<substep>_<TAG>.png" — no per-image subfolder,
//!   output mirrors input. Naming convention documented in docs/diagrams/README.md.
//!
//! Snapshot tag protocol: each TAG (PREPROC, SLIC, Geodesics, SEGMASK, DETECT, FILL, FINAL) names
//! a Rectangle
//! inside a Container on docs/diagrams/JBA2B.drawio.svg, tagged there with a matching bright-green
//! bold one-word prefix (see that file's mxCell "value" text). Adding a new snapshot moment here
//! means: tag its diagram rectangle the same way first, then reuse that tag as the filename's TAG,
//! picking a moduleindex/substep per docs/diagrams/README.md's naming-convention section.

use csp::core::config::MARGIN_FRACTION;
use csp::core::preprocessor as load;
use csp::core::processor::{fill, geometry, resize};
use csp::core::shot_classifier::{classify, detect};
use image::{Rgb, RgbImage};
use std::path::{Path, PathBuf};

const SUPPORTED: &[&str] = &["jpg", "jpeg", "png", "bmp", "tif", "tiff", "webp"];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: csp_analyzer <input_file_or_dir> <output_dir>");
        std::process::exit(1);
    }
    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);

    if input.is_dir() {
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

        std::fs::create_dir_all(&output).map_err(|e| format!("create output dir: {e}")).unwrap();
        for path in &files {
            if let Err(e) = analyze_one(path, &output) {
                eprintln!("{}: {e}", path.display());
            }
        }
    } else if let Err(e) = analyze_one(&input, &output) {
        eprintln!("{}: {e}", input.display());
        std::process::exit(1);
    }
}

// Replay the pipeline on one image, dumping a snapshot after each internal decision point directly
// into `out_dir` (no per-image subfolder — output mirrors input's flat structure). Every filename
// is "<stem>___<moduleindex>_<substep>_<TAG>.png" (see docs/diagrams/README.md), so sorting
// filenames alphabetically reproduces pipeline order: the moduleindex digit orders containers,
// the substep (gapped by 10s, so a later insertion never renumbers its neighbors) orders steps
// within a container, and TAG is the green-bold tag on that stage's Rectangle in
// docs/diagrams/JBA2B.drawio.svg.
fn analyze_one(input: &Path, out_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("create output dir: {e}"))?;
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("image");

    // --- Preprocessor: no internal sub-steps are exposed today, so one snapshot of its output.
    // Diagram tag: PREPROC (both "Colour-manage to sRGB" and "Leave colour untouched" leaf boxes). ---
    let loaded = load::load_image(input)?;
    save(&loaded.rgb, out_dir, stem, "2-prep_10_PREPROC")?;

    // --- Shot Classifier: SLIC superpixels (tag: SLIC) feed the geodesic distance-to-border (tag:
    // Geodesics — user-authored diagram tag, not all-caps like the others; kept verbatim), which
    // gets thresholded into the live segmentation mask (tag: SEGMASK, the "Threshold + Mask" box),
    // from which the final box (tag: DETECT) is derived — substeps 02/05/10/20 preserve that real
    // execution order. ---
    let slic = detect::debug_slic(&loaded.rgb);
    save(&slic, out_dir, stem, "3-class_02_SLIC")?;

    let geodesic = detect::debug_geodesic(&loaded.rgb);
    save(&geodesic, out_dir, stem, "3-class_05_Geodesics")?;

    let det = classify(&loaded.rgb, loaded.alpha.as_ref());

    let mask = detect::debug_segment(&loaded.rgb, false);
    save(&mask, out_dir, stem, "3-class_10_SEGMASK")?;

    let mut overlay = loaded.rgb.clone();
    let b = det.box_;
    let margin = (b.longest_side() as f64 * MARGIN_FRACTION).round() as i32;
    draw_rect(&mut overlay, b.x, b.y, b.w, b.h, Rgb([255, 0, 0]));
    draw_rect(&mut overlay, b.x - margin, b.y - margin, b.w + 2 * margin, b.h + 2 * margin, Rgb([0, 200, 0]));
    save(&overlay, out_dir, stem, "3-class_20_DETECT")?;

    // --- Processor: after fill (tag: FILL, "Crop the original to the layout" box) and after final
    // resize (tag: FINAL, "Final square image -> Exporter" box). ---
    let (w, h) = (loaded.rgb.width() as i32, loaded.rgb.height() as i32);
    let mut layout = geometry::plan(&det, w, h);
    let min_fill = resize::min_fill_side();
    if !layout.already_square && layout.side < min_fill {
        layout.side = min_fill;
    }

    let square = fill::render(&loaded.rgb, &layout);
    save(&square, out_dir, stem, "4-proc_10_FILL")?;


    println!(
        "{}\n  kind={:?} conf={:.2} shadow={:.3}\n  box=({},{},{},{}) intersects T{} B{} L{} R{} frame={}x{}\n  layout side={} already_square={}",
        input.display(), det.kind, det.confidence, det.hard_shadow_fraction,
        b.x, b.y, b.w, b.h,
        det.intersects.top as u8, det.intersects.bottom as u8, det.intersects.left as u8, det.intersects.right as u8,
        w, h,
        layout.side, layout.already_square,
    );
    println!("  -> {}", out_dir.display());

    Ok(())
}

fn save<P>(img: &image::ImageBuffer<P, Vec<u8>>, dir: &Path, stem: &str, suffix: &str) -> Result<(), String>
where
    P: image::Pixel<Subpixel = u8> + image::PixelWithColorType,
    [u8]: image::EncodableLayout,
{
    let name = format!("{stem}___{suffix}.png");
    img.save(dir.join(&name)).map_err(|e| format!("save {name}: {e}"))
}

fn draw_rect(img: &mut RgbImage, x: i32, y: i32, w: i32, h: i32, c: Rgb<u8>) {
    let (iw, ih) = (img.width() as i32, img.height() as i32);
    let th = (iw.max(ih) / 300).max(2);
    for t in 0..th {
        for xx in x..(x + w) {
            put(img, xx, y + t, c, iw, ih);
            put(img, xx, y + h - 1 - t, c, iw, ih);
        }
        for yy in y..(y + h) {
            put(img, x + t, yy, c, iw, ih);
            put(img, x + w - 1 - t, yy, c, iw, ih);
        }
    }
}

fn put(img: &mut RgbImage, x: i32, y: i32, c: Rgb<u8>, iw: i32, ih: i32) {
    if x >= 0 && y >= 0 && x < iw && y < ih {
        img.put_pixel(x as u32, y as u32, c);
    }
}
