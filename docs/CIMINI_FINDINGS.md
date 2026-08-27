# CSP behavior on the CiMini dataset — findings

Run: current `main.rs` pipeline (`load → detect → geometry::plan → fill::render → resize::resize_to_spec → save`),
executed sequentially and instrumented per step (see `docs/CIMINI_TEST_LOG.md` for the raw timings).

Dataset: `test/datasets/CiMini` from the `prism` repo. 116 images assembled from the dataset's flat
files, its three loose subfolders (`26182-Denim-801`, `99984901`, `foldercontainsID99984905`), and
`3 images.zip`. Excludes `Brackets-Complete.xlsx`, `README.md`, and the three `expected-*.json`
oracle files (not images).

## Headline result

All 116 images processed without error (0 failures). No crashes, no panics, nothing left unprocessed.

## Detection outcome distribution

| `DetectionKind` | Count |
|---|---|
| `Subject` | 112 |
| `SalientSquare` | 2 |
| `WholeFrame` | 2 |

| Edges intersected | Count |
|---|---|
| 0 | 113 |
| 1 | 1 |
| 2 | 1 |
| 4 | 1 |

**113/116 (97%) register as touching zero image edges.** This reproduces the pattern already
identified in decision #16 of the spec (104/107 in the prior sample) on a different, larger sample —
the edge-intersection analyzer is very likely still misclassifying real edge-touching geometry as
"0 edges," rather than 97% of this dataset's shots genuinely floating clear of every border. This
routes almost everything through `center_and_stretch` instead of `detail_crop`.

## Detection confidence

`det.confidence` ranges 0.10–1.00, average 0.56, roughly evenly spread: 37 images <0.4, 38 between
0.4–0.7, 41 ≥0.7. There is no confidence cliff — the detector produces a graded, not bimodal, signal
across this dataset, so a hard confidence threshold for routing would need tuning against ground
truth rather than picking an obvious knee.

## Shadow suppression: confirmed inert

`det.hard_shadow_fraction` is **0.00 on every single image**, including the ones with visible drop
shadows in the dataset (e.g. the `100267_*`, `OMB-E166-BV_*` product-on-white shots). This is a live
confirmation of decision #12: the geodesic detection path never populates a nonzero shadow fraction,
i.e. the shadow-carve logic is not exercised by the current pipeline. This directly explains problem
P1 (box creeping into shadow) — the field exists in the `Detection` struct but nothing upstream of it
in the live path is providing a signal.

## The "nonna"-style whole-frame case reproduces

`OMB-E180-BV_6.jpg` (1624×2080, a real-life/non-flat-background shot per decision #14) detects as
`Subject`, 0 edges, confidence 0.53, and plans a fill target `side=2131` — **larger than the image's
own longest dimension (2080)**. That means the planned box+margin already exceeds the real pixels
available before any stretch is applied, so `center_and_stretch` grows canvas on both axes to reach
it. This is the double-axis background stretch described in decision #14, and it is still happening
in the current build. Fixing this is exactly the motivation for the CoG/MSR crop-in-first redesign
already agreed in the spec (decision #15).

## Upscale / downscale spread

Final square side vs. the input's longest dimension:
- Most images upscale mildly (5–8%) to hit an `[800, 2000]`-clamped square side — well inside the 42% cap.
- One outlier, `triggered_black-tshirt-front-detail.jpg` (677×677 → 800), upscales **+18.2%** — still under the 42% ceiling but the largest margin-to-cap observed in this sample.
- The two largest source images (`87186790_1.jpg`, `87186790_2.jpg`, 3543×5314) are heavily **downscaled** (side 3567–4394 → clamped output 2000), a ~62% reduction — these dominate the runtime tail (see below).
- 3/116 images arrive already square and skip fill entirely (`already_square=true`).

## Performance

- **Total wall time, 116 images, sequential, single core:** 23.9s (avg 206 ms/image, median 125 ms, p90 317 ms).
- Step share of total time: **detect 33%**, **fill 27%**, **resize 17%**, **save 14%**, **load 9%**. `geometry::plan` is consistently ~0 ms (pure arithmetic, no image ops) — it never shows up as a cost center.
- Runtime is dominated by a handful of large or low-confidence images, not by the median case:
  - `87186790_1.jpg` (3543×5314): 2727 ms total — fill (1654 ms) is the bottleneck at this resolution because the background-stretch fill operates on the multi-megapixel crop before the final downscale.
  - `2021_3024_46_B.jpg` (2500×2500): 1055 ms, `100267_6 - BW001_c.jpg` (2500×2500): 683 ms — same pattern, fill cost scales with megapixels.
  - `99218809_det0.jpg` / `99218809_det1.jpg` / `99218793_det1.jpg` / `99218810_det1.jpg` (all 800×800, small): 300–354 ms each, but here **detect** is the cost (270–304 ms), not fill — these are the lowest-confidence detections in the set (0.10–0.54), consistent with the detector doing more search/iteration work when the scene is ambiguous.
  - In short: large images are fill/resize-bound; small, hard-to-classify images are detect-bound.

## Practical implications for the redesign

1. Shadow-carve is not just "regressed" — it is measurably zero-signal on a 116-image real sample. Any P1 fix needs a real replacement source of shadow evidence in the live path (SC's contrast/shadow analysis, decision #20), not a re-wire of the old dead code, since the old code's *output* was never actually reaching this dataset either.
2. The edge-intersection failure (113/116 reading as 0-edge) is confirmed at a larger scale than before — the planned 4.2%-border-ring Canny approach (decision #17) has a big, measurable population of currently-misrouted images to validate against once implemented.
3. The nonna-style over-large `side` (exceeding the source's longest dimension) is not a one-off — it's the direct, reproducible symptom that CoG/MSR crop-in-first is meant to fix. `OMB-E180-BV_6.jpg` is a ready-made regression case for that work.
4. Fill cost on large source images (2500px+) is nontrivial (0.4–1.7s) — worth keeping in mind once SC's extra analysis (SLIC, geodesic prior, contrast binning) adds more up-front cost per image; the detect-bound low-confidence cases suggest the existing detector already spends real time iterating on ambiguous shots.
