use csp_shot_classifier::segmentation::{Instance, Mask};
use csp_shot_classifier::{classify_instance, geometry::Rect, refine::RefineParams, refine::RefinementInput, Edge};
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
