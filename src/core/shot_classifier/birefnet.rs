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

use super::geometry::Rect;
use super::segmentation::{Instance, Mask, SegmentationModel};
use image::{imageops::FilterType, RgbImage};
use ort::session::Session;
use ort::value::Tensor;

/// The BiRefNet weights, compiled into the executable.
///
/// CSP ships as one file, so the model is part of the binary rather than a `.onnx` beside it. The
/// path is fixed and the file is gitignored: a missing model is a *build* failure naming the file,
/// which is the right time to find out — the alternative is an exe that builds fine and then can
/// never classify anything.
///
/// The bytes live in the exe's mapped image, so they cost address space rather than committed
/// memory until ONNX Runtime copies them into its own session at load.
static MODEL_BYTES: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/birefnet_lite_512.onnx"));

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
    /// Points `ort` at the ONNX Runtime shared library, once per process.
    ///
    /// The path is checked before `ort` is touched: `ort::init_from(..).commit()` *panics* rather
    /// than returning `Err` when the dylib is missing, so without this the operator gets a stack
    /// trace out of a dylib loader instead of a sentence naming the file.
    ///
    /// This exists only because the runtime cannot yet be linked in — see `Cargo.toml`'s `ort`
    /// entry. The path handed in is `core::runtime`'s temp copy of the embedded `onnxruntime.dll`,
    /// not a file beside the exe. When a static ORT >= 1.22 is available, this function and
    /// `core::runtime` both go.
    pub fn init_runtime(onnxruntime_dylib_path: impl AsRef<std::path::Path>) -> Result<(), String> {
        let dylib = onnxruntime_dylib_path.as_ref();
        if !dylib.is_file() {
            return Err(format!("ONNX Runtime not found at {}", dylib.display()));
        }
        ort::init_from(dylib.to_string_lossy().to_string())
            .commit()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Loads the embedded model.
    ///
    /// There is nothing to find on disk: the weights are [`MODEL_BYTES`], compiled in. Failure
    /// here means the runtime cannot build a session from that graph, which is fatal for the run —
    /// see `core::preflight`. Call [`Self::init_runtime`] first.
    pub fn load(config: BiRefNetConfig) -> Result<Self, String> {
        Self::build(config, |b| b.commit_from_memory(MODEL_BYTES)).map_err(|e| e.to_string())
    }

    /// Loads a model from disk instead of the embedded one.
    ///
    /// For the `examples/` dev harness, which exists to run *other* exports — a re-quantized model,
    /// a different side — against the same session settings. The shipped binary never calls this.
    pub fn load_from_file(
        model_path: impl AsRef<std::path::Path>,
        config: BiRefNetConfig,
    ) -> Result<Self, String> {
        let model = model_path.as_ref();
        if !model.is_file() {
            return Err(format!("BiRefNet model not found at {}", model.display()));
        }
        Self::build(config, |b| b.commit_from_file(model)).map_err(|e| e.to_string())
    }

    /// The session settings both loaders share; only the source of the graph differs.
    fn build(
        config: BiRefNetConfig,
        commit: impl FnOnce(ort::session::builder::SessionBuilder) -> ort::Result<Session>,
    ) -> ort::Result<Self> {
        // BiRefNet's activations are heavy (a swin backbone + deformable
        // decoder). Keep peak memory bounded: no arena growth, single
        // intra-op thread, and cap graph optimization at Level1 — the
        // default (Level3) transient-buffers the 512/1024 graph hard
        // enough to OOM a memory-constrained host during session init,
        // for no measurable speedup on a one-shot forward pass.
        let builder = Session::builder()?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level1)?
            .with_memory_pattern(false)?
            .with_intra_threads(1)?;
        let session = commit(builder)?;
        Ok(BiRefNetModel { session, config })
    }

    /// Reads the model's declared NCHW input side. Dynamic axes come back as `-1`, which would
    /// otherwise become a ~4-billion-pixel allocation, so they are rejected here with a message
    /// naming the fix rather than dying in the allocator.
    fn input_size(&self) -> Result<(u32, u32), String> {
        let dims = match &self.session.inputs[0].input_type {
            ort::value::ValueType::Tensor { dimensions, .. } => dimensions,
            other => return Err(format!("unexpected BiRefNet input type: {other:?}")),
        };
        if dims.len() != 4 {
            return Err(format!("expected a 4-D NCHW input, got {} dims", dims.len()));
        }
        if dims[2] <= 0 || dims[3] <= 0 {
            return Err(format!(
                "model declares a dynamic input size ({}x{}); re-export at a fixed side",
                dims[3], dims[2]
            ));
        }
        Ok((dims[3] as u32, dims[2] as u32)) // NCHW: width, height
    }
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

impl SegmentationModel for BiRefNetModel {
    fn segment(&self, image: &RgbImage) -> Result<Vec<Instance>, String> {
        let (in_w, in_h) = self.input_size()?;

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
            .map_err(|e| format!("build input tensor: {e}"))?;

        let input_values =
            ort::inputs!["input_image" => input].map_err(|e| format!("build session inputs: {e}"))?;
        let outputs = self
            .session
            .run(input_values)
            .map_err(|e| format!("BiRefNet inference: {e}"))?;

        let logits = outputs["output_image"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("output_image was not an f32 tensor: {e}"))?;
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
        Ok(vec![Instance { mask, bbox, class_id: 0, confidence: 1.0 }])
    }
}
