//! Shot code: the classifier's side of the per-image tag — the `EIX` (edge intersection) bits.
//!
//! The `BGC`/`FGC` colours are not here: they are sampled by the exporter
//! (`exporter::colors`), which also assembles the full tag.
//!
//! Partial — a background-type (`BG`) tag is still to come. Until then,
//! `mask_too_small` reports the case that tag would cover — a mask covering too
//! little of the frame to be a subject — as *information only*.
//! `eix` always reports the classifier's real verdict:
//! a tag must never claim an edge intersection the pipeline didn't actually
//! route on. Routing (`processor::routes::Route::select`) reads only
//! `touches_edges`, never this heuristic, so letting it override `eix` would
//! make the tag lie about what the pipeline did with the image.

use super::super::config::MASK_MIN_COVERAGE;
use super::{Edge, Mask};
use image::RgbImage;

/// EIX string when the classifier produced no verdict at all. Four
/// underscores mirror the 4-bit TRBL width of a real verdict.
pub const NO_VERDICT_EIX: &str = "____";

/// Does the mask cover too little of the frame to be a subject?
///
/// One measurement, deliberately: the share of the frame the mask covers, against
/// [`MASK_MIN_COVERAGE`]. Nothing about *why* it is small — the two images this catches are
/// close-ups where the frame was already all product and the segmenter latched onto the
/// highest-contrast detail in it, but the route does not need that story to be true, only that
/// there is no subject here worth framing around.
///
/// A second condition — "and no uniform background was found" — was measured and left out. It is
/// the only thing that would tell a genuinely small product on clean paper, jewellery being the
/// obvious case, from this. Across the 37-image set the two rules flag exactly the same images, so
/// it earns nothing yet; add it back against a picture that needs it.
pub fn mask_too_small(working: &RgbImage, mask: &Mask) -> bool {
    let fg = mask.data.iter().filter(|&&v| v > 0).count();
    let total = (working.width() as usize * working.height() as usize).max(1);
    (fg as f64 / total as f64) < MASK_MIN_COVERAGE
}

/// 4-bit edge-intersection string in fixed TRBL order.
pub fn eix_bits(edges: &[Edge]) -> String {
    let bit = |e: Edge| if edges.contains(&e) { '1' } else { '0' };
    [bit(Edge::Top), bit(Edge::Right), bit(Edge::Bottom), bit(Edge::Left)]
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eix_is_trbl_order() {
        assert_eq!(eix_bits(&[]), "0000");
        assert_eq!(eix_bits(&[Edge::Top]), "1000");
        assert_eq!(eix_bits(&[Edge::Right]), "0100");
        assert_eq!(eix_bits(&[Edge::Bottom]), "0010");
        assert_eq!(eix_bits(&[Edge::Left]), "0001");
        assert_eq!(eix_bits(&[Edge::Right, Edge::Bottom]), "0110");
    }

    #[test]
    fn an_almost_empty_mask_is_too_small() {
        // 6 in miniature: the segmenter marked next to nothing.
        let img = RgbImage::from_pixel(40, 40, image::Rgb([200, 200, 200]));
        let mut mask = Mask { width: 40, height: 40, data: vec![0; 40 * 40] };
        mask.data[20 * 40 + 20] = 255;
        assert!(mask_too_small(&img, &mask));
    }

    #[test]
    fn a_mask_over_the_floor_is_kept_however_plain_the_backdrop() {
        // Coverage is the whole test — the backdrop plays no part in it. 10% of the frame, which
        // sits between 30's 4.4% and the 13.5% of the smallest mask that framed correctly.
        let img = RgbImage::from_pixel(100, 100, image::Rgb([255, 255, 255]));
        let mut mask = Mask { width: 100, height: 100, data: vec![0; 100 * 100] };
        for y in 45..55 {
            for x in 0..100 {
                mask.data[y * 100 + x] = 255;
            }
        }
        assert!(!mask_too_small(&img, &mask));
    }
}
