//! Compile-time tuning constants.
//!
//! Detection constants mirror PRISM `ClassifyConfig.json -> SubjectDetector`; sizing/encoding
//! constants mirror the reference `process_images.py` defaults. No shadow defaults: every value
//! that governs behaviour is named here, never buried inline.

// ---- Detection ------------------------------------------------------------------------------

/// Analysis resolution cap. Detection runs at this longest-side (downscaled if larger, full if not).
pub const ANALYSIS_SIZE: u32 = 1024;
/// Escalation resolution for non-flat / real-life backgrounds.
pub const REALLIFE_ANALYSIS_SIZE: u32 = 1536;
/// Border-ring chroma residual above which we escalate to the real-life pass.
pub const REALLIFE_RESIDUAL_THRESHOLD: f64 = 3.5;

/// Local-texture window (box filter side) used for the std-dev texture signal.
pub const TEXTURE_WINDOW: i32 = 14;
/// High-pass sigma: anything blurrier than this is not surface texture (shadow penumbra).
pub const TEXTURE_DETAIL_SIGMA: f64 = 4.0;
/// Robust-spread multiples above background that count as product.
pub const OUTLIER_SPREAD_MULTIPLIER: f64 = 4.0;

/// Min blob size as a fraction of image area.
pub const MIN_COMPONENT_AREA_FRACTION: f64 = 0.0005;
/// Min blob size as a fraction of the largest blob (studio pass).
pub const MIN_COMPONENT_AREA_RATIO: f64 = 0.05;
/// Min blob size as a fraction of the largest blob (real-life escalation pass, stricter).
pub const REALLIFE_MIN_COMPONENT_AREA_RATIO: f64 = 0.12;
/// Min blob size as an absolute pixel floor.
pub const MIN_COMPONENT_AREA_PIXELS: f64 = 25.0;

/// A box covering this much of the frame counts as "no detection".
pub const WHOLE_FRAME_FRACTION: f64 = 0.985;
/// Opening size (px) that strips a hard shadow's thin edge from texture-only pixels.
pub const SHADOW_EDGE_KERNEL: i32 = 15;
/// Fraction of texture-only pixels stripped as shadow edge, above which shadow is "present".
pub const HARD_SHADOW_EVIDENCE_FRACTION: f64 = 0.05;

// Shadow suppression: a cast shadow is a low-texture, low-chroma darkening of the background. It is
// told apart from a dark PRODUCT by the *amount* of darkening — a soft/medium shadow drops Lab L
// within a band, whereas a black product drops it far past the band. Grey products are saved by
// their surface texture. Lab L is 0..100.
/// Minimum Lab-L drop below the fitted background lightness to consider a pixel shadow. Kept above
/// zero so white-on-white product (drop ~0) is never carved — only darker-than-background pixels.
pub const SHADOW_DARKEN_MIN: f32 = 4.0;
/// Maximum Lab-L drop that still reads as shadow; darker than this is treated as product.
pub const SHADOW_DARKEN_MAX: f32 = 45.0;
/// A shadow pixel must be near-neutral in ABSOLUTE Lab a/b (sqrt(a²+b²) below this). Absolute
/// neutrality — not distance from the (possibly tinted) background — is what separates a colourless
/// cast shadow on a warm sweep from real product colour.
pub const SHADOW_MAX_ABS_CHROMA: f32 = 6.0;
/// A shadow pixel must be near-featureless (texture below this).
pub const SHADOW_MAX_TEXTURE: f32 = 4.0;

/// Auto-Canny threshold width around the median gradient.
pub const CANNY_SIGMA: f64 = 0.33;
/// Gap-closing size for the Canny edge map before border flood-fill.
pub const CANNY_CLOSE_KERNEL: i32 = 5;
/// Border-ring width as a fraction of each dimension (background sample + edge test).
pub const BORDER_RING_FRACTION: f64 = 0.02;
/// Studio-sweep speckle open kernel (0 = skip, used when background is flat).
pub const SWEEP_SPECKLE_KERNEL: i32 = 7;

/// Weak-threshold multiplier for hysteresis: a connected pixel above this fraction of the strong
/// limit is absorbed into the product mask (recovers thin low-contrast appendages).
pub const HYSTERESIS_WEAK_FRACTION: f64 = 0.7;

/// Flood guard: a weak hysteresis component is absorbed only if its area is at most this multiple
/// of the strong seed area it touches. A thin strap adds little; a background flood adds a lot.
pub const HYSTERESIS_FLOOD_CAP: f64 = 0.6;

/// Chroma-distance floor (Lab units) that counts as product.
pub const CHROMA_FLOOR: f64 = 2.0;
/// Local-contrast floor that counts as product surface.
pub const TEXTURE_FLOOR: f64 = 2.0;

/// CLAHE clip limit + tile size for the detection lightness channel.
pub const CLAHE_CLIP_LIMIT: f64 = 2.0;
pub const CLAHE_TILE_SIZE: u32 = 16;

/// Fraction of an outermost mask row/col that must be product to count as an edge contact.
pub const BLEED_CONTACT: f64 = 0.2;
/// Canvas edges the subject may run off before the frame is treated as a detail shot.
pub const BLEED_EDGES: u32 = 2;
/// Border texture above this means the frame is product-filled (detail shot), not a sweep.
pub const SWEEP_TEXTURE_LIMIT: f64 = 2.0;
/// Zoom factor for a frame-filling detail shot: the salient square is this fraction of the shorter
/// side, cropping slightly inward onto the busiest content instead of taking the whole frame.
pub const SALIENT_ZOOM: f64 = 0.9;

/// Minimum confidence a subject box needs to be trusted for routing.
pub const SUBJECT_PROMOTION_MIN_CONFIDENCE: f64 = 0.35;

// ---- Superpixel figure/ground segmentation --------------------------------------------------

/// Working resolution (longest side) for superpixel segmentation.
pub const SEG_SIZE: u32 = 480;
/// Target number of superpixels.
pub const SEG_K: usize = 700;
/// SLIC compactness (higher = more square/regular superpixels).
pub const SEG_COMPACT: f32 = 14.0;
/// SLIC iterations.
pub const SEG_ITERS: usize = 8;
/// Colour step (Lab) below which movement between adjacent superpixels is free (intra-background).
pub const GEO_CLIP: f32 = 3.0;
/// Geodesic distance to the border above which a superpixel is foreground.
pub const GEO_THRESHOLD: f32 = 12.0;
/// Below this foreground fraction the segmentation is treated as "found nothing"; combined with high
/// border texture that means a frame-filling detail shot (else a blank frame).
pub const SEG_MIN_FG_FRACTION: f64 = 0.02;
/// Lightness spread (Lab L p95−p5) below which the frame is low-contrast and gets a CLAHE rescue
/// before segmentation — lifts a white-on-white / black-on-black product's silhouette into view.
pub const LOW_CONTRAST_L_SPREAD: f32 = 30.0;

// ---- Low-contrast fallback (zone stretch) ---------------------------------------------------
/// Bilateral denoise for the low-contrast rescue: spatial sigma (px), range sigma (Lab L), radius.
pub const LC_BILATERAL_SPATIAL_SIGMA: f64 = 3.0;
pub const LC_BILATERAL_RANGE_SIGMA: f64 = 6.0;
pub const LC_BILATERAL_RADIUS: i32 = 4;
/// Standard-deviation count for the zone-stretch endpoints (μ ± k·σ). Aggression scales as 1/σ.
pub const LC_SIGMA_K: f64 = 2.0;
/// Feather sigma (px) for blending the boosted zone back into the frame.
pub const LC_FEATHER_SIGMA: f64 = 6.0;
/// Below this fraction of dark pixels there is no real shadow cluster — the whole frame is the zone.
pub const LC_DARK_FRACTION_MIN: f64 = 0.08;

// ---- Geometry / sizing (process_images.py defaults) -----------------------------------------

/// Margin per side, as a fraction of the product's longest edge.
pub const MARGIN_FRACTION: f64 = 0.042;
/// Minimum output square side.
pub const MIN_SIZE: u32 = 800;
/// Maximum output square side.
pub const MAX_SIZE: u32 = 2000;
/// Maximum whole-image upscale factor (background fills any remaining gap).
pub const MAX_UPSCALE: f64 = 1.42;
/// Hard-shadow bottom-edge shrink applied to the box before a center-and-stretch.
pub const SHADOW_BOTTOM_SHRINK_FRACTION: f64 = 0.06;
/// Radius (fraction of half-width) of the saliency center-prior bias.
pub const CENTER_PRIOR_FALLOFF: f32 = 0.8;
/// Below this much background on an axis, pad (replicate) instead of stretching a band.
pub const LOW_BACKGROUND_FRACTION: f64 = 0.06;

// ---- Encoding -------------------------------------------------------------------------------

/// JPEG quality for the saved output.
pub const JPEG_QUALITY: u8 = 95;
