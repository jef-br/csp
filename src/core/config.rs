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

// ---- Optimiser trim ---------------------------------------------------------------------------
//
// After the colour balance, the optimiser crops the backdrop away so the 384px model sees the product
// larger (`preprocessor::optimizer`). All four numbers are in working-resolution pixels or shares.

/// Backdrop samples taken along each edge of the frame, evenly spaced from the edge's start
/// (4 gives 0%, 25%, 50% and 75%; 100% is the next edge's 0%). The lowest and highest value of every
/// colour channel over all the kept samples is the colour range that counts as backdrop.
pub const TRIM_SAMPLES_PER_EDGE: u32 = 4;

/// Outlier drop on the border samples. A sample is kept when its L1 colour distance to the
/// per-channel median of the 16 samples (`|ΔR|+|ΔG|+|ΔB|`, so 0 to 765) stays at or below
/// `TRIM_OUTLIER_FLOOR + TRIM_OUTLIER_K * MAD`, where MAD is the median of those distances. The
/// floor is a fixed safety band around the median so a near-uniform edge is not shredded; the
/// MAD-scaled term grows the band on noisy edges. `TRIM_MIN_KEEP` samples are always kept — the
/// ones closest to the median — so a mostly-product edge cannot collapse the backdrop range.
pub const TRIM_OUTLIER_FLOOR: u32 = 120;
pub const TRIM_OUTLIER_K: f64 = 1.5;
pub const TRIM_MIN_KEEP: usize = 4;

/// A pixel outside the backdrop range only counts as product when it sits inside a block of this
/// many pixels per side that is entirely outside it. Smaller specks and thinner lines are treated as
/// backdrop (a morphological closing of the backdrop with a square window of this size).
pub const TRIM_CLOSE_K: u32 = 2;

/// The box around the product is first squared to its largest side, then grown on every side by the
/// larger of `TRIM_MARGIN_MIN_PX` and `TRIM_MARGIN_SHARE` of that side. The resulting square is slid to
/// stay inside the frame; it is never cut unless no square fits, and then the margin is the minimum
/// (see `optimizer::square_crop`).
pub const TRIM_MARGIN_MIN_PX: u32 = 10;
pub const TRIM_MARGIN_SHARE: f64 = 0.10;

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
