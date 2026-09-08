//! Imaging core: load → preprocess → classify → dispatch → export.

pub mod config;
pub mod exporter;
pub mod preprocessor;
pub mod processor;
pub mod shot_classifier;

use shot_classifier::ShotClassification;
use preprocessor::Prepared;
use std::path::Path;

/// Full per-file pipeline.
pub fn process_file(input: &Path, output: &Path) -> Result<(), String> {
    let prep = preprocessor::prepare(input)?;
    let class = classify(&prep);
    let img = processor::routes::dispatch(&prep, class.as_ref());
    exporter::export(&img, output, input)
}

/// Run the shot classifier over the working-resolution copy.
///
/// `None` means no verdict — the model is unavailable, or inference failed on this image. That is
/// not an error for the batch: `routes::dispatch` sends a no-verdict image down R3, which frames it
/// safely without claiming to know where the subject is.
#[cfg(feature = "birefnet")]
fn classify(prep: &Prepared) -> Option<ShotClassification> {
    use crate::core::shot_classifier::segmentation::SegmentationModel;
    use crate::core::shot_classifier::{classify_instance, RefineParams, RefinementInput};

    let instances = model().ok()?.segment(&prep.working).ok()?;
    let instance = instances.into_iter().next()?;

    let input = RefinementInput {
        working_image: &prep.working,
        original_image: &prep.original,
        instance: &instance,
    };
    let params = RefineParams {
        band_px: config::REFINE_BAND_PX,
        safety_px: config::REFINE_SAFETY_PX,
        context_px: config::REFINE_CONTEXT_PX,
    };
    Some(classify_instance(&input, config::GATE_MARGIN_PX, params))
}

/// Without the `birefnet` feature there is no model, so nothing is ever classified and every image
/// takes R3. A working pipeline, just an unrouted one.
#[cfg(not(feature = "birefnet"))]
fn classify(_prep: &Prepared) -> Option<ShotClassification> {
    None
}

/// The one shared model instance.
///
/// `batch::run` fans `process_file` across cores with rayon, so this must not load per image — the
/// session is ~180MB. Loaded once on first use; the outcome is cached either way.
#[cfg(feature = "birefnet")]
fn model() -> Result<&'static shot_classifier::BiRefNetModel, String> {
    use shot_classifier::{BiRefNetConfig, BiRefNetModel};
    use std::sync::OnceLock;

    static MODEL: OnceLock<Result<BiRefNetModel, String>> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            let model_path = sidecar("BIREFNET_ONNX", "birefnet_lite_512.onnx");
            let dylib_path = sidecar("ORT_DYLIB_PATH", "onnxruntime.dll");
            BiRefNetModel::load(&model_path, &dylib_path, BiRefNetConfig::default())
        })
        .as_ref()
        .map_err(|e| e.clone())
}

/// Check the classifier can actually run, before any image is touched.
///
/// A missing ONNX Runtime or model is an environment fault, not an image outcome: it should stop
/// the run loudly rather than quietly sending every image down the no-verdict route. R3 is for
/// images the classifier looked at and could not judge — not for a broken install.
///
/// Callers run this once at startup and abort on `Err`.
pub fn preflight() -> Result<(), String> {
    #[cfg(feature = "birefnet")]
    {
        model()?;
    }
    Ok(())
}

/// Resolve a file shipped alongside the executable: `$var` if set, else `<exe dir>/<name>`, else
/// the bare name relative to the working directory.
#[cfg(feature = "birefnet")]
fn sidecar(var: &str, name: &str) -> std::path::PathBuf {
    if let Some(p) = std::env::var_os(var) {
        return std::path::PathBuf::from(p);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .unwrap_or_else(|| std::path::PathBuf::from(name))
}
