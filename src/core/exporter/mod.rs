//! Exporter: enforce the output size envelope, save to `CSP-OUTPUT`, then remove the original from
//! `CSP-INPUT`.
//!
//! Every route ends here, so the envelope is applied here — one place, no route can bypass it.
//! Delete only runs after a successful save, so a write failure leaves the source untouched for a
//! retry.
//!
//! ## Debug tags (`CSP_DEBUG_TAGS` marker file)
//!
//! Off by default — the shipped exe writes a clean `<stem>.jpg`. Turned on by a file named
//! `CSP_DEBUG_TAGS` sitting next to the executable — no shell, no environment variable: the same
//! sidecar convention `core::sidecar` already uses to find `onnxruntime.dll`/the `.onnx` model.
//! The file's content is ignored; only its presence matters. Delete it, or the exe, to turn tagging
//! back off.
//!
//! When on, each output is renamed `<stem>--EIX=<trbl>[--BGC=<hex>][--FGC=<hex>]` from the shot
//! classifier's verdict, with the working-resolution segmentation mask written alongside as
//! `<stem>_segmask.png`. When the classifier produced no verdict the name carries `EIX=____` and no
//! segmask is written — that happens both for a single failed inference *and*, systematically for
//! every image, when the model/runtime sidecar files aren't next to the exe. A run of all-`EIX=____`
//! files means "the classifier never ran", not "every image was unreadable" — check the sidecar
//! files before suspecting the classifier itself.

pub mod icc;
pub mod resize;
pub mod save;

use super::config;
use super::shot_classifier::shotcode::{self, NO_VERDICT_EIX};
use super::shot_classifier::{Mask, ShotClassification};
use image::{GrayImage, RgbImage};
use std::path::Path;
use std::sync::OnceLock;

/// Everything the debug tags need from the classifier, threaded through from `core::process_file`.
/// `None` at the call site means "no verdict".
pub struct ShotInputs<'a> {
    /// The working-resolution image the mask lines up with — BGC/FGC are sampled from it.
    pub working: &'a RgbImage,
    /// The working-resolution segmentation mask.
    pub mask: &'a Mask,
    /// The edge verdict.
    pub class: &'a ShotClassification,
}

pub fn export(img: &RgbImage, dest: &Path, source: &Path, shot: Option<ShotInputs<'_>>) -> Result<(), String> {
    let sized = resize::to_envelope(img);

    if tags_enabled() {
        write_tagged(&sized, dest, shot)?;
    } else {
        save::save_jpeg_srgb(&sized, dest, config::JPEG_QUALITY)?;
    }

    std::fs::remove_file(source).map_err(|e| format!("delete original: {e}"))
}

/// A `CSP_DEBUG_TAGS` file next to the running executable turns on filename tagging and the
/// per-image segmask. Read once.
fn tags_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("CSP_DEBUG_TAGS")))
            .is_some_and(|p| p.is_file())
    })
}

/// Save `<stem>--EIX=...[...].<ext>` and, when there is a verdict, `<stem>_segmask.png` beside it.
/// With no verdict there is no mask: the name carries `EIX=____` and no segmask is written.
fn write_tagged(img: &RgbImage, dest: &Path, shot: Option<ShotInputs<'_>>) -> Result<(), String> {
    let dir = dest.parent().unwrap_or_else(|| Path::new("."));
    let stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    let ext = dest.extension().and_then(|s| s.to_str()).unwrap_or("jpg");

    let (tags, mask) = match &shot {
        Some(s) => (shotcode::derive(s.working, s.mask, s.class).tags(), Some(s.mask)),
        None => (format!("--EIX={NO_VERDICT_EIX}"), None),
    };

    let image_path = dir.join(format!("{stem}{tags}.{ext}"));
    save::save_jpeg_srgb(img, &image_path, config::JPEG_QUALITY)?;

    if let Some(mask) = mask {
        save_segmask(mask, &dir.join(format!("{stem}_segmask.png")))?;
    }

    Ok(())
}

/// The working-resolution mask as an 8-bit grayscale PNG (0 = background).
fn save_segmask(mask: &Mask, path: &Path) -> Result<(), String> {
    let mut img = GrayImage::new(mask.width, mask.height);
    for (px, &v) in img.pixels_mut().zip(mask.data.iter()) {
        px.0[0] = v;
    }
    img.save(path).map_err(|e| format!("segmask: {e}"))
}
