//! Exporter: enforce the output size envelope, save to `PPRONI-OUTPUT`, then move the original out of
//! `PPRONI-INPUT` into `PPRONI-BACKUP`.
//!
//! Every route ends here, so the envelope is applied here — one place, no route can bypass it.
//! The move only runs after a successful save, so a write failure leaves the source untouched for a
//! retry. Originals are never deleted — they keep their name and extension under `PPRONI-BACKUP`, in
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
//! classifier's verdict, with the segmentation mask written alongside as `<stem>_segmask.jpg` —
//! upscaled (nearest-neighbour) to the output image's size and saved as sRGB JPEG, so it overlays
//! the tagged output pixel-for-pixel. When the classifier produced no verdict the name carries
//! `EIX=____` and no
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

/// Move the original to its `PPRONI-BACKUP` slot, creating the subfolder if needed. `rename` is the
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

/// Save `<stem>--EIX=...[...].<ext>` and, when there is a verdict, `<stem>_segmask.jpg` beside it.
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
        save_segmask(mask, img.width(), img.height(), &dir.join(format!("{stem}_segmask.jpg")))?;
    }

    Ok(())
}

/// The working-resolution mask, upscaled to the output image's size and written as an sRGB JPEG so
/// it lines up pixel-for-pixel with the tagged output beside it. Nearest-neighbour keeps the
/// subject/background boundary truthful — JPEG softens it a touch regardless. Grayscale is expanded
/// to RGB (0 = background) and saved through the same JPEG path as the outputs.
fn save_segmask(mask: &Mask, out_w: u32, out_h: u32, path: &Path) -> Result<(), String> {
    let mut gray = GrayImage::new(mask.width, mask.height);
    for (px, &v) in gray.pixels_mut().zip(mask.data.iter()) {
        px.0[0] = v;
    }
    let scaled =
        image::imageops::resize(&gray, out_w, out_h, image::imageops::FilterType::Nearest);

    let mut rgb = RgbImage::new(out_w, out_h);
    for (dst, src) in rgb.pixels_mut().zip(scaled.pixels()) {
        let v = src.0[0];
        dst.0 = [v, v, v];
    }
    save::save_jpeg_srgb(&rgb, path, config::JPEG_QUALITY)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The debug segmask is written as an sRGB JPEG at the *output* size (not the working
    /// resolution), with the mask nearest-upscaled so the subject/background boundary stays crisp.
    #[test]
    fn segmask_is_output_sized_srgb_jpeg_nearest() {
        // 2x2 mask: top row background (0), bottom row foreground (255).
        let mask = Mask { width: 2, height: 2, data: vec![0, 0, 255, 255] };
        let (out_w, out_h) = (8u32, 6u32);

        let path = std::env::temp_dir().join("csp_test_segmask.jpg");
        let _ = std::fs::remove_file(&path);
        save_segmask(&mask, out_w, out_h, &path).expect("write segmask");

        // Decodes as a valid image at the output size — not the 2x2 working size.
        let img = image::open(&path).expect("decode segmask").to_rgb8();
        assert_eq!(img.dimensions(), (out_w, out_h), "segmask must match output size");

        // Grayscale expanded to RGB: channels equal (JPEG 4:4:4 keeps this exact here).
        for px in img.pixels() {
            assert_eq!(px.0[0], px.0[1]);
            assert_eq!(px.0[1], px.0[2]);
        }

        // Nearest-neighbour block replication: the top half stays dark, the bottom half bright.
        let top = img.get_pixel(out_w / 2, 0).0[0] as i32;
        let bot = img.get_pixel(out_w / 2, out_h - 1).0[0] as i32;
        assert!(top < 40, "top (background) should be near black, got {top}");
        assert!(bot > 215, "bottom (foreground) should be near white, got {bot}");

        let _ = std::fs::remove_file(&path);
    }
}
