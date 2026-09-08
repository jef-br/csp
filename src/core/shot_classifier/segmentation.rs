//! Segmentation types.
//!
//! [`SegmentationModel`] deliberately abstracts over the actual BiRefNet
//! ONNX inference call. Everything else here depends only on the
//! [`Mask`]/[`Instance`] shapes, not on how they were produced.

use super::geometry::{Grid, Rect};

/// A binary instance mask in *working resolution* coordinates — i.e. the
/// preprocessor's output space (the 1020px-longest-side working image),
/// which is what the segmentation model actually ran on.
///
/// BiRefNet produces its mask at a fixed internal resolution (1024x1024)
/// and it is resized to this size; that resampling is why the mask
/// boundary is not pixel-accurate, and why the pass-2 refinement in
/// [`super::refine`] exists at all.
#[derive(Debug, Clone)]
pub struct Mask {
    pub width: u32,
    pub height: u32,
    /// Row-major, one byte per pixel: 0 = background, non-zero = foreground.
    pub data: Vec<u8>,
}

impl Mask {
    pub fn get(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.data[(y * self.width + x) as usize] != 0
    }

    /// Nearest-neighbor upsamples the sub-region `region` (in this
    /// mask's own coordinate space) to a `target_w x target_h` mask.
    /// Used to project the working-resolution mask into a full
    /// original-resolution crop's coordinate space as a starting point
    /// for the trimap — not claimed to be pixel-accurate on its own.
    pub fn upsample_region_nearest(&self, region: Rect, target_w: u32, target_h: u32) -> Mask {
        let target_w = target_w.max(1);
        let target_h = target_h.max(1);
        let mut data = vec![0u8; (target_w * target_h) as usize];
        for ty in 0..target_h {
            let sy = region.y + (ty as u64 * region.h.max(1) as u64 / target_h as u64) as u32;
            for tx in 0..target_w {
                let sx = region.x + (tx as u64 * region.w.max(1) as u64 / target_w as u64) as u32;
                let sx = sx.min(self.width.saturating_sub(1));
                let sy = sy.min(self.height.saturating_sub(1));
                if self.get(sx, sy) {
                    data[(ty * target_w + tx) as usize] = 255;
                }
            }
        }
        Mask { width: target_w, height: target_h, data }
    }
}

impl Grid for Mask {
    type Item = bool;
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn get(&self, x: u32, y: u32) -> bool {
        Mask::get(self, x, y)
    }
}

/// One detected subject: its working-resolution mask and bounding box.
#[derive(Debug, Clone)]
pub struct Instance {
    pub mask: Mask,
    pub bbox: Rect,
    pub class_id: u32,
    pub confidence: f32,
}

/// Abstraction over the BiRefNet segmentation model, so the gate/refine
/// logic doesn't hard-depend on a specific ONNX runtime binding.
pub trait SegmentationModel {
    /// `image` is the preprocessor's working-resolution RGB output.
    ///
    /// Returns `Err` when no verdict could be produced — a failed
    /// inference, an unusable model output. That is a distinct outcome
    /// from "segmented, and the subject touches no edge", and callers
    /// route on the difference: a batch keeps going and the image falls
    /// to its no-verdict route rather than the whole run panicking.
    fn segment(&self, image: &image::RgbImage) -> Result<Vec<Instance>, String>;
}
