//! Imaging core: load → preprocess → classify → dispatch → export.

pub mod config;
pub mod exporter;
pub mod preprocessor;
pub mod processor;
pub mod runtime;
pub mod shot_classifier;
pub mod trace;

use shot_classifier::{Mask, ShotClassification};
use preprocessor::Prepared;
use std::path::Path;

/// Full per-file pipeline.
pub fn process_file(input: &Path, output: &Path, backup: &Path) -> Result<(), String> {
    let _trace = trace::begin(output);
    let prep = preprocessor::prepare(input)?;
    let classified = classify(&prep);
    let shot = classified
        .as_ref()
        .map(|(class, mask)| processor::routes::Shot { class, mask });
    let route = processor::routes::Route::select(shot.as_ref().map(|s| s.class));
    trace_stage_90(&prep, shot.as_ref(), route);
    let img = processor::routes::dispatch(&prep, shot);
    let shot = classified.as_ref().map(|(class, mask)| exporter::ShotInputs {
        working: &prep.working,
        mask,
        class,
    });
    exporter::export(&img, output, input, backup, shot)
}

/// Run the shot classifier: the model sees the optimiser's crop, the edge checks the true-colour image.
///
/// `None` means no verdict — the model is unavailable, or inference failed on this image. That is
/// not an error for the batch: `routes::dispatch` sends a no-verdict image down R3, which frames it
/// safely without claiming to know where the subject is.
///
/// The mask is carried out alongside the verdict: the exporter samples the BGC/FGC debug tags from
/// it and writes it as `<stem>_segmask.jpg` when `CSP_DEBUG_TAGS` is set.
fn classify(prep: &Prepared) -> Option<(ShotClassification, Mask)> {
    use crate::core::shot_classifier::{
        classify_instance, CroppedModel, RefineParams, RefinementInput, SegmentationModel,
    };

    // The model is shown only the optimiser's crop; its mask is put back into the full working frame.
    let model = CroppedModel::new(model().ok()?, prep.working.dimensions(), prep.crop);
    let instances = model.segment(&prep.model_input).ok()?;
    let instance = instances.into_iter().next()?;
    trace::note(|| format!("mask covers {:.1}% of the frame", trace::coverage_pct(&instance.mask)));

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

/// Stage 90: "what we know" about the image, from which the route is picked. The same drawing
/// code runs every image; a value that is absent (no bbox, no plan, no overhang) is simply not
/// drawn, and not written to the log. The header names the chosen route so the picture and the
/// routing decision are read together, not from two separate places.
fn trace_stage_90(
    prep: &Prepared,
    shot: Option<&processor::routes::Shot<'_>>,
    route: processor::routes::Route,
) {
    use processor::routes::center_and_stretch::plan::{Pins, SquarePlan};
    use processor::routes::Route;
    use shot_classifier::shotcode::{eix_bits, NO_VERDICT_EIX};

    if !trace::active() {
        return;
    }

    let subject = shot.and_then(|s| s.subject_bbox_in(prep));
    let plan = shot.and_then(|s| {
        subject.and_then(|subj| {
            SquarePlan::new(subj, config::R1_MARGIN, Pins::from_edges(&s.class.touches_edges))
        })
    });
    let gated: Vec<_> = shot
        .map(|s| s.class.refinements.iter().map(|r| r.edge).collect())
        .unwrap_or_default();
    let touching: Vec<_> = shot.map(|s| s.class.touches_edges.clone()).unwrap_or_default();

    let route_label = match route {
        Route::CenterAndStretch => "[R1 - CenterAndStretch]",
        Route::CropSquare => "[R2 - CropSquare]",
        Route::Fallback => "[R3 - Fallback]",
    };

    let plan_tuple = plan.as_ref().map(|p| (p.x, p.y, p.side));
    let header = format!(
        "{route_label} - bbox (cyan), final canvas (yellow), grey (invented area); red (edge touches) orange (edge gated but cleared)"
    );
    trace::image(trace::stage::R1_PLAN, &header, || {
        trace::plan_view(&prep.original, subject, plan_tuple, &gated, &touching)
    });

    // Line 2: bbox and R1's planned canvas, each shown only if we have it.
    let mut l2 = String::new();
    if let Some(b) = subject {
        l2.push_str(&format!("bbox [x:{}, y:{}, w:{}, h:{}]", b.x, b.y, b.w, b.h));
    }
    if let Some(p) = &plan {
        if !l2.is_empty() {
            l2.push_str(" → ");
        }
        l2.push_str(&format!(
            "canvas: [x:{}, y:{}, w:{}, h:{}]",
            p.x, p.y, p.side, p.side
        ));
    }
    if !l2.is_empty() {
        trace::note(move || l2);
    }

    // Line 3: shot code. EIX is always known; BGC/FGC only when a verdict and a mask were produced.
    let (eix, colors) = match shot {
        Some(s) => (
            eix_bits(&s.class.touches_edges),
            Some(exporter::colors::sample(&prep.working, s.mask)),
        ),
        None => (NO_VERDICT_EIX.to_owned(), None),
    };
    let mut l3 = format!("EIX={eix}");
    if let Some(c) = colors.as_ref().and_then(|c| c.bgc.as_ref()) {
        l3.push_str(&format!("  BGC={c}"));
    }
    if let Some(c) = colors.as_ref().and_then(|c| c.fgc.as_ref()) {
        l3.push_str(&format!("  FGC={c}"));
    }
    trace::note(move || l3);

    // Line 4: overhang (only when there is a plan), mask flag and touched edges (only with a verdict).
    let mut l4 = String::new();
    if let Some(p) = &plan {
        let o = p.overhang(prep.original.width(), prep.original.height());
        l4.push_str(&format!(
            "overhang [T:{}, B:{}, L:{}, R:{}]",
            o.top, o.bottom, o.left, o.right
        ));
    }
    if let Some(s) = shot {
        if !l4.is_empty() {
            l4.push_str("  ");
        }
        l4.push_str(&format!(
            "mask_too_small = {}, touches {:?}",
            s.class.mask_too_small, s.class.touches_edges
        ));
    }
    if !l4.is_empty() {
        trace::note(move || l4);
    }
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
