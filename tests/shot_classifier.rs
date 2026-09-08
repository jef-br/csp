use csp::core::shot_classifier::segmentation::{Instance, Mask};
use csp::core::shot_classifier::{classify_instance, geometry::Rect, refine::RefineParams, refine::RefinementInput, Edge};
use image::{Rgb, RgbImage};

/// Builds a working-resolution image (mid-gray background) and its
/// matching mask for a `subject` rectangle painted white, at `scale`x
/// the "original" resolution (original == working here, scale = 1, to
/// keep the synthetic case simple and deterministic).
fn build_case(width: u32, height: u32, subject: Rect) -> (RgbImage, Mask) {
    let mut image = RgbImage::from_pixel(width, height, Rgb([120, 120, 120]));
    let mut mask = Mask { width, height, data: vec![0u8; (width * height) as usize] };
    for y in subject.y..(subject.y + subject.h).min(height) {
        for x in subject.x..(subject.x + subject.w).min(width) {
            image.put_pixel(x, y, Rgb([230, 230, 230]));
            mask.data[(y * width + x) as usize] = 255;
        }
    }
    (image, mask)
}

fn instance(mask: Mask, bbox: Rect) -> Instance {
    Instance { mask, bbox, class_id: 0, confidence: 0.9 }
}

#[test]
fn subject_touching_left_edge_is_detected() {
    let (image, mask) = build_case(200, 200, Rect { x: 0, y: 60, w: 80, h: 80 });
    let inst = instance(mask, Rect { x: 0, y: 60, w: 80, h: 80 });
    let input = RefinementInput { working_image: &image, original_image: &image, instance: &inst };

    let result = classify_instance(&input, 20, RefineParams::default());

    assert!(result.touches_edges.contains(&Edge::Left), "expected Left edge to be flagged as touching");
    assert!(!result.touches_edges.contains(&Edge::Right));
    assert!(!result.touches_edges.contains(&Edge::Top));
    assert!(!result.touches_edges.contains(&Edge::Bottom));
}

#[test]
fn subject_away_from_all_edges_is_not_touching() {
    let (image, mask) = build_case(200, 200, Rect { x: 60, y: 60, w: 80, h: 80 });
    let inst = instance(mask, Rect { x: 60, y: 60, w: 80, h: 80 });
    let input = RefinementInput { working_image: &image, original_image: &image, instance: &inst };

    let result = classify_instance(&input, 20, RefineParams::default());

    assert!(result.touches_edges.is_empty(), "expected no edges flagged as touching, got {:?}", result.touches_edges);
}

#[test]
fn subject_touching_two_adjacent_edges() {
    let (image, mask) = build_case(200, 200, Rect { x: 0, y: 0, w: 80, h: 80 });
    let inst = instance(mask, Rect { x: 0, y: 0, w: 80, h: 80 });
    let input = RefinementInput { working_image: &image, original_image: &image, instance: &inst };

    let result = classify_instance(&input, 20, RefineParams::default());

    assert!(result.touches_edges.contains(&Edge::Top));
    assert!(result.touches_edges.contains(&Edge::Left));
    assert!(!result.touches_edges.contains(&Edge::Bottom));
    assert!(!result.touches_edges.contains(&Edge::Right));
}

/// Builds a case where the mask and the *image* disagree at the border: a subject column whose
/// pixels run all the way to the bottom edge, but whose mask stops `short_px` above it.
///
/// Pass 1 flags the bottom (mask is within the gate margin). Pass 2 then has to decide the last
/// `short_px` rows from colour alone — they sit inside the unknown ring, so the verdict is produced
/// by the matting step and nothing else. `subject_to_border` says whether the image's subject
/// colour actually continues into those rows.
fn border_disagreement_case(subject_to_border: bool) -> (RgbImage, Mask) {
    let (w, h) = (200u32, 200u32);
    let (x0, x1) = (80u32, 120u32);
    let mask_bottom = 190u32; // mask stops here
    let image_bottom = if subject_to_border { h } else { mask_bottom };

    let mut image = RgbImage::from_pixel(w, h, Rgb([120, 120, 120]));
    let mut mask = Mask { width: w, height: h, data: vec![0u8; (w * h) as usize] };
    for y in 60..image_bottom {
        for x in x0..x1 {
            image.put_pixel(x, y, Rgb([230, 230, 230]));
        }
    }
    for y in 60..mask_bottom {
        for x in x0..x1 {
            mask.data[(y * w + x) as usize] = 255;
        }
    }
    (image, mask)
}

#[test]
fn refinement_extends_the_mask_to_the_border_when_the_colour_continues() {
    let (image, mask) = border_disagreement_case(true);
    let inst = instance(mask, Rect { x: 80, y: 60, w: 40, h: 130 });
    let input = RefinementInput { working_image: &image, original_image: &image, instance: &inst };

    let result = classify_instance(&input, 20, RefineParams::default());

    // The raw mask stops 10px short of the bottom, so this verdict can only come from the matting
    // step reclassifying the unknown ring. If pass 2 were inert, this would be `false`.
    assert!(
        result.touches_edges.contains(&Edge::Bottom),
        "matting should have carried the subject to the border, got {:?}",
        result.touches_edges
    );
}

#[test]
fn refinement_does_not_extend_the_mask_when_the_colour_stops() {
    let (image, mask) = border_disagreement_case(false);
    let inst = instance(mask, Rect { x: 80, y: 60, w: 40, h: 130 });
    let input = RefinementInput { working_image: &image, original_image: &image, instance: &inst };

    let result = classify_instance(&input, 20, RefineParams::default());

    // Same geometry, but the last rows are background-coloured: matting must leave them background.
    assert!(
        !result.touches_edges.contains(&Edge::Bottom),
        "matting should not have invented a border touch, got {:?}",
        result.touches_edges
    );
}
