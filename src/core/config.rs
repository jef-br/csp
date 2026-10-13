//! Compile-time tuning constants.
//!
//! Detection and geometry constants retired with the classical detector; the routes that replace
//! them will add their own here as they grow past stubs.

/// JPEG quality for the saved output.
pub const JPEG_QUALITY: u8 = 95;

// ---- Shot classification ----------------------------------------------------------------------
//
// The classifier runs on a downscaled *working* copy, and its pass-2 refinement then works against
// full-resolution pixels. The three params below are expressed in working-resolution pixels, so
// they are coupled to WORKING_SIZE — change one and the others shift meaning.

/// Longest side of the working-resolution copy the segmentation model runs on.
pub const WORKING_SIZE: u32 = 1024;

/// Pass-1 gate: an edge is worth refining when foreground appears within this many pixels of it.
pub const GATE_MARGIN_PX: u32 = 20;

/// Pass-2 band width: how far in from the border the refinement looks. Also the *wide* band of
/// the graze test below.
pub const REFINE_BAND_PX: u32 = 20;

/// Pass-2 graze test, narrow band: compared against `REFINE_BAND_PX` to tell a subject truncated
/// by the frame from one that merely grazes it.
///
/// Keep it well below the wide band — the test's separation collapses as the two converge. Across
/// 46 masks the gap between the worst graze and the best bleed-off was 0.31 at 5px/20px, but only
/// 0.18 at 10px/20px. Not narrower than ~5 either: the mask over-reaches the border by about one
/// antialiased row, and a 2px band gives that one bad row half the vote.
pub const REFINE_NARROW_BAND_PX: u32 = 5;

/// Pass-2 graze test threshold. Below this ratio of narrow-band to wide-band coverage the contact
/// is a graze — the silhouette runs *along* the border instead of off it — and the edge reports as
/// not touching, so the image routes R1 rather than R2.
///
/// Sits in an empty gap: the two hand-labelled grazes measure 0.611 and 0.614, and all 22
/// bleed-offs measure 0.926 or above. Size cannot make this call — a graze at 17.0% of the edge
/// and a real bleed-off at 17.7% differ only in shape.
pub const GRAZE_RATIO_MAX: f32 = 0.80;

/// Below this fraction of the frame, a mask is not a subject — see `shotcode::mask_too_small`.
///
/// Measured over the 37-image set: the two masks that describe nothing cover 0.0% and 4.4% of the
/// frame, and the smallest mask that describes a real product covers 13.5%. Nothing lands between
/// them, so the exact value barely matters.
pub const MASK_MIN_COVERAGE: f64 = 0.05;

// ---- Backdrop trim ----------------------------------------------------------------------------
//
// Before BiRefNet sees an image, the empty backdrop around the product is cropped away
// (`preprocessor::trim`), so the product fills more of what the 384px model looks at. Measured on the
// 384-image batch against the untrimmed run: 17 of 35 empty masks became real masks, none of the
// images that already worked became empty, and every edge verdict (EIX) was unchanged.

/// Trim step 1: per-channel distance from a corner colour within which a pixel counts as that backdrop.
pub const TRIM_CORNER_TOL: i32 = 10;

/// Trim step 2: the same tolerance, in buckets of 4 colour levels, around every colour found on the
/// edge of the step-1 result. It absorbs noise and soft lighting.
pub const TRIM_EDGE_TOL_BUCKETS: i32 = 1;

/// Trim step 2: an edge pixel only counts as backdrop if it is within this per-channel distance of the
/// median edge colour. Keeps a product that touches the edge from being learned as backdrop.
pub const TRIM_EDGE_SPREAD: i32 = 40;

/// Share of a row or column that may be non-backdrop and still be trimmed away (stray noise pixels).
pub const TRIM_OUTLIER: f32 = 0.003;

/// Trim step 3: the box is grown back out by a margin of its own size, so the edge verdict (EIX) cannot
/// change. The margin runs from `TRIM_MARGIN_MIN` when little was trimmed to `TRIM_MARGIN_MAX` when
/// nearly everything was.
pub const TRIM_MARGIN_MIN: f32 = 0.10;
pub const TRIM_MARGIN_MAX: f32 = 0.20;

/// A trim leaving less than this share of the frame, or a side shorter than `TRIM_MIN_SIDE` pixels, is
/// not trusted (the product was probably mistaken for backdrop) and the whole frame is used instead.
pub const TRIM_MIN_AREA: f32 = 0.02;
pub const TRIM_MIN_SIDE: u32 = 24;

/// Truncation guard. A mask that runs into a crop edge which is not the image edge may continue past
/// it (a white garment on a white backdrop, trimmed too tight). "Runs into" = at least
/// `TRIM_CUT_FRACTION` of that edge's length is mask, within `TRIM_CUT_BAND_PX` of it.
pub const TRIM_CUT_BAND_PX: u32 = 3;
pub const TRIM_CUT_FRACTION: f32 = 0.03;

/// A truncated side is grown by this share of the crop's size and the model run again, at most
/// `TRIM_GROW_ROUNDS` times; if the mask still runs into the cut, the untrimmed result is used.
pub const TRIM_GROW: f32 = 0.25;
pub const TRIM_GROW_ROUNDS: u32 = 2;

/// Pass-2 safety margin excluded around the mask when sampling background colour.
pub const REFINE_SAFETY_PX: u32 = 10;

/// Pass-2 context grown around the band-intersection region before cropping, so the colour
/// samplers have enough pixels to work with.
pub const REFINE_CONTEXT_PX: u32 = 30;

// ---- Output envelope --------------------------------------------------------------------------
//
// Applied by the exporter, which every route ends at, so no route can bypass it. The scale is
// uniform, so a square in stays a square out.

/// Minimum output square side.
pub const MIN_SIZE: u32 = 800;
/// Maximum output square side.
pub const MAX_SIZE: u32 = 2000;

// ---- R1 · Center & Stretch --------------------------------------------------------------------

/// Margin R1 leaves around the subject, as a fraction of the square's side.
///
/// The square is `max(bbox_w, bbox_h) * (1 + R1_MARGIN)`, so the margin is shared between the two
/// sides of the subject's longest axis — roughly 4.2% of the side on each.
pub const R1_MARGIN: f32 = 2.0*0.042;

/// R1 safety inset, part one: how far the *working-resolution* mask boundary can be wrong, in
/// working pixels. Scaled up to full resolution before use, because that is where the error lands.
pub const R1_SAFETY_MASK_PX: f32 = 2.0;

/// R1 safety inset, part two: how soft the boundary is in the full-resolution image itself —
/// antialiasing, JPEG ringing, a hairline of contact shadow. Already in full-resolution pixels, so
/// it is not scaled.
pub const R1_SAFETY_EDGE_PX: f32 = 2.0;
