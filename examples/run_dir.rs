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
//!        Forced to `1111` for a full-bleed close-up (tiny mask + no
//!        uniform background).
//!   BGC  background colour: dominant colour of the inverse BiRefNet
//!        mask, emitted only when it covers >97.5% of that region.
//!   FGC  foreground colour: weighted-median colour of the largest colour
//!        blob inside the mask. Omitted if the subject is too small to
//!        sample.
//!   `--` separates the stem from the tags; `=` stands in for the spec's
//!   `:` (a colon is illegal in Windows filenames).
//!
//! Usage:
//!   cargo run --release --features birefnet --example run_dir -- <input> [output_dir]
//!   <input> is a folder of images or a single image file.
//!
//! Paths (override via env):
//!   ORT_DYLIB_PATH   ONNX Runtime shared library  (default: ./onnxruntime.dll)
//!   BIREFNET_ONNX    BiRefNet model               (default: ./birefnet_lite_512.onnx)

use csp::core::shot_classifier::geometry::Rect;
use csp::core::shot_classifier::refine::{RefineParams, RefinementInput};
use csp::core::shot_classifier::{classify_instance, BiRefNetConfig, BiRefNetModel, Edge, Mask, SegmentationModel};
use image::{Rgb, RgbImage};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const DEFAULT_OUTPUT: &str = "test data/OUTPUT";

/// A colour must occupy at least this fraction of the background region
/// (inverse mask) to be reported as BGC.
const BGC_PRESENCE: f64 = 0.975;
/// A region smaller than this fraction of the frame is too small to
/// sample a colour from.
const MIN_REGION_FRAC: f64 = 0.01;
/// If the BiRefNet mask covers less than this fraction of the frame *and*
/// no uniform background was found, the shot is treated as a full-bleed
/// close-up: the subject fills the frame, so EIX is forced to `1111`.
/// Interim rule — the texture / BG-type pass will replace it.
const FULL_BLEED_MAX_FG: f64 = 0.05;

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

        // --- shot code so far: BGC/FGC from the mask, EIX from the verdict ---
        let ShotColors { bgc, fgc } = shot_colors(&image, &inst.mask);
        let fg_ratio = fg as f64 / total as f64;

        // Full-bleed close-up: the segmenter found almost no subject and
        // there is no uniform background -> the whole frame is subject.
        let full_bleed = fg_ratio < FULL_BLEED_MAX_FG && bgc.is_none();
        let eix = if full_bleed { "1111".to_string() } else { eix_bits(&result.touches_edges) };

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
            if full_bleed { " · full-bleed" } else { "" },
        );
        for r in &result.refinements {
            println!("    gate flagged {:<6} -> refined: touching={}", edge_name(&r.edge), r.touching);
        }

        // step 0: the input image, verbatim, renamed with the shot code.
        let mut tags = vec![format!("EIX={eix}")];
        if let Some(c) = &bgc {
            tags.push(format!("BGC={c}"));
        }
        if let Some(c) = &fgc {
            tags.push(format!("FGC={c}"));
        }
        let tagged_name = format!("{stem}--{}.{ext}", tags.join("--"));
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
            opt_str(&bgc),
            opt_str(&fgc),
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

/// 4-bit edge-intersection string in fixed TRBL order.
fn eix_bits(edges: &[Edge]) -> String {
    let bit = |e: Edge| if edges.contains(&e) { '1' } else { '0' };
    [bit(Edge::Top), bit(Edge::Right), bit(Edge::Bottom), bit(Edge::Left)]
        .into_iter()
        .collect()
}

struct ShotColors {
    bgc: Option<String>,
    fgc: Option<String>,
}

/// BGC and FGC, derived from the original image partitioned by the
/// BiRefNet mask (background = inverse mask, foreground = mask).
fn shot_colors(image: &RgbImage, mask: &Mask) -> ShotColors {
    let (mut fg_hist, mut bg_hist) = (Hist::new(), Hist::new());
    let w = image.width().min(mask.width);
    let h = image.height().min(mask.height);
    for y in 0..h {
        for x in 0..w {
            let p = image.get_pixel(x, y).0;
            if mask.get(x, y) {
                fg_hist.add(p);
            } else {
                bg_hist.add(p);
            }
        }
    }
    let frame = (w as u64 * h as u64).max(1) as f64;

    // BGC: a single colour must cover >97.5% of the inverse-mask region.
    let bgc = (bg_hist.total() as f64 >= frame * MIN_REGION_FRAC)
        .then(|| dominant_cluster(&bg_hist))
        .flatten()
        .filter(|(frac, _)| *frac >= BGC_PRESENCE)
        .map(|(_, c)| hex(c));

    // FGC: the largest colour blob inside the mask -> its weighted-median
    // colour. "Blob" is the fullest histogram cluster (the winning 5-bit
    // bin plus its ±1 neighbours), not a spatially-connected region.
    let fgc = (fg_hist.total() as f64 >= frame * MIN_REGION_FRAC)
        .then(|| dominant_bin(&fg_hist))
        .flatten()
        .map(|top| {
            let blob = cluster_bins(top);
            // Per-channel value histograms over the blob's pixels, for a
            // frequency-weighted median that shrugs off folds/shadows.
            let mut chan = [[0u64; 256]; 3];
            for y in 0..h {
                for x in 0..w {
                    if !mask.get(x, y) {
                        continue;
                    }
                    let p = image.get_pixel(x, y).0;
                    if blob.contains(&bin(p[0] >> 3, p[1] >> 3, p[2] >> 3)) {
                        for c in 0..3 {
                            chan[c][p[c] as usize] += 1;
                        }
                    }
                }
            }
            hex([median(&chan[0]), median(&chan[1]), median(&chan[2])])
        });

    ShotColors { bgc, fgc }
}

/// Frequency-weighted median of a 256-bin value histogram.
fn median(hist: &[u64; 256]) -> u8 {
    let half = (hist.iter().sum::<u64>() + 1) / 2;
    let mut cum = 0u64;
    for (v, &c) in hist.iter().enumerate() {
        cum += c;
        if cum >= half {
            return v as u8;
        }
    }
    0
}

/// 5-bit-per-channel RGB histogram (32³ bins) that also carries the
/// running colour sum per bin, so a bin's mean colour is recoverable.
struct Hist {
    count: Vec<u32>,
    sum: Vec<[u64; 3]>,
}

impl Hist {
    fn new() -> Self {
        Hist { count: vec![0; 32 * 32 * 32], sum: vec![[0; 3]; 32 * 32 * 32] }
    }
    fn add(&mut self, p: [u8; 3]) {
        let i = bin(p[0] >> 3, p[1] >> 3, p[2] >> 3);
        self.count[i] += 1;
        self.sum[i][0] += p[0] as u64;
        self.sum[i][1] += p[1] as u64;
        self.sum[i][2] += p[2] as u64;
    }
    fn total(&self) -> u64 {
        self.count.iter().map(|&c| c as u64).sum()
    }
}

fn bin(r: u8, g: u8, b: u8) -> usize {
    ((r as usize) << 10) | ((g as usize) << 5) | (b as usize)
}

/// Index of the fullest histogram bin, or `None` if the region is empty.
fn dominant_bin(h: &Hist) -> Option<usize> {
    if h.total() == 0 {
        return None;
    }
    h.count.iter().enumerate().max_by_key(|(_, &c)| c).map(|(i, _)| i)
}

/// The bin set forming one colour "blob": `top` plus its ±1 neighbours
/// per channel, so JPEG dither around a boundary stays one colour.
fn cluster_bins(top: usize) -> HashSet<usize> {
    let (tr, tg, tb) = ((top >> 10) & 31, (top >> 5) & 31, top & 31);
    let mut set = HashSet::new();
    for dr in -1..=1i32 {
        for dg in -1..=1i32 {
            for db in -1..=1i32 {
                set.insert(bin(
                    (tr as i32 + dr).clamp(0, 31) as u8,
                    (tg as i32 + dg).clamp(0, 31) as u8,
                    (tb as i32 + db).clamp(0, 31) as u8,
                ));
            }
        }
    }
    set
}

/// Fraction of the region occupied by the dominant colour blob, and that
/// blob's mean RGB. `None` if the region is empty.
fn dominant_cluster(h: &Hist) -> Option<(f64, [u8; 3])> {
    let region = h.total();
    let top = dominant_bin(h)?;
    let (mut n, mut s) = (0u64, [0u64; 3]);
    for i in cluster_bins(top) {
        n += h.count[i] as u64;
        s[0] += h.sum[i][0];
        s[1] += h.sum[i][1];
        s[2] += h.sum[i][2];
    }
    (n > 0).then(|| (n as f64 / region as f64, [(s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8]))
}

fn hex(c: [u8; 3]) -> String {
    format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2])
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
