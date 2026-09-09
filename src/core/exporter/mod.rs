//! Exporter: enforce the output size envelope, save to `CSP-OUTPUT`, then move the original out of
//! `CSP-INPUT` into `CSP-BACKUP`.
//!
//! Every route ends here, so the envelope is applied here — one place, no route can bypass it.
//! The move only runs after a successful save, so a write failure leaves the source untouched for a
//! retry. Originals are never deleted — they keep their name and extension under `CSP-BACKUP`, in
//! the same subfolder layout the output uses.
//!
//! ## Debug tags (`CSP_DEBUG_TAGS` marker file)
//!
//! Off by default — the shipped exe writes a clean `<stem>.jpg`. Turned on by a file named
//! `CSP_DEBUG_TAGS` sitting next to the executable — no shell, no environment variable, because
//! CSP ships as one file with nothing to configure. The file's content is ignored; only its
//! presence matters. Delete it to turn tagging back off. This marker is the *only* thing CSP ever
//! looks for beside the exe; the runtime and the weights are inside it.
//!
//! When on, each output is renamed `<stem>--EIX=<trbl>[--BGC=<hex>][--FGC=<hex>]` from the shot
//! classifier's verdict, with the working-resolution segmentation mask written alongside as
//! `<stem>_segmask.png`. When the classifier produced no verdict the name carries `EIX=____` and no
//! segmask is written. Since the model is embedded and `core::preflight` aborts the run if its
//! session will not build, a systematic run of all-`EIX=____` files no longer means a broken
//! install — it points at the classifier or the images themselves.

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

pub fn export(
    img: &RgbImage,
    dest: &Path,
    source: &Path,
    backup: &Path,
    shot: Option<ShotInputs<'_>>,
) -> Result<(), String> {
    let sized = resize::to_envelope(img);

    if tags_enabled() {
        write_tagged(&sized, dest, shot)?;
    } else {
        save::save_jpeg_srgb(&sized, dest, config::JPEG_QUALITY)?;
    }

    move_to_backup(source, backup)
}

/// Move the original to its `CSP-BACKUP` slot, creating the subfolder if needed. `rename` is the
/// fast path; when it fails (a different volume, typically) fall back to copy-then-remove so the
/// original still leaves the input folder.
fn move_to_backup(source: &Path, backup: &Path) -> Result<(), String> {
    if let Some(parent) = backup.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("backup folder: {e}"))?;
    }
    if std::fs::rename(source, backup).is_ok() {
        return Ok(());
    }
    std::fs::copy(source, backup).map_err(|e| format!("back up original: {e}"))?;
    std::fs::remove_file(source).map_err(|e| format!("remove original after backup: {e}"))
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
