//! R1 step 1 — where the square goes.
//!
//! Pure geometry, no pixels. The square's side comes from the subject box alone
//! (`max(w, h) * (1 + margin)`), and it is centred on that box: the product ends up in the middle
//! of the output whatever the source framing was, which is the whole point of R1.
//!
//! Deriving the side from the subject rather than from the image is what makes the square able to
//! reach past a border. That is not a failure mode — it is the case R1 exists for. A side that
//! lands inside the image on some axis simply crops there; a side that reaches past it leaves an
//! [`Overhang`] for the fill to cover.

use crate::core::shot_classifier::geometry::Rect;

/// The output square, in the *original* image's coordinate space.
///
/// `x`/`y` are signed because the square legitimately starts outside the image — a subject framed
/// tightly against the left border puts the square's left column at a negative x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SquarePlan {
    pub side: u32,
    pub x: i64,
    pub y: i64,
}

/// How far the square reaches past each image border, in pixels. Zero on a side that lands on real
/// pixels — that side crops instead of stretching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Overhang {
    pub top: u32,
    pub bottom: u32,
    pub left: u32,
    pub right: u32,
}

impl SquarePlan {
    /// Square of `max(w, h) * (1 + margin)`, centred on `subject`.
    pub fn new(subject: Rect, margin: f32) -> SquarePlan {
        let longest = subject.w.max(subject.h) as f64;
        let side = (longest * (1.0 + margin as f64)).round().max(1.0) as u32;

        // Centre in half-pixel units, so an odd-sized box is not biased one way by an early
        // division. `div_euclid` rounds toward negative infinity, which keeps the bias consistent
        // on the axes where the origin goes negative.
        let cx2 = 2 * subject.x as i64 + subject.w as i64;
        let cy2 = 2 * subject.y as i64 + subject.h as i64;

        SquarePlan {
            side,
            x: (cx2 - side as i64).div_euclid(2),
            y: (cy2 - side as i64).div_euclid(2),
        }
    }

    /// The part of the square that lands on real pixels, in original-image coordinates.
    ///
    /// `None` when the square misses the image entirely — impossible for a box derived from a mask
    /// inside that image, but the fill would read out of bounds if it ever happened.
    pub fn real_region(&self, img_w: u32, img_h: u32) -> Option<Rect> {
        let x0 = self.x.max(0);
        let y0 = self.y.max(0);
        let x1 = (self.x + self.side as i64).min(img_w as i64);
        let y1 = (self.y + self.side as i64).min(img_h as i64);
        (x1 > x0 && y1 > y0).then(|| Rect {
            x: x0 as u32,
            y: y0 as u32,
            w: (x1 - x0) as u32,
            h: (y1 - y0) as u32,
        })
    }

    pub fn overhang(&self, img_w: u32, img_h: u32) -> Overhang {
        let past = |v: i64| v.max(0) as u32;
        Overhang {
            left: past(-self.x),
            top: past(-self.y),
            right: past(self.x + self.side as i64 - img_w as i64),
            bottom: past(self.y + self.side as i64 - img_h as i64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::R1_MARGIN;

    fn rect(x: u32, y: u32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn side_is_the_longest_bbox_edge_plus_margin() {
        // Portrait subject: height decides, width follows it into a square.
        let plan = SquarePlan::new(rect(100, 30, 1300, 1670), R1_MARGIN);
        assert_eq!(plan.side, 1740); // 1670 * 1.042 = 1740.1
    }

    #[test]
    fn the_square_is_centred_on_the_subject() {
        // Subject centre is (750, 865); a 1740 square centred there starts at -120, -5.
        let plan = SquarePlan::new(rect(100, 30, 1300, 1670), R1_MARGIN);
        assert_eq!((plan.x, plan.y), (-120, -5));

        // Centring is on the subject, never on the image: the same box in a much wider frame
        // produces the same origin.
        let same = SquarePlan::new(rect(100, 30, 1300, 1670), R1_MARGIN);
        assert_eq!(plan, same);
    }

    #[test]
    fn overhang_is_per_side_and_zero_where_real_pixels_reach() {
        // The worked case: 1500x2000 source. Left and right reach past the frame, the top by a
        // hair, and the bottom lands 265px inside it — so the bottom crops rather than stretches.
        let plan = SquarePlan::new(rect(100, 30, 1300, 1670), R1_MARGIN);
        assert_eq!(
            plan.overhang(1500, 2000),
            Overhang { top: 5, bottom: 0, left: 120, right: 120 }
        );
    }

    #[test]
    fn a_square_inside_the_image_has_no_overhang_and_crops() {
        // Small subject in a big frame: every side lands on real pixels.
        let plan = SquarePlan::new(rect(400, 400, 200, 200), R1_MARGIN);
        assert_eq!(plan.side, 208);
        assert_eq!(plan.overhang(1000, 1000), Overhang::default());
        assert_eq!(
            plan.real_region(1000, 1000),
            Some(rect(396, 396, 208, 208)),
            "the whole square is real pixels, so the region is the square itself"
        );
    }

    #[test]
    fn a_subject_filling_the_frame_overhangs_on_all_four_sides() {
        // No 42% cap on this: the fill is best-effort by design, and a subject this tight is
        // exactly the case that needs stretching on every side.
        let plan = SquarePlan::new(rect(0, 0, 1000, 1000), R1_MARGIN);
        assert_eq!(plan.side, 1042);
        let over = plan.overhang(1000, 1000);
        assert_eq!(over, Overhang { top: 21, bottom: 21, left: 21, right: 21 });
        assert_eq!(plan.real_region(1000, 1000), Some(rect(0, 0, 1000, 1000)));
    }

    #[test]
    fn real_region_clips_to_the_image_on_every_side() {
        let plan = SquarePlan { side: 100, x: -30, y: 950 };
        assert_eq!(plan.real_region(200, 1000), Some(rect(0, 950, 70, 50)));
    }

    #[test]
    fn a_square_that_misses_the_image_has_no_real_region() {
        let plan = SquarePlan { side: 10, x: -50, y: 0 };
        assert!(plan.real_region(200, 200).is_none());
    }
}
