# CLAUDE.md
Project notes for any Claude session working in this repo.

## Layout
One crate, one `src/` tree.
* `src/app/` — host shell: folder flow, batch, i18n.
* `src/core/` — the pipeline: `load → preprocess → classify → dispatch → export`,
  one module per stage. The shot classifier lives at `src/core/shot_classifier/`.
* `examples/`, `tools/` — the BiRefNet dev harness and the ONNX export script.

## Ships as one hardened exe, no install
A hard requirement, met — not optional, and never solved by putting a file beside the exe.
`cargo build --release` produces `csp.exe` and nothing else. The `.onnx` model
(`shot_classifier::birefnet`) and `onnxruntime.dll` (`core::runtime`) are gitignored, live at the
repo root, and are `include_bytes!`'d into the binary — a missing one breaks the *build*, not the
run. At startup the runtime is unpacked to a per-user cache file (`ort`'s loader needs a real file
to `LoadLibrary`) and the classifier session is built; if that fails the exe aborts before touching
any file (`core::preflight`) rather than silently taking the R3 fallback for the whole batch.
Real inference is always built in — no feature flag. `load-dynamic` stays only because no static
ONNX Runtime >= 1.22 exists for this target (pyke's is 1.20, short of the `DeformConv` op BiRefNet
needs). `docs/ARCHITECTURE.md` §5 has the detail and the clean finish.

## Diagrams
`docs/diagrams/JBA2B.drawio.svg` is the one remaining diagram — an end-to-end App/Core flowchart.
Reach it through the diagram-sync skill: you read the embedded mxGraph XML, not the picture.
Editing needs something that rewrites both layers together — the VS Code **Draw.io Integration**
extension, or a headless render through the drawio webapp that extension bundles. Hand-editing the
compressed blob desyncs source from image.

## Maintenance rule — live again
One node per module, one per `struct`/`enum`; add a module or public function → patch the node in
the same commit. It was suspended while `JBA2B.drawio.svg` still described the retired classical
detector; the diagram was rebuilt against the current code on 2026-09-09, so every stage has a node
to patch again. `docs/ARCHITECTURE.md` §6 says what each container covers.

## Docs
* `docs/ARCHITECTURE.md` — module map, pipeline stages, routing, known gaps.
* `docs/csp-spec.md` — the design spec. Its §5 table is six *behaviours*, not six routes:
  there are three routes, and §5's rows live inside R1 and R2.
