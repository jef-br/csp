//! Real BiRefNet (dichotomous image segmentation) inference, via ONNX
//! Runtime. This is a pure foreground/background model: no boxes, no
//! classes, no NMS — one sigmoid mask at full model resolution answering
//! "subject or not" per pixel.
//!
//! Verified against exported `BiRefNet_lite` ONNX models:
//! - Input `input_image`: `[1, 3, S, S]` float32, RGB, resized (not
//!   letterboxed — BiRefNet's own preprocessing is a plain resize) and
//!   ImageNet-normalized (mean `[0.485, 0.456, 0.406]`, std
//!   `[0.229, 0.224, 0.225]`). The side `S` (512 and 1024 are the common
//!   full-size exports; a smaller `S` trades edge accuracy for far less
//!   memory) is read from the model's own input metadata at load time,
//!   not hardcoded.
//! - Output `output_image`: `[1, 1, S, S]` float32 — raw logits; apply
//!   sigmoid, threshold, resize back to the source image size.
//!
//! Note: BiRefNet's decoder uses deformable convolutions. Export with
//! ONNX opset >= 19 so they become a single native `DeformConv` op
//! (needs ONNX Runtime >= 1.22) rather than a `GatherND` decomposition
//! that allocates enormous intermediates.
//!
//! To slot into the existing gate/refine pipeline unchanged, the whole-
//! image mask is wrapped as a single [`Instance`] whose `bbox` is the
//! full image (there's no per-object box to report) and whose
//! `class_id`/`confidence` are unused placeholders — [`SegmentationModel`]
//! only cares about the mask from here on.

use crate::geometry::Rect;
use crate::segmentation::{Instance, Mask, SegmentationModel};
use image::{imageops::FilterType, RgbImage};
use ort::session::Session;
use ort::value::Tensor;

const IMAGENET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const IMAGENET_STD: [f32; 3] = [0.229, 0.224, 0.225];

#[derive(Debug, Clone)]
pub struct BiRefNetConfig {
    /// Threshold applied to the sigmoid output to binarize the mask.
    pub mask_threshold: f32,
}

impl Default for BiRefNetConfig {
    fn default() -> Self {
        BiRefNetConfig { mask_threshold: 0.5 }
    }
}

pub struct BiRefNetModel {
    session: Session,
    config: BiRefNetConfig,
}

impl BiRefNetModel {
    pub fn load(
        model_path: impl AsRef<std::path::Path>,
        onnxruntime_dylib_path: impl AsRef<std::path::Path>,
        config: BiRefNetConfig,
    ) -> ort::Result<Self> {
        ort::init_from(onnxruntime_dylib_path.as_ref().to_string_lossy().to_string()).commit()?;
        // BiRefNet's activations are heavy (a swin backbone + deformable
        // decoder). Keep peak memory bounded: no arena growth, single
        // intra-op thread, and cap graph optimization at Level1 — the
        // default (Level3) transient-buffers the 512/1024 graph hard
        // enough to OOM a memory-constrained host during session init,
        // for no measurable speedup on a one-shot forward pass.
        let session = Session::builder()?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level1)?
            .with_memory_pattern(false)?
            .with_intra_threads(1)?
            .commit_from_file(model_path)?;
        Ok(BiRefNetModel { session, config })
    }

    fn input_size(&self) -> (u32, u32) {
        let dims = match &self.session.inputs[0].input_type {
            ort::value::ValueType::Tensor { dimensions, .. } => dimensions,
            _ => panic!("unexpected BiRefNet input type — expected a tensor"),
        };
        (dims[3] as u32, dims[2] as u32) // NCHW: width, height
    }
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

impl SegmentationModel for BiRefNetModel {
    fn segment(&self, image: &RgbImage) -> Vec<Instance> {
        let (in_w, in_h) = self.input_size();

        // Plain resize (no letterbox/padding) per BiRefNet's own
        // preprocessing, then ImageNet normalization.
        let resized = image::imageops::resize(image, in_w, in_h, FilterType::Triangle);
        let mut chw = vec![0f32; 3 * (in_h * in_w) as usize];
        let plane = (in_h * in_w) as usize;
        for (i, px) in resized.pixels().enumerate() {
            for c in 0..3 {
                chw[c * plane + i] = (px[c] as f32 / 255.0 - IMAGENET_MEAN[c]) / IMAGENET_STD[c];
            }
        }
        let input = Tensor::from_array(([1usize, 3, in_h as usize, in_w as usize], chw))
            .expect("building the input tensor from a correctly-sized buffer cannot fail");

        let input_values = ort::inputs!["input_image" => input]
            .expect("building the session inputs from a single named tensor cannot fail");
        let outputs = self
            .session
            .run(input_values)
            .expect("BiRefNet inference failed — see the ort error for details");

        let logits = outputs["output_image"]
            .try_extract_tensor::<f32>()
            .expect("output_image was not the expected f32 tensor");
        let logit_data: Vec<f32> = logits.iter().copied().collect();

        // Resize the model-resolution mask back to the working image's
        // own dimensions with nearest-neighbor sampling on the sigmoid +
        // threshold result (matches how the mask will be consumed
        // downstream — a binary mask, not a soft alpha).
        let (out_w, out_h) = (image.width(), image.height());
        let mut data = vec![0u8; (out_w * out_h) as usize];
        for oy in 0..out_h {
            for ox in 0..out_w {
                let mx = ((ox as f32 + 0.5) / out_w as f32 * in_w as f32) as usize;
                let my = ((oy as f32 + 0.5) / out_h as f32 * in_h as f32) as usize;
                let mx = mx.min(in_w as usize - 1);
                let my = my.min(in_h as usize - 1);
                let logit = logit_data[my * in_w as usize + mx];
                if sigmoid(logit) > self.config.mask_threshold {
                    data[(oy * out_w + ox) as usize] = 255;
                }
            }
        }
        let mask = Mask { width: out_w, height: out_h, data };

        // No box/class from this model — report the full image as the
        // bbox so the gate/refine pipeline (which only reads the mask
        // near the border) works unmodified.
        let bbox = Rect::from_bounds(0, 0, out_w.saturating_sub(1), out_h.saturating_sub(1));
        vec![Instance { mask, bbox, class_id: 0, confidence: 1.0 }]
    }
}
