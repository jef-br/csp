//! Imaging core: load → preprocess → classify → dispatch → export.

pub mod config;
pub mod exporter;
pub mod preprocessor;
pub mod processor;
pub mod runtime;
pub mod shot_classifier;

use shot_classifier::{Mask, ShotClassification};
use preprocessor::Prepared;
use std::path::Path;

/// Full per-file pipeline.
pub fn process_file(input: &Path, output: &Path, backup: &Path) -> Result<(), String> {
    let prep = preprocessor::prepare(input)?;
    let classified = classify(&prep);
    let shot = classified
        .as_ref()
        .map(|(class, mask)| processor::routes::Shot { class, mask });
    let img = processor::routes::dispatch(&prep, shot);
    let shot = classified.as_ref().map(|(class, mask)| exporter::ShotInputs {
        working: &prep.working,
        mask,
        class,
    });
    exporter::export(&img, output, input, backup, shot)
}

/// Run the shot classifier over the working-resolution copy.
///
/// `None` means no verdict — the model is unavailable, or inference failed on this image. That is
/// not an error for the batch: `routes::dispatch` sends a no-verdict image down R3, which frames it
/// safely without claiming to know where the subject is.
///
/// The mask is carried out alongside the verdict: the exporter samples the BGC/FGC debug tags from
/// it and writes it as `<stem>_segmask.jpg` when `CSP_DEBUG_TAGS` is set.
fn classify(prep: &Prepared) -> Option<(ShotClassification, Mask)> {
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
        narrow_band_px: config::REFINE_NARROW_BAND_PX,
        graze_ratio_max: config::GRAZE_RATIO_MAX,
    };
    let class = classify_instance(&input, config::GATE_MARGIN_PX, params);
    Some((class, instance.mask))
}

/// The one shared model instance.
///
/// `batch::run` fans `process_file` across cores with rayon, so this must not load per image — the
/// session is ~180MB. Loaded once on first use; the outcome is cached either way.
fn model() -> Result<&'static shot_classifier::BiRefNetModel, String> {
    use shot_classifier::{BiRefNetConfig, BiRefNetModel};
    use std::sync::OnceLock;

    static MODEL: OnceLock<Result<BiRefNetModel, String>> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            // Weights and runtime are both embedded; the runtime is unpacked to a temp
            // file first because `ort`'s loader needs a path. See `core::runtime`.
            BiRefNetModel::init_runtime(runtime::onnxruntime_dylib()?)?;
            BiRefNetModel::load(BiRefNetConfig::default())
        })
        .as_ref()
        .map_err(|e| e.clone())
}

/// Check the classifier can actually run, before any image is touched.
///
/// The weights are embedded now, so the only file this can fail to find is `onnxruntime.dll`.
/// That, or a session that will not build, is an environment fault rather than an image outcome:
/// it should stop the run loudly rather than quietly sending every image down the no-verdict
/// route. R3 is for images the classifier looked at and could not judge — not for a broken build.
///
/// Callers run this once at startup and abort on `Err`. It doubles as the point where the ~180MB
/// session is paid for, off the batch's critical path.
pub fn preflight() -> Result<(), String> {
    model()?;
    Ok(())
}
