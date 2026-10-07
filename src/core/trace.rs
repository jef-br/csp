//! Debug trace: save every intermediate image of a run, so a bad result can be followed back to the
//! stage that caused it.
//!
//! Debug builds only. In a release build every function here is an empty stub, so the shipped exe
//! carries none of this: no stage labels, no file writing, no environment variable name.
//!
//! **Turning it on.** Set `CSP_TRACE_DIR` to a folder. With it unset (the normal case) nothing is
//! written and the cost is one environment lookup per image. The folder is created if missing but
//! never emptied here — emptying it is the launch task's job (`.vscode/tasks.json`), so a stray
//! `cargo run` can't delete anything.
//!
//! **What gets written**, per image, flat in that folder:
//! * `<stem>  -  <NN>.jpg` — one file per stage; `NN` comes from [`stage`]. Numbers are fixed per
//!   stage, not counted per run, so `10` is the model input on every image and a skipped stage
//!   leaves a gap rather than renumbering its neighbours.
//! * `<stem>  -  log.txt` — the stages that were written, in order, plus the numbers behind each
//!   decision (mask coverage, which edges the gate flagged, the graze ratio, the route and why).
//!
//! **How to add a stage.** Pick the next free number in [`stage`], then call [`image`] where the
//! picture exists, passing a closure that builds it. The closure only runs while tracing, so
//! building an overlay costs nothing in a normal run.

use super::shot_classifier::geometry::Rect;
use super::shot_classifier::{Edge, Mask};
use image::{Rgb, RgbImage};
use std::path::Path;

/// Stage numbers. A group takes a block of ten so a stage can be added without renumbering.
pub mod stage {
    /// Decoded, EXIF-rotated, alpha flattened onto white. Full resolution.
    pub const ORIGINAL: u8 = 1;
    /// After colour management into sRGB. Only written when the file carried an ICC profile.
    pub const ORIGINAL_SRGB: u8 = 2;
    /// The working-resolution copy the model runs on.
    pub const WORKING: u8 = 3;
    /// The working copy after the colour optimiser — what BiRefNet actually sees.
    pub const OPTIMIZED: u8 = 4;

    /// The primary BiRefNet model: seven stages, `PRIMARY + 0..=6` (see [`super::MODEL_STEPS`]).
    pub const PRIMARY: u8 = 10;
    /// The fallback model, same seven stages. Only present when the primary found no subject.
    pub const FALLBACK: u8 = 20;

    /// Pass-1 gate: the border bands, red where foreground was found.
    pub const GATE: u8 = 30;
    /// Pass-2 refinement, one block per edge: `REFINE + 10 * slot + 0..=3`
    /// (crop, upsampled mask, trimap, refined mask). Slots follow [`super::edge_slot`].
    pub const REFINE: u8 = 40;
    /// The edge verdict drawn on the working image.
    pub const VERDICT: u8 = 80;

    /// R1: the subject box and the square planned around it, drawn on the original.
    pub const R1_PLAN: u8 = 90;
    /// R1: the picture after the horizontal stretch, before the vertical one.
    pub const R1_STRETCH: u8 = 91;
    /// Whatever the chosen route produced, before the output size envelope.
    pub const ROUTE_OUT: u8 = 92;
    /// The final image, as it is saved.
    pub const FINAL: u8 = 99;
}

/// The seven stages of one BiRefNet run, as offsets from [`stage::PRIMARY`] or [`stage::FALLBACK`].
pub const MODEL_STEPS: [&str; 7] = [
    "model input: working image squashed to the model's square",
    "model input after ImageNet normalisation (stretched to be visible)",
    "raw model output (logits, stretched)",
    "after sigmoid: probability of subject, 0 black to 1 white",
    "thresholded at the model's own resolution",
    "mask resampled to working resolution (what the rest of the pipeline uses)",
    "that mask on the working image (everything outside it dimmed blue)",
];

/// Where an edge's refinement block starts: `stage::REFINE + 10 * edge_slot(edge)`.
pub fn edge_slot(edge: Edge) -> u8 {
    match edge {
        Edge::Top => 0,
        Edge::Bottom => 1,
        Edge::Left => 2,
        Edge::Right => 3,
    }
}

/// The share of the frame a mask covers, as a percentage.
pub fn coverage_pct(mask: &Mask) -> f64 {
    let fg = mask.data.iter().filter(|&&v| v > 0).count();
    100.0 * fg as f64 / (mask.data.len().max(1)) as f64
}

// ---- the switch -------------------------------------------------------------------------------

/// Held for the length of one image's run. Dropping it writes that image's `log.txt`.
pub struct Session {
    #[cfg(debug_assertions)]
    owns: bool,
}

#[cfg(debug_assertions)]
mod live {
    use std::cell::RefCell;
    use std::path::PathBuf;

    pub struct State {
        pub dir: PathBuf,
        pub stem: String,
        pub log: Vec<String>,
    }

    // One image runs start to finish on one thread (rayon fans out across images, not inside one),
    // so a thread-local is the right scope: parallel images never mix their logs.
    thread_local! {
        pub static CURRENT: RefCell<Option<State>> = const { RefCell::new(None) };
    }
}

/// Start tracing one image. Inert unless `CSP_TRACE_DIR` is set (and always inert in release).
pub fn begin(source: &Path) -> Session {
    #[cfg(debug_assertions)]
    {
        let Some(dir) = std::env::var_os("CSP_TRACE_DIR") else {
            return Session { owns: false };
        };
        let already = live::CURRENT.with(|c| c.borrow().is_some());
        if already {
            return Session { owns: false };
        }
        let dir = std::path::PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&dir);
        let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("image").to_owned();
        live::CURRENT.with(|c| *c.borrow_mut() = Some(live::State { dir, stem, log: Vec::new() }));
        Session { owns: true }
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = source;
        Session {}
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if self.owns {
            if let Some(state) = live::CURRENT.with(|c| c.borrow_mut().take()) {
                let path = state.dir.join(format!("{}  -  log.txt", state.stem));
                let _ = std::fs::write(path, state.log.join("\n") + "\n");
            }
        }
    }
}

/// Is a trace running on this thread? For callers that would otherwise build something expensive
/// just to hand it to [`image`] or [`note`] — normally the closures make this unnecessary.
#[allow(dead_code)]
pub fn active() -> bool {
    #[cfg(debug_assertions)]
    {
        live::CURRENT.with(|c| c.borrow().is_some())
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

/// Save one stage as `<stem>  -  <index>.jpg`. `make` runs only while tracing.
pub fn image(index: u8, label: &str, make: impl FnOnce() -> RgbImage) {
    #[cfg(debug_assertions)]
    live::CURRENT.with(|c| {
        // Take the state out while `make` runs: it may itself call back into this module.
        let Some(mut state) = c.borrow_mut().take() else { return };
        let img = make();
        let path = state.dir.join(format!("{}  -  {index:02}.jpg", state.stem));
        let saved = super::exporter::save::save_jpeg_srgb(&img, &path, 90);
        state.log.push(format!(
            "{index:02}  {}{label}  ({}x{}){}",
            group(index),
            img.width(),
            img.height(),
            if saved.is_err() { "  [SAVE FAILED]" } else { "" }
        ));
        *c.borrow_mut() = Some(state);
    });
    #[cfg(not(debug_assertions))]
    {
        let _ = (index, label, make);
    }
}

/// Add a line to this image's `log.txt`. `make` runs only while tracing.
pub fn note(make: impl FnOnce() -> String) {
    #[cfg(debug_assertions)]
    live::CURRENT.with(|c| {
        let text = {
            if c.borrow().is_none() {
                return;
            }
            make()
        };
        if let Some(state) = c.borrow_mut().as_mut() {
            state.log.push(format!("      {text}"));
        }
    });
    #[cfg(not(debug_assertions))]
    {
        let _ = make;
    }
}

/// Which block a stage number belongs to, as a log prefix.
#[cfg(debug_assertions)]
fn group(index: u8) -> String {
    match index {
        10..=19 => "[primary model] ".into(),
        20..=29 => "[fallback model] ".into(),
        40..=79 => {
            let edge = match (index - 40) / 10 {
                0 => "Top",
                1 => "Bottom",
                2 => "Left",
                _ => "Right",
            };
            let step = ["crop of the original", "mask upsampled into the crop", "trimap (white = subject, grey = unknown, black = background)", "refined mask"]
                [((index - 40) % 10).min(3) as usize];
            format!("[refine {edge}] {step}")
        }
        _ => String::new(),
    }
}

// ---- picture builders -------------------------------------------------------------------------
//
// Plain helpers that turn the pipeline's own data into something viewable. They are only ever
// called from inside the closures above, so they cost nothing in a normal run.

/// A mask as a black-and-white picture.
pub fn mask_image(mask: &Mask) -> RgbImage {
    let mut img = RgbImage::new(mask.width.max(1), mask.height.max(1));
    for (px, &v) in img.pixels_mut().zip(&mask.data) {
        *px = Rgb([v, v, v]);
    }
    img
}

/// Greyscale from raw values, stretched so the smallest becomes black and the largest white.
/// (Logits have no fixed range, so a fixed scale would leave most images unreadable.)
pub fn stretched_gray(values: &[f32], w: u32, h: u32) -> RgbImage {
    let (lo, hi) = values.iter().fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    let span = (hi - lo).max(f32::EPSILON);
    let mut img = RgbImage::new(w, h);
    for (px, &v) in img.pixels_mut().zip(values) {
        let g = (((v - lo) / span) * 255.0).round() as u8;
        *px = Rgb([g, g, g]);
    }
    img
}

/// Greyscale from values already in 0..=1.
pub fn unit_gray(values: &[f32], w: u32, h: u32) -> RgbImage {
    let mut img = RgbImage::new(w, h);
    for (px, &v) in img.pixels_mut().zip(values) {
        let g = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        *px = Rgb([g, g, g]);
    }
    img
}

/// A three-channel planar (CHW) tensor as a colour picture, stretched over its whole range.
pub fn stretched_chw(chw: &[f32], w: u32, h: u32) -> RgbImage {
    let plane = (w * h) as usize;
    let (lo, hi) = chw.iter().fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    let span = (hi - lo).max(f32::EPSILON);
    let to_u8 = |v: f32| (((v - lo) / span) * 255.0).round() as u8;
    let mut img = RgbImage::new(w, h);
    for (i, px) in img.pixels_mut().enumerate() {
        *px = Rgb([to_u8(chw[i]), to_u8(chw[plane + i]), to_u8(chw[2 * plane + i])]);
    }
    img
}

/// `base` with everything outside the mask dimmed and tinted blue, so the silhouette can be judged
/// against the picture. The subject keeps its own colours — tinting it instead hid red products.
pub fn tint(base: &RgbImage, mask: &Mask) -> RgbImage {
    let mut out = base.clone();
    for y in 0..out.height() {
        for x in 0..out.width() {
            if !mask.get(x * mask.width / out.width().max(1), y * mask.height / out.height().max(1)) {
                let Rgb([r, g, b]) = *out.get_pixel(x, y);
                out.put_pixel(x, y, Rgb([r / 3, g / 3, b / 3 + 110]));
            }
        }
    }
    out
}

/// A hollow rectangle, clipped to the picture. `(x, y)` may be negative.
pub fn outline(img: &mut RgbImage, x: i64, y: i64, w: u32, h: u32, colour: [u8; 3], thick: u32) {
    let (iw, ih) = (img.width() as i64, img.height() as i64);
    let mut paint = |px: i64, py: i64| {
        if px >= 0 && py >= 0 && px < iw && py < ih {
            img.put_pixel(px as u32, py as u32, Rgb(colour));
        }
    };
    let (w, h, t) = (w as i64, h as i64, thick as i64);
    for d in 0..t {
        for i in 0..w {
            paint(x + i, y + d);
            paint(x + i, y + h - 1 - d);
        }
        for j in 0..h {
            paint(x + d, y + j);
            paint(x + w - 1 - d, y + j);
        }
    }
}

/// The pass-1 gate: the mask tinted, and each edge's band outlined — red if the gate found
/// foreground in it (so pass 2 runs), green if not.
pub fn gate_view(working: &RgbImage, mask: &Mask, margin_px: u32, near: &[Edge]) -> RgbImage {
    let mut img = tint(working, mask);
    let (w, h) = (img.width(), img.height());
    let m = margin_px.min(w).min(h);
    for edge in Edge::ALL {
        let colour = if near.contains(&edge) { [255, 0, 0] } else { [0, 200, 0] };
        let (x, y, bw, bh) = match edge {
            Edge::Top => (0, 0, w, m),
            Edge::Bottom => (0, h - m, w, m),
            Edge::Left => (0, 0, m, h),
            Edge::Right => (w - m, 0, m, h),
        };
        outline(&mut img, x as i64, y as i64, bw, bh, colour, 2);
    }
    img
}

/// The edge verdict: a thick bar along each edge — red where the subject touches it, orange where
/// the gate flagged it but refinement said no, nothing where the gate never looked.
pub fn verdict_view(working: &RgbImage, mask: &Mask, near: &[Edge], touching: &[Edge]) -> RgbImage {
    let mut img = tint(working, mask);
    let (w, h) = (img.width(), img.height());
    let bar = (w.max(h) / 100).max(4);
    for edge in near {
        let colour = if touching.contains(edge) { [255, 0, 0] } else { [255, 165, 0] };
        let (x, y, bw, bh) = match edge {
            Edge::Top => (0, 0, w, bar),
            Edge::Bottom => (0, h.saturating_sub(bar), w, bar),
            Edge::Left => (0, 0, bar, h),
            Edge::Right => (w.saturating_sub(bar), 0, bar, h),
        };
        for py in y..(y + bh).min(h) {
            for px in x..(x + bw).min(w) {
                img.put_pixel(px, py, Rgb(colour));
            }
        }
    }
    img
}

/// R1's plan on the original: the subject box (cyan) and the square cut around it (yellow). The
/// square can reach past the picture, so the picture sits on a grey canvas padded to hold all of it
/// — the grey is exactly the part R1 has to invent by stretching.
pub fn plan_view(original: &RgbImage, subject: Rect, sq_x: i64, sq_y: i64, side: u32) -> RgbImage {
    let (w, h) = (original.width() as i64, original.height() as i64);
    let pad_l = (-sq_x).max(0);
    let pad_t = (-sq_y).max(0);
    let pad_r = (sq_x + side as i64 - w).max(0);
    let pad_b = (sq_y + side as i64 - h).max(0);

    let mut canvas = RgbImage::from_pixel(
        (w + pad_l + pad_r) as u32,
        (h + pad_t + pad_b) as u32,
        Rgb([128, 128, 128]),
    );
    image::imageops::replace(&mut canvas, original, pad_l, pad_t);

    let thick = ((w.max(h) / 400).max(2)) as u32;
    outline(&mut canvas, pad_l + subject.x as i64, pad_t + subject.y as i64, subject.w, subject.h, [0, 255, 255], thick);
    outline(&mut canvas, pad_l + sq_x, pad_t + sq_y, side, side, [255, 255, 0], thick);
    canvas
}
