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
input folder with `rayon`; on success the source file is moved to the backup folder (`CSP-BACKUP`),
on failure it is left in place.

```
load ──▶ preprocess ──▶ classify ──▶ dispatch ──▶ export
```

| Stage | Module | In → Out |
|---|---|---|
| **load** | `core::preprocessor::load` | path → `Decoded` (image + ICC profile, EXIF applied) |
| **preprocess** | `core::preprocessor::preprocess` | `Decoded` → `Prepared` (sRGB, alpha flattened, + working copy) |
| **classify** | `core::classify` (private) | `Prepared` → `Option<(ShotClassification, Mask)>` |
| **dispatch** | `core::processor::routes::dispatch` | `Prepared` + verdict → `RgbImage` |
| **export** | `core::exporter::export` | `RgbImage` (+ optional `ShotInputs`) → size envelope → JPEG on disk, source moved to `CSP-BACKUP` |

`classify` carries the segmentation `Mask` out with the verdict; the exporter needs it for the
debug tags below.

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
| `app::batch` | `run(input, output, backup) -> Summary`, `Summary` | rayon fan-out, one level of subfolder recursion |
| `app::shell` | `run()` | Windows double-click flow: desktop `CSP-INPUT` → `CSP-OUTPUT`, originals → `CSP-BACKUP` |
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
| `core::exporter` | `export`, `ShotInputs`; `save::save_jpeg_srgb`, `icc::srgb_profile` |

The refine params live in `config` next to `WORKING_SIZE` because they are expressed in
working-resolution pixels — change one and the others shift meaning.

#### Debug tags (`CSP_DEBUG_TAGS` marker file)

Off by default — the shipped exe writes a clean `<stem>.jpg`. Drop a file named `CSP_DEBUG_TAGS`
next to the executable (content ignored, only presence matters — same sidecar convention
`core::sidecar` uses for `onnxruntime.dll`/the `.onnx` model) and the exporter instead names each
output from the shot classifier's verdict:

```
<stem>--EIX=<trbl>[--BGC=<hex>][--FGC=<hex>].jpg   the export, renamed
<stem>_segmask.png                                the working-resolution mask, 8-bit grey
```

`EIX` is the 4-bit top-right-bottom-left edge string — always the classifier's real verdict, the
same one `Route::select` routed on; `BGC`/`FGC` are the dominant background / weighted-median
foreground colours, each emitted only when sampleable. `=` stands in for the spec's `:` (illegal in
Windows filenames). All three are derived in `shot_classifier::shotcode`, shared with the `run_dir`
dev harness. With no verdict (a failed inference, or the sidecar files not found) there is no mask:
the name carries `EIX=____` and no segmask is written.

`shotcode::ShotCode` also carries `full_bleed`: a tiny mask with no uniform background (probable
full-bleed close-up). It is informational only and never overrides `eix` — a tag must never claim an
edge intersection the pipeline didn't actually route on. `full_bleed` doesn't appear in the filename
today; `run_dir`'s console output and `zzz_results.json` are where it's currently visible.

### `core::shot_classifier` — the classifier

One module per job. `SegmentationModel` keeps the gate/refine logic independent of the ONNX
binding, so it can also be exercised against a stand-in model in tests — real BiRefNet inference is
always built into the exe (no feature flag).

| Module | Surface | Job |
|---|---|---|
| `shot_classifier` | `ShotClassification`, `classify_instance` | runs both passes for one instance |
| `gate` | `EdgeProximity`, `gate` | pass 1: cheap border-band scan, per edge |
| `refine` | `RefinementInput`, `RefineParams`, `EdgeRefinement`, `refine_edge` | pass 2: trimap + colour matting at full resolution |
| `geometry` | `Edge`, `Grid`, `EdgeView`, `Rect` | edge-relative coordinates |
| `segmentation` | `Mask`, `Instance`, `SegmentationModel` | model-agnostic types |
| `sampling` | `ColorStats`, `sample_background_color` | per-edge background colour |
| `shotcode` | `ShotCode`, `derive`, `eix_bits`, `NO_VERDICT_EIX` | the `EIX`/`BGC`/`FGC` debug tag |
| `birefnet` | `BiRefNetConfig`, `BiRefNetModel` | ONNX inference, always built in |

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

- R1 and R2 are stubs, so a classified image is passed through and only the envelope resizes it.
- BiRefNet inference is always built in; `core::preflight` aborts at startup, before any file is
  touched, if the ONNX session can't be built (e.g. the embedded runtime can't be unpacked to any
  writable directory) — a batch never silently falls back to R3 for every image.

---

## 5. Single hardened exe, no install

**CSP ships as one hardened executable with no installation step. This is a hard requirement.**

Hardening is already in place in `Cargo.toml`'s release profile — fat LTO, one codegen unit,
symbols stripped, no PDB, abort on panic — and is anti-RE as much as size.

**Met.** `cargo build --release` produces one `csp.exe` (~195MB) and nothing else. Two things are
compiled in:

- **The model.** `shot_classifier::birefnet` embeds `birefnet_lite_512.onnx` with `include_bytes!`
  and loads it through `Session::commit_from_memory`.
- **The ONNX Runtime.** `core::runtime` embeds `onnxruntime.dll` the same way. `ort`'s
  `load-dynamic` needs a real path to `LoadLibrary`, so at first use the bytes are written once to
  `<%LOCALAPPDATA%|temp>/csp-ort-<tag>/onnxruntime.dll` and that path is handed to `ort::init_from`.
  The `<tag>` is the library's size plus a content hash: a rebuilt runtime lands in a fresh
  directory, repeat runs reuse the file already there, and the write is atomic (temp name then
  rename) so concurrent CSP processes don't corrupt it.

Both source files are gitignored and expected at the repo root — a missing one is a *build* failure
naming the file, not a runtime surprise. The only files CSP still touches beside the exe are
optional markers (`CSP_DEBUG_TAGS`) and the `ORT_DYLIB_PATH` override the `examples/` dev harness
uses to point at a hand-placed runtime.

### Why the runtime is unpacked rather than linked

Static linking was the first choice and it doesn't work here. pyke publishes a prebuilt *static*
ONNX Runtime for `x86_64-pc-windows-msvc` (`ortrs_static`, listed in `ort-sys`' `dist.txt`), but it
is **ORT 1.20.0**, and BiRefNet's decoder needs the native `DeformConv` op that only arrives in ORT
1.22:

```
Could not find an implementation for DeformConv(19) node with name 'node_deform_conv2d'
```

The session builds and then fails at load, on every image. Newer `ort` does not rescue it: from
`ort-sys` rc.13 the Windows distribution is Microsoft's own build (`ms@1.28.0`), a DLL — pyke
stopped shipping static Windows libraries. Re-exporting the model to avoid `DeformConv` is not the
way out either: without the native op it decomposes to `GatherND`, which allocates enormous
intermediates (see the `birefnet` module docs). So the runtime stays a DLL — it is just carried
*inside* the exe and unpacked on demand rather than shipped next to it.

### Linking it in later, if a static ORT >= 1.22 appears

Build ONNX Runtime >= 1.22 from source with CMake as a static library, point `ort-sys` at it with
`ORT_LIB_LOCATION`, and swap `load-dynamic` for the default linking. `ort` rc.9 requests API
version 20 and the ORT C API is forward-compatible, so a 1.22+ library serves it. `birefnet`'s
`init_runtime` and the whole of `core::runtime` then delete — nothing else in the codebase knows
the runtime is dynamic. This would drop the runtime unpack (a one-time ~14MB write on first run)
and shrink the exe slightly, but it is no longer on the critical path for the single-file
requirement.

### Note: the runtime DLL still needs the VC++ runtime

`onnxruntime.dll` imports `msvcp140.dll` / `vcruntime140*.dll`. Those are present on any
up-to-date Windows 11 and were already a requirement of the old sidecar layout, so this is not a
regression — but a truly bare target still needs the Visual C++ Redistributable.

---

## 6. Diagram drift

`docs/diagrams/JBA2B.drawio.svg` is an end-to-end flowchart across four containers: **App**,
**Core** (Preprocessor → Shot Classifier → Processor → Exporter), and a proposed **CSP-Analyzer**.
Its App container still matches the code. The rest has drifted:

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
become the three-way `Route::select`. The `MIN_SIZE / MAX_UPSCALE` growth node should be deleted
outright: `MAX_UPSCALE` was a whole-image upscale cap that no longer exists anywhere in the code and
is not coming back. The `[800, 2000]` resize node stays, but moves to the Exporter container — that
is where `resize::to_envelope` now runs.

Not to be confused with the spec's 42% background-stretch limit (`docs/csp-spec.md` §6), which is a
different rule that happens to share the number 1.42: it caps how far a background *band* may be
stretched during fill, and is live design for R1.

**Exporter container** — needs the `[800, 2000]` resize node moved in from the Processor container
(above), plus a node for the optional `CSP_DEBUG_TAGS` branch: rename the output to
`<stem>--EIX=…[--BGC=…][--FGC=…]` and write `<stem>_segmask.png` beside it, from
`shot_classifier::shotcode`. The classify step's output also changes from `Option<ShotClassification>`
to `Option<(ShotClassification, Mask)>` — the mask is threaded to the Exporter for that branch.

**CSP-Analyzer container** — its target, `src/bin/csp_analyzer.rs`, was deleted. Either drop the
container or re-point it at `examples/run_dir.rs`, which now fills that role.

These edits must be made through the draw.io editor (the VS Code **Draw.io Integration** extension),
not by hand-editing the embedded XML — the `content` attribute and the rendered SVG have to be
regenerated together or the picture and its source silently disagree. See
`.claude/skills/diagram-sync/SKILL.md`.
