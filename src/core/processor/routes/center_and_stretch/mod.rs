//! R1 — the subject touches no edge (EIX `0000`).
//!
//! The subject stands clear of every border, so there are real background pixels on all sides.
//! Build a square of `max(bbox_w, bbox_h) * (1 + margin)` centred on the subject ([`plan`]), then
//! fill whatever it reaches past the image by stretching the background bands outward
//! ([`compose`]). A side that lands inside the image crops instead — same code, no branch.
//!
//! The subject region is the segmentation mask's own bounding box. **No saliency here**: the
//! saliency-weighted centre of gravity belongs to [`super::crop_square`], where a fully-bled
//! subject leaves no background to anchor against. R1 always has real background on all four
//! sides, so the mask's bbox is enough to place the square.
//!
//! ## Deliberately not validated
//!
//! The background band is taken as trustworthy from `safety_px` outside the mask all the way to
//! the border. Two validations of it were designed and then *deferred*, not forgotten: trimming a
//! flat, artificial background run off the outer end, and advancing the inner end past a cast
//! shadow the mask does not cover. Plain resampling of the whole band proved good enough on the
//! test set, so neither is worth its complexity until an image argues otherwise. The argument will
//! look like a smeared shadow, or a stretched band of the wrong colour along one side.

pub mod compose;
pub mod plan;

use super::super::super::config;
use super::super::super::preprocessor::Prepared;
use super::{fallback, Shot};
use image::RgbImage;
use plan::SquarePlan;

/// How far outside the mask the background is trusted, in full-resolution pixels.
///
/// Two errors stack: the mask's own boundary is only good to a couple of *working*-resolution
/// pixels, which is that many times the upscale factor once it lands here, and the full-resolution
/// boundary is itself soft by a pixel or two. Doubled for headroom.
fn safety_px(prep: &Prepared) -> u32 {
    let scale = (prep.original.width() as f32 / prep.working.width().max(1) as f32)
        .max(prep.original.height() as f32 / prep.working.height().max(1) as f32)
        .max(1.0);
    (2.0 * (config::R1_SAFETY_MASK_PX * scale + config::R1_SAFETY_EDGE_PX)).round() as u32
}

pub fn apply(prep: &Prepared, shot: Shot<'_>) -> RgbImage {
    // An empty mask means the classifier marked nothing: there is no subject to centre on, so the
    // safe framing of R3 is a better answer than a square around an arbitrary point.
    let Some(subject) = shot.subject_bbox_in(prep) else {
        return fallback::apply(prep);
    };
    let plan = SquarePlan::new(subject, config::R1_MARGIN);
    compose::compose(&prep.original, &plan, subject, safety_px(prep))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::shot_classifier::{geometry::Rect, Mask, ShotClassification};
    use image::Rgb;

    /// A `w x h` image: uniform background, with `subject` painted a different colour.
    fn scene(w: u32, h: u32, subject: Rect, bg: [u8; 3], fg: [u8; 3]) -> (RgbImage, Mask) {
        let mut img = RgbImage::from_pixel(w, h, Rgb(bg));
        let mut mask = Mask { width: w, height: h, data: vec![0u8; (w * h) as usize] };
        for y in subject.y..subject.y + subject.h {
            for x in subject.x..subject.x + subject.w {
                img.put_pixel(x, y, Rgb(fg));
                mask.data[(y * w + x) as usize] = 255;
            }
        }
        (img, mask)
    }

    fn run(img: RgbImage, mask: &Mask) -> RgbImage {
        let prep = Prepared { original: img.clone(), working: img };
        let class = ShotClassification::default();
        apply(&prep, Shot { class: &class, mask })
    }

    #[test]
    fn output_is_square_and_the_subject_is_centred() {
        // Subject sits well off-centre, low and left; R1 must put it in the middle.
        let (img, mask) = scene(400, 300, Rect { x: 20, y: 180, w: 60, h: 90 }, [200, 200, 200], [10, 10, 10]);
        let out = run(img, &mask);

        assert_eq!(out.width(), out.height());
        assert_eq!(out.width(), 94, "90 * 1.042 = 93.8");
        // Centre of the output is subject, and the corners are background.
        assert_eq!(*out.get_pixel(47, 47), Rgb([10, 10, 10]));
        assert_eq!(*out.get_pixel(1, 1), Rgb([200, 200, 200]));
        assert_eq!(*out.get_pixel(92, 92), Rgb([200, 200, 200]));
    }

    #[test]
    fn a_square_that_fits_is_a_pure_crop() {
        // Every side lands on real pixels, so nothing is resampled: a uniform background stays
        // exactly its own value, and the subject band is copied untouched.
        let (img, mask) = scene(400, 400, Rect { x: 150, y: 150, w: 100, h: 100 }, [240, 240, 240], [0, 0, 0]);
        let out = run(img, &mask);
        assert_eq!(out.dimensions(), (104, 104));
        assert_eq!(*out.get_pixel(52, 52), Rgb([0, 0, 0]));
        assert_eq!(*out.get_pixel(0, 0), Rgb([240, 240, 240]));
    }

    #[test]
    fn a_subject_wider_than_the_frame_is_filled_on_the_narrow_axis() {
        // 200x100 frame, subject nearly filling it: the square is taller than the image, so the
        // top and bottom bands stretch. Corners come out background — the proof that the
        // two-pass order fills them, since they are outside the image on both axes.
        let (img, mask) = scene(200, 100, Rect { x: 40, y: 20, w: 120, h: 60 }, [180, 190, 200], [30, 40, 50]);
        let out = run(img, &mask);
        assert_eq!(out.dimensions(), (125, 125));
        assert_eq!(*out.get_pixel(2, 2), Rgb([180, 190, 200]), "top-left corner");
        assert_eq!(*out.get_pixel(122, 122), Rgb([180, 190, 200]), "bottom-right corner");
        assert_eq!(*out.get_pixel(62, 62), Rgb([30, 40, 50]), "centre is still subject");
    }

    #[test]
    fn a_gradient_background_keeps_its_gradient_when_stretched() {
        // The case a flat fill loses. The ramp runs left-to-right and the stretch is horizontal,
        // so the left band has to carry its gradient across 44 invented columns rather than
        // settle at one averaged value.
        let mut img = RgbImage::new(120, 400);
        for y in 0..400 {
            for x in 0..120 {
                let v = (x * 255 / 119) as u8;
                img.put_pixel(x, y, Rgb([v, v, v]));
            }
        }
        let mut mask = Mask { width: 120, height: 400, data: vec![0u8; 120 * 400] };
        for y in 100..300 {
            for x in 20..100 {
                img.put_pixel(x, y, Rgb([255, 0, 0]));
                mask.data[(y * 120 + x) as usize] = 255;
            }
        }
        let out = run(img, &mask);
        assert_eq!(out.dimensions(), (208, 208), "200 * 1.042 = 208.4");

        // Left band: source columns 0..12 (subject at 20, safety 8) stretched over output
        // columns 0..56. Both ends pinned, and no reversal in between.
        let row = out.height() / 2;
        let band: Vec<u8> = (0..56).map(|x| out.get_pixel(x, row)[0]).collect();
        assert_eq!(band[0], 0, "outer end is pinned to the image's own border pixel");
        assert_eq!(band[55], 23, "inner end meets column 11 of the source, value 23");
        assert!(
            band.windows(2).all(|w| w[0] <= w[1]),
            "a stretched gradient must stay monotonic, got {band:?}"
        );
    }

    #[test]
    fn an_empty_mask_falls_back_instead_of_centring_on_nothing() {
        let img = RgbImage::from_pixel(200, 100, Rgb([1, 2, 3]));
        let mask = Mask { width: 200, height: 100, data: vec![0u8; 200 * 100] };
        let out = run(img, &mask);
        // R3's answer: whole image on a square canvas at the longest side.
        assert_eq!(out.dimensions(), (200, 200));
    }
}
