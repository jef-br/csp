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
/// sides of the subject's longest axis — roughly 2.1% of the side on each.
pub const R1_MARGIN: f32 = 0.042;

/// R1 safety inset, part one: how far the *working-resolution* mask boundary can be wrong, in
/// working pixels. Scaled up to full resolution before use, because that is where the error lands.
pub const R1_SAFETY_MASK_PX: f32 = 2.0;

/// R1 safety inset, part two: how soft the boundary is in the full-resolution image itself —
/// antialiasing, JPEG ringing, a hairline of contact shadow. Already in full-resolution pixels, so
/// it is not scaled.
pub const R1_SAFETY_EDGE_PX: f32 = 2.0;
