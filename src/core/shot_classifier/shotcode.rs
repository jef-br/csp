//! Shot code: the compact per-image verdict used to tag exported files.
//!
//! `EIX` (edge intersection) plus optional `BGC`/`FGC` colours. Both the
//! pipeline exporter and the `run_dir` dev harness derive the code here, so
//! the two never drift.
//!
//! `BGC`/`FGC` are sampled from the working-resolution image partitioned by
//! the segmentation mask (background = inverse mask, foreground = mask).
//!
//! Partial — a background-type (`BG`) tag is still to come. Until then,
//! `full_bleed` flags the case that tag would cover — a tiny mask with no
//! uniform background, i.e. a probable full-bleed close-up — as
//! *information only*. `eix` always reports the classifier's real verdict:
//! a tag must never claim an edge intersection the pipeline didn't actually
//! route on. Routing (`processor::routes::Route::select`) reads only
//! `touches_edges`, never this heuristic, so letting it override `eix` would
//! make the tag lie about what the pipeline did with the image.

use super::{Edge, Mask, ShotClassification};
use image::RgbImage;
use std::collections::HashSet;

/// A colour must occupy at least this fraction of the background region
/// (inverse mask) to be reported as BGC.
const BGC_PRESENCE: f64 = 0.975;
/// A region smaller than this fraction of the frame is too small to sample a
/// colour from.
const MIN_REGION_FRAC: f64 = 0.01;
/// If the mask covers less than this fraction of the frame *and* no uniform
/// background was found, `full_bleed` flags the shot as a probable
/// full-bleed close-up (the subject likely fills the frame). Informational
/// only — see the module doc for why it must not touch `eix`.
/// Interim rule — the texture / BG-type pass will replace it.
const FULL_BLEED_MAX_FG: f64 = 0.05;

/// EIX string when the classifier produced no verdict at all. Four
/// underscores mirror the 4-bit TRBL width of a real verdict.
pub const NO_VERDICT_EIX: &str = "____";

/// The derived shot code for one classified image.
pub struct ShotCode {
    /// 4-bit TRBL edge-intersection string, always the classifier's real
    /// verdict — never touched by `full_bleed` (see the module doc).
    pub eix: String,
    /// Background colour as `rrggbb`, when one colour dominates the
    /// background region.
    pub bgc: Option<String>,
    /// Foreground colour as `rrggbb`, when the subject is big enough to
    /// sample.
    pub fgc: Option<String>,
    /// Informational: a tiny mask with no uniform background, i.e. a
    /// probable full-bleed close-up. Does not affect `eix`.
    pub full_bleed: bool,
}

impl ShotCode {
    /// The filename tag string: `--EIX=<trbl>[--BGC=<hex>][--FGC=<hex>]`.
    /// `--` separates the stem from the tags and each tag from the next; `=`
    /// stands in for the spec's `:` (illegal in Windows filenames).
    pub fn tags(&self) -> String {
        let mut s = format!("--EIX={}", self.eix);
        if let Some(c) = &self.bgc {
            s.push_str("--BGC=");
            s.push_str(c);
        }
        if let Some(c) = &self.fgc {
            s.push_str("--FGC=");
            s.push_str(c);
        }
        s
    }
}

/// Derive the shot code from the working image, its segmentation mask, and
/// the edge verdict.
pub fn derive(working: &RgbImage, mask: &Mask, class: &ShotClassification) -> ShotCode {
    let ShotColors { bgc, fgc } = shot_colors(working, mask);

    let fg = mask.data.iter().filter(|&&v| v > 0).count();
    let total = (working.width() as usize * working.height() as usize).max(1);
    let fg_ratio = fg as f64 / total as f64;

    // Full-bleed close-up: the segmenter found almost no subject and there is
    // no uniform background -> probably the whole frame is subject. Flagged
    // for information only; `eix` stays the real verdict (see module doc).
    let full_bleed = fg_ratio < FULL_BLEED_MAX_FG && bgc.is_none();
    let eix = eix_bits(&class.touches_edges);

    ShotCode { eix, bgc, fgc, full_bleed }
}

/// 4-bit edge-intersection string in fixed TRBL order.
pub fn eix_bits(edges: &[Edge]) -> String {
    let bit = |e: Edge| if edges.contains(&e) { '1' } else { '0' };
    [bit(Edge::Top), bit(Edge::Right), bit(Edge::Bottom), bit(Edge::Left)]
        .into_iter()
        .collect()
}

struct ShotColors {
    bgc: Option<String>,
    fgc: Option<String>,
}

/// BGC and FGC, derived from the original image partitioned by the mask
/// (background = inverse mask, foreground = mask).
fn shot_colors(image: &RgbImage, mask: &Mask) -> ShotColors {
    let (mut fg_hist, mut bg_hist) = (Hist::new(), Hist::new());
    let w = image.width().min(mask.width);
    let h = image.height().min(mask.height);
    for y in 0..h {
        for x in 0..w {
            let p = image.get_pixel(x, y).0;
            if mask.get(x, y) {
                fg_hist.add(p);
            } else {
                bg_hist.add(p);
            }
        }
    }
    let frame = (w as u64 * h as u64).max(1) as f64;

    // BGC: a single colour must cover >97.5% of the inverse-mask region.
    let bgc = (bg_hist.total() as f64 >= frame * MIN_REGION_FRAC)
        .then(|| dominant_cluster(&bg_hist))
        .flatten()
        .filter(|(frac, _)| *frac >= BGC_PRESENCE)
        .map(|(_, c)| hex(c));

    // FGC: the largest colour blob inside the mask -> its weighted-median
    // colour. "Blob" is the fullest histogram cluster (the winning 5-bit bin
    // plus its ±1 neighbours), not a spatially-connected region.
    let fgc = (fg_hist.total() as f64 >= frame * MIN_REGION_FRAC)
        .then(|| dominant_bin(&fg_hist))
        .flatten()
        .map(|top| {
            let blob = cluster_bins(top);
            // Per-channel value histograms over the blob's pixels, for a
            // frequency-weighted median that shrugs off folds/shadows.
            let mut chan = [[0u64; 256]; 3];
            for y in 0..h {
                for x in 0..w {
                    if !mask.get(x, y) {
                        continue;
                    }
                    let p = image.get_pixel(x, y).0;
                    if blob.contains(&bin(p[0] >> 3, p[1] >> 3, p[2] >> 3)) {
                        for c in 0..3 {
                            chan[c][p[c] as usize] += 1;
                        }
                    }
                }
            }
            hex([median(&chan[0]), median(&chan[1]), median(&chan[2])])
        });

    ShotColors { bgc, fgc }
}

/// Frequency-weighted median of a 256-bin value histogram.
fn median(hist: &[u64; 256]) -> u8 {
    let half = hist.iter().sum::<u64>().div_ceil(2);
    let mut cum = 0u64;
    for (v, &c) in hist.iter().enumerate() {
        cum += c;
        if cum >= half {
            return v as u8;
        }
    }
    0
}

/// 5-bit-per-channel RGB histogram (32³ bins) that also carries the running
/// colour sum per bin, so a bin's mean colour is recoverable.
struct Hist {
    count: Vec<u32>,
    sum: Vec<[u64; 3]>,
}

impl Hist {
    fn new() -> Self {
        Hist { count: vec![0; 32 * 32 * 32], sum: vec![[0; 3]; 32 * 32 * 32] }
    }
    fn add(&mut self, p: [u8; 3]) {
        let i = bin(p[0] >> 3, p[1] >> 3, p[2] >> 3);
        self.count[i] += 1;
        self.sum[i][0] += p[0] as u64;
        self.sum[i][1] += p[1] as u64;
        self.sum[i][2] += p[2] as u64;
    }
    fn total(&self) -> u64 {
        self.count.iter().map(|&c| c as u64).sum()
    }
}

fn bin(r: u8, g: u8, b: u8) -> usize {
    ((r as usize) << 10) | ((g as usize) << 5) | (b as usize)
}

/// Index of the fullest histogram bin, or `None` if the region is empty.
fn dominant_bin(h: &Hist) -> Option<usize> {
    if h.total() == 0 {
        return None;
    }
    h.count.iter().enumerate().max_by_key(|(_, &c)| c).map(|(i, _)| i)
}

/// The bin set forming one colour "blob": `top` plus its ±1 neighbours per
/// channel, so JPEG dither around a boundary stays one colour.
fn cluster_bins(top: usize) -> HashSet<usize> {
    let (tr, tg, tb) = ((top >> 10) & 31, (top >> 5) & 31, top & 31);
    let mut set = HashSet::new();
    for dr in -1..=1i32 {
        for dg in -1..=1i32 {
            for db in -1..=1i32 {
                set.insert(bin(
                    (tr as i32 + dr).clamp(0, 31) as u8,
                    (tg as i32 + dg).clamp(0, 31) as u8,
                    (tb as i32 + db).clamp(0, 31) as u8,
                ));
            }
        }
    }
    set
}

/// Fraction of the region occupied by the dominant colour blob, and that
/// blob's mean RGB. `None` if the region is empty.
fn dominant_cluster(h: &Hist) -> Option<(f64, [u8; 3])> {
    let region = h.total();
    let top = dominant_bin(h)?;
    let (mut n, mut s) = (0u64, [0u64; 3]);
    for i in cluster_bins(top) {
        n += h.count[i] as u64;
        s[0] += h.sum[i][0];
        s[1] += h.sum[i][1];
        s[2] += h.sum[i][2];
    }
    (n > 0).then(|| (n as f64 / region as f64, [(s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8]))
}

fn hex(c: [u8; 3]) -> String {
    format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2])
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
    fn tags_are_dash_joined_and_omit_missing_colours() {
        let code = ShotCode {
            eix: "0110".into(),
            bgc: None,
            fgc: None,
            full_bleed: false,
        };
        assert_eq!(code.tags(), "--EIX=0110");

        let code = ShotCode { bgc: Some("ffffff".into()), ..code };
        assert_eq!(code.tags(), "--EIX=0110--BGC=ffffff");

        let code = ShotCode { fgc: Some("102030".into()), ..code };
        assert_eq!(code.tags(), "--EIX=0110--BGC=ffffff--FGC=102030");
    }

    #[test]
    fn full_bleed_is_flagged_but_never_overrides_the_real_eix() {
        // A noisy image so no single colour dominates the background, and an
        // almost-empty mask: `full_bleed` fires, but `eix` must still match
        // the real (empty) verdict — a tag must never claim a route the
        // pipeline didn't take (Route::select only ever reads
        // `touches_edges`, never `full_bleed`).
        let mut img = RgbImage::new(40, 40);
        for (i, px) in img.pixels_mut().enumerate() {
            let v = (i as u32 * 37 % 256) as u8;
            *px = image::Rgb([v, v.wrapping_add(80), v.wrapping_add(160)]);
        }
        let mask = Mask { width: 40, height: 40, data: vec![0; 40 * 40] };
        let class = ShotClassification::default();

        let code = derive(&img, &mask, &class);
        assert!(code.full_bleed);
        assert_eq!(code.eix, "0000");
        assert!(code.bgc.is_none());
    }

    #[test]
    fn uniform_background_reports_bgc_and_keeps_the_verdict_eix() {
        // Solid white frame, empty mask: background is one colour, so BGC is
        // reported, the full-bleed rule does not fire, and EIX stays 0000.
        let img = RgbImage::from_pixel(40, 40, image::Rgb([255, 255, 255]));
        let mask = Mask { width: 40, height: 40, data: vec![0; 40 * 40] };
        let code = derive(&img, &mask, &ShotClassification::default());

        assert!(!code.full_bleed);
        assert_eq!(code.eix, "0000");
        assert_eq!(code.bgc.as_deref(), Some("ffffff"));
    }
}
