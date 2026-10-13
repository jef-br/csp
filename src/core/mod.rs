//! Imaging core: load → preprocess → classify → dispatch → export.

pub mod config;
pub mod exporter;
pub mod preprocessor;
pub mod processor;
pub mod runtime;
pub mod shot_classifier;
pub mod trace;

use shot_classifier::geometry::Rect;
use shot_classifier::{shotcode, Instance, Mask, SegmentationModel, ShotClassification};
use preprocessor::Prepared;
use std::path::Path;

/// Full per-file pipeline.
pub fn process_file(input: &Path, output: &Path, backup: &Path) -> Result<(), String> {
    let _trace = trace::begin(input);
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

/// Run the shot classifier: the model sees the optimised copy, the edge checks the true-colour one.
///
/// `None` means no verdict — the model is unavailable, or inference failed on this image. That is
/// not an error for the batch: `routes::dispatch` sends a no-verdict image down R3, which frames it
/// safely without claiming to know where the subject is.
///
/// The mask is carried out alongside the verdict: the exporter samples the BGC/FGC debug tags from
/// it and writes it as `<stem>_segmask.jpg` when `CSP_DEBUG_TAGS` is set.
fn classify(prep: &Prepared) -> Option<(ShotClassification, Mask)> {
    use crate::core::shot_classifier::{classify_instance, RefineParams, RefinementInput};

    let instance = segment_trimmed(model().ok()?, prep)?;
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

/// Run the model on the product, not on the empty backdrop around it, and return a full-frame mask.
///
/// The optimised image is trimmed (`preprocessor::trim`) and the model runs on the crop; its mask is
/// pasted back into the full frame, so everything after this sees the same kind of mask as before.
/// Every way the trim can go wrong ends in the plain pass on the whole frame, which is what ran before
/// the trim existed: the trimmed pass came back empty, its mask ran into a crop edge that is not the
/// image edge even after the crop was widened (the trim cut into the product), or the model failed.
fn segment_trimmed(model: &impl SegmentationModel, prep: &Prepared) -> Option<Instance> {
    use crate::core::preprocessor::trim;

    let full = prep.model_input();
    let (w, h) = full.dimensions();
    let plain = || model.segment(full).ok().and_then(|found| found.into_iter().next());

    let Some(mut crop) = trim::plan(full) else {
        trace::note(|| "trim: nothing to trim, the model sees the whole frame".into());
        return plain();
    };
    trace::image(trace::stage::TRIM, "backdrop trim: the crop the model sees (red box) on the optimised image", || {
        let mut img = full.clone();
        trace::outline(&mut img, crop.x as i64, crop.y as i64, crop.w, crop.h, [255, 0, 0], 3);
        img
    });

    for round in 0..=config::TRIM_GROW_ROUNDS {
        let Some(found) = model.segment(&trim::crop_view(full, crop)).ok().and_then(|f| f.into_iter().next()) else {
            return plain();
        };
        let cut = trim::cut_sides(&found.mask, (w, h), crop);
        if !cut.contains(&true) {
            let mask = trim::paste_back(&found.mask, crop, w, h);
            if shotcode::mask_too_small(&prep.working, &mask) {
                trace::note(|| format!("trim: crop {crop:?} found no subject, falling back to the whole frame"));
                return plain();
            }
            trace::note(|| format!("trim: model ran on crop {crop:?} (grown {round}x)"));
            return Some(Instance { mask, bbox: Rect::from_bounds(0, 0, w - 1, h - 1), ..found });
        }
        if round == config::TRIM_GROW_ROUNDS {
            trace::note(|| format!("trim: the mask still runs into a cut edge {cut:?} of {crop:?}, falling back to the whole frame"));
            return plain();
        }
        trace::note(|| format!("trim: the mask runs into the cut edge {cut:?} of {crop:?}, widening the crop"));
        crop = trim::grow(crop, cut, (w, h));
    }
    plain()
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};
    use std::cell::RefCell;

    const BACKDROP: Rgb<u8> = Rgb([245, 245, 245]);

    /// A stand-in for BiRefNet: marks every pixel that differs from the backdrop, and records the size
    /// of every view it is shown. `blind_below` makes it return nothing for views smaller than that.
    struct Stub {
        views: RefCell<Vec<(u32, u32)>>,
        blind_below: u32,
    }

    impl Stub {
        fn new(blind_below: u32) -> Self {
            Stub { views: RefCell::new(Vec::new()), blind_below }
        }
    }

    impl SegmentationModel for Stub {
        fn segment(&self, image: &RgbImage) -> Result<Vec<Instance>, String> {
            let (w, h) = image.dimensions();
            self.views.borrow_mut().push((w, h));
            let data: Vec<u8> = image
                .pixels()
                .map(|p| {
                    let differs = (0..3).any(|c| (i32::from(p[c]) - i32::from(BACKDROP[c])).abs() > 3);
                    if differs && w.max(h) >= self.blind_below { 255 } else { 0 }
                })
                .collect();
            let mask = Mask { width: w, height: h, data };
            Ok(vec![Instance { mask, bbox: Rect::from_bounds(0, 0, w - 1, h - 1), class_id: 0, confidence: 1.0 }])
        }
    }

    fn prepared(img: &RgbImage) -> Prepared {
        Prepared { original: img.clone(), working: img.clone(), optimized: None }
    }

    fn paint(img: &mut RgbImage, rect: (u32, u32, u32, u32), colour: Rgb<u8>) {
        for y in rect.1..rect.1 + rect.3 {
            for x in rect.0..rect.0 + rect.2 {
                img.put_pixel(x, y, colour);
            }
        }
    }

    fn lit(mask: &Mask) -> usize {
        mask.data.iter().filter(|&&v| v > 0).count()
    }

    /// The model is shown the crop, not the frame, and the mask it returns lands back in the frame.
    #[test]
    fn the_model_sees_the_crop_and_the_mask_lands_in_the_full_frame() {
        let mut img = RgbImage::from_pixel(400, 600, BACKDROP);
        paint(&mut img, (125, 225, 150, 150), Rgb([30, 40, 90]));
        let stub = Stub::new(0);

        let found = segment_trimmed(&stub, &prepared(&img)).expect("a verdict");

        let views = stub.views.borrow();
        assert_eq!(views.len(), 1, "one pass is enough: {views:?}");
        assert!(views[0].0 < 400 && views[0].1 < 600, "the model must see a crop: {views:?}");
        assert_eq!((found.mask.width, found.mask.height), (400, 600));
        assert_eq!(lit(&found.mask), 150 * 150);
        assert!(found.mask.get(125, 225) && found.mask.get(274, 374) && !found.mask.get(124, 225));
    }

    /// If the crop yields nothing, the whole frame is tried, as before the trim existed.
    #[test]
    fn an_empty_trimmed_pass_falls_back_to_the_whole_frame() {
        let mut img = RgbImage::from_pixel(400, 600, BACKDROP);
        paint(&mut img, (125, 225, 150, 150), Rgb([30, 40, 90]));
        let stub = Stub::new(600); // blind to anything but the full 600px-tall frame

        let found = segment_trimmed(&stub, &prepared(&img)).expect("a verdict");

        assert_eq!(stub.views.borrow().len(), 2, "crop, then the whole frame");
        assert_eq!(lit(&found.mask), 150 * 150);
    }

    /// A crop that cuts into the product is widened until the mask no longer runs into the cut.
    #[test]
    fn a_crop_that_cuts_the_product_is_widened() {
        // The lower half of the product is nearly the backdrop colour, so the trim treats it as backdrop.
        let mut img = RgbImage::from_pixel(400, 600, BACKDROP);
        paint(&mut img, (125, 225, 150, 75), Rgb([30, 40, 90]));
        paint(&mut img, (125, 300, 150, 75), Rgb([236, 236, 236]));
        let stub = Stub::new(0);

        let found = segment_trimmed(&stub, &prepared(&img)).expect("a verdict");

        assert!(stub.views.borrow().len() > 1, "the first crop is cut, so there must be another pass");
        assert_eq!(lit(&found.mask), 150 * 150, "the whole product, not the part the trim kept");
    }

    /// With no backdrop to trim there is one plain pass on the whole frame.
    #[test]
    fn nothing_to_trim_means_one_plain_pass() {
        let img = RgbImage::from_pixel(200, 300, BACKDROP);
        let stub = Stub::new(0);
        let found = segment_trimmed(&stub, &prepared(&img)).expect("a verdict");
        assert_eq!(*stub.views.borrow(), vec![(200, 300)]);
        assert_eq!(lit(&found.mask), 0);
    }
}
