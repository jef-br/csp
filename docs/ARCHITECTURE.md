# CSP architecture

Map of every module, its public surface, and how data flows through the pipeline.

**Maintenance rule:** one node per module, one node per `struct`/`enum`. Add a module or a public
function → patch the diagram node in the same commit. A node with no matching file means the
diagram drifted.

> **Diagram status.** The four diagrams this document used to embed (`class-map`, `pipeline-flow`,
> `detection-internals`, `geometry-routing`) were deleted when the classical detector was retired.
> `JBA2B.drawio.svg` survives but has drifted — see [Diagram drift](#diagram-drift) for the
> node-level list of what it still needs. Until that is done the maintenance rule above cannot be
> followed, because there is no node to patch.

---

## 1. Pipeline

`core::process_file` runs five stages in order. `app::batch::run` fans this over every image in the
input folder with `rayon`; on success the source file is deleted, on failure it is left in place.

```
load ──▶ preprocess ──▶ classify ──▶ dispatch ──▶ export
```

| Stage | Module | In → Out |
|---|---|---|
| **load** | `core::preprocessor::load` | path → `Decoded` (image + ICC profile, EXIF applied) |
| **preprocess** | `core::preprocessor::preprocess` | `Decoded` → `Prepared` (sRGB, alpha flattened, + working copy) |
| **classify** | `core::classify` (private) | `Prepared` → `Option<ShotClassification>` |
| **dispatch** | `core::processor::routes::dispatch` | `Prepared` + verdict → `RgbImage` |
| **export** | `core::exporter::export` | `RgbImage` → JPEG on disk, source deleted |

### Why two resolutions

`Prepared` carries the image twice: `original` at full resolution and `working` reduced to
`config::WORKING_SIZE` on its longest side.

The segmentation model runs on `working`. Its pass-2 refinement then re-decides the subject boundary
against full-resolution pixels in `original`, because the mask it produced is upsampled and not
pixel-accurate at the edge. Hand the classifier the same image twice and the scale between them
collapses to 1.0, the refinement ring shrinks to its floor, and pass 2 stops doing anything.

### Why classification is optional

`classify` returns `Option`. `None` means *no verdict* — the model is unavailable, or inference
failed on this image. That is deliberately distinct from `Some` with an empty `touches_edges`, which
means *we looked, and the subject touches no edge*.

They route differently, so a model failure degrades one image to a safe framing instead of being
silently processed as a clean free-standing shot.

---

## 2. Modules

### `app` — the host shell

| Module | Surface | Notes |
|---|---|---|
| `app::batch` | `run(input, output) -> Summary`, `Summary` | rayon fan-out, one level of subfolder recursion |
| `app::shell` | `run()` | Windows double-click flow: desktop `CSP-INPUT` → `CSP-OUTPUT` |
| `app::console` | `init`, `ui_language`, `folder_link`, `pause` | Windows-only; ANSI, OSC-8 links, UI language |
| `app::desktop` | `desktop_dir()` | Windows-only |
| `app::i18n` | `Lang`, `drop_prompt`, `processing`, `finished`, `close_line` | EN/ES/FR/NL/IT/DE |

`console`, `desktop` and `shell` are `#[cfg(windows)]`. On other hosts only the two-argument CLI form
(`csp <input_dir> <output_dir>`) is available, and `i18n` reads as dead code because nothing calls
it — that is expected, not drift.

### `core` — the imaging pipeline

| Module | Surface |
|---|---|
| `core` | `process_file(input, output) -> Result<(), String>` |
| `core::config` | `JPEG_QUALITY`, `WORKING_SIZE`, `GATE_MARGIN_PX`, `REFINE_BAND_PX`, `REFINE_SAFETY_PX`, `REFINE_CONTEXT_PX` |
| `core::preprocessor` | `Decoded`, `Prepared`, `load`, `preprocess`, `prepare` |
| `core::processor::routes` | `Route`, `Route::select`, `dispatch` |
| `core::processor::routes::{center_and_stretch, crop_square, fallback}` | `apply` |
| `core::exporter` | `export`; `save::save_jpeg_srgb`, `icc::srgb_profile` |

The refine params live in `config` next to `WORKING_SIZE` because they are expressed in
working-resolution pixels — change one and the others shift meaning.

### `shot-classifier` — the classifier crate

A separate workspace member, so it can be exercised without CSP.

| Module | Surface | Job |
|---|---|---|
| `lib` | `ShotClassification`, `classify_instance` | runs both passes for one instance |
| `gate` | `EdgeProximity`, `gate` | pass 1: cheap border-band scan, per edge |
| `refine` | `RefinementInput`, `RefineParams`, `EdgeRefinement`, `refine_edge` | pass 2: trimap + colour matting at full resolution |
| `geometry` | `Edge`, `Grid`, `EdgeView`, `Rect` | edge-relative coordinates |
| `segmentation` | `Mask`, `Instance`, `SegmentationModel` | model-agnostic types |
| `sampling` | `ColorStats`, `sample_background_color` | per-edge background colour |
| `birefnet` | `BiRefNetConfig`, `BiRefNetModel` | ONNX inference (feature `birefnet`) |

`EdgeView` is the load-bearing idea: every per-edge algorithm is written once in "top edge, depth
increasing downward" terms against the `Grid` trait, and the other three edges are presented by
coordinate transform. Nothing edge-specific lives outside `geometry`.

---

## 3. Routing

Three routes, keyed on the edge-intersection (EIX) verdict:

| Route | Verdict | Strategy |
|---|---|---|
| **R1** `CenterAndStretch` | EIX `0000` | crop a square around the subject; background fills the rest |
| **R2** `CropSquare` | EIX set, non-zero | crop into real pixels, anchored on the edges touched |
| **R3** `Fallback` | EIX not set | fit the whole image into a square canvas, centred, white fill |

**Three routes, six behaviours.** `docs/csp-spec.md` §5's table is not a competing route list — it is
the *inside* of these routes. Its `0 edges` row is R1's strategy; its `1`, `2 opposite`,
`2 adjacent`, `3` and `4` rows are cases `crop_square` matches on internally as R2 grows. Choosing
among them is a behaviour, not a routing decision, so it never reaches `Route::select`.

R3 has no spec row. It is the answer to "we don't know": nothing cropped, nothing scaled, so a wrong
guess costs framing rather than pixels.

**Status:** R3 is implemented. R1 and R2 are stubs returning the image unchanged; each carries the
spec behaviours it owns in its module docstring.

---

## 4. Known gaps

- **The output size envelope is not enforced.** `MIN_SIZE` / `MAX_SIZE` / `resize_to_spec` were
  retired with the classical detector and have no replacement, so output is currently whatever the
  route produces. The spec's 800–2000px envelope and the ≤1.42× upscale cap need a home — either in
  each route or as a step between `dispatch` and `export`.
- R1 and R2 are stubs, so every classified image comes out unmodified.
- `birefnet` is off by default; without it nothing is classified and every image takes R3.

---

## 5. Diagram drift

`docs/diagrams/JBA2B.drawio.svg` is an end-to-end flowchart across four containers: **App**,
**Core** (Preprocessor → Shot Classifier → Processor → Exporter), and a proposed **CSP-Analyzer**.
Its App and Exporter containers still match the code. The rest has drifted:

**Preprocessor container**
- `Alpha channel present?` / `Use decoded RGB as-is (no alpha)` — alpha is now always flattened onto
  white and the channel is never carried forward. The branch is gone.
- Missing a node for the working-resolution downscale to `WORKING_SIZE`.

**Shot Classifier container** — every node is obsolete. It describes the classical detector:
`Alpha mask available?`, `Box = bounding extent of alpha > 8`, the low-contrast zone-stretch retry,
`Foreground fraction tiny AND border-ring texture high?`, `→ SalientSquare`,
`Box covers ≥ 98.5% of the frame?`, `Box touches ≥ 2 frame edges? (BLEED_EDGES)`, `→ WholeFrame`.
Replace with: segment the working copy → pass 1 gate per edge → pass 2 refine for gated edges →
`touches_edges`, or no verdict.

**Processor container** — obsolete. `Detection kind = SalientSquare?` and `Edge-intersect count = 0?`
become the three-way `Route::select`. The `MIN_SIZE / MAX_UPSCALE` growth and the `[800, 2000]`
resize nodes describe code that no longer exists (see Known gaps) — either mark them as pending or
remove them until the envelope is reimplemented.

**CSP-Analyzer container** — its target, `src/bin/csp_analyzer.rs`, was deleted. Either drop the
container or re-point it at `shot-classifier/examples/run_dir.rs`, which now fills that role.

These edits must be made through the draw.io editor (the VS Code **Draw.io Integration** extension),
not by hand-editing the embedded XML — the `content` attribute and the rendered SVG have to be
regenerated together or the picture and its source silently disagree. See
`.claude/skills/diagram-sync/SKILL.md`.
