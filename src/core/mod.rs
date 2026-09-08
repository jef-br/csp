//! Imaging core: load → preprocess → classify → dispatch → export.

pub mod config;
pub mod exporter;
pub mod preprocessor;
pub mod processor;

use csp_shot_classifier::ShotClassification;
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
    use csp_shot_classifier::segmentation::SegmentationModel;
    use csp_shot_classifier::{classify_instance, RefineParams, RefinementInput};

    let instances = model()?.segment(&prep.working).ok()?;
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
/// session is tens to hundreds of megabytes. Loaded once on first use; a load failure is reported
/// once and then every image falls to R3 rather than each one re-reporting it.
#[cfg(feature = "birefnet")]
fn model() -> Option<&'static csp_shot_classifier::BiRefNetModel> {
    use csp_shot_classifier::{BiRefNetConfig, BiRefNetModel};
    use std::sync::OnceLock;

    static MODEL: OnceLock<Option<BiRefNetModel>> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            let model_path = sidecar("BIREFNET_ONNX", "birefnet_lite_512.onnx");
            let dylib_path = sidecar("ORT_DYLIB_PATH", "onnxruntime.dll");
            match BiRefNetModel::load(&model_path, &dylib_path, BiRefNetConfig::default()) {
                Ok(m) => Some(m),
                Err(e) => {
                    eprintln!(
                        "shot classifier unavailable ({e}); every image will take the fallback \
                         route.\n  model:   {}\n  runtime: {}",
                        model_path.display(),
                        dylib_path.display()
                    );
                    None
                }
            }
        })
        .as_ref()
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
