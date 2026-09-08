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

/// Pass-2 band width: how far in from the border the refinement looks.
pub const REFINE_BAND_PX: u32 = 20;

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
