# CLAUDE.md
Project notes for any Claude session working in this repo.

## Layout
Cargo workspace, two members:
* `.` — the `csp` binary: app shell (folder flow, batch, i18n) + imaging core
  (`load → preprocess → classify → dispatch → export`).
* `shot-classifier/` — the BiRefNet shot classifier, usable on its own.

Real inference sits behind the `birefnet` feature, off by default. Without it nothing is
classified and every image takes the R3 fallback route. The `.onnx` model and `onnxruntime.dll`
are gitignored and live beside the executable, not in the repo.

## Ships as one hardened exe, no install
A hard requirement, currently unmet — the two files above sit beside the exe. The release profile
already carries the anti-RE hardening; the single-file half is what is missing. Do not quietly
redesign around the sidecar arrangement, and do not restate the requirement as optional; it is a
temporary state. `docs/ARCHITECTURE.md` §5 says what closing it takes.

## Diagrams
* `docs/diagrams/JBA2B.drawio.svg` is the one remaining draw.io diagram — an end-to-end
  App/Core flowchart. Access it via the diagram-sync skill, which explains why you read the
  embedded mxGraph XML rather than the rendered picture.
* Editing a `.drawio.svg` requires the VS Code **Draw.io Integration** extension. Hand-editing
  the compressed blob desyncs the source from the rendered image. With no editor available,
  read the diagram and write down what needs to change instead.

## Maintenance rule
One node per module, one node per `struct`/`enum`. Add a module or a public function → patch
the node in the same commit. A node with no matching file means the diagram drifted.

**Currently suspended.** The four diagrams this rule was written for (`class-map`,
`pipeline-flow`, `detection-internals`, `geometry-routing`) were deleted with the classical
detector, and `JBA2B.drawio.svg` has known drift of its own — so for the modules added during
the classifier rebuild there is no node to patch. `docs/ARCHITECTURE.md` §6 lists the
outstanding diagram edits node by node. Un-suspend this rule once those land.

## Docs
* `docs/ARCHITECTURE.md` — module map, pipeline stages, routing, known gaps.
* `docs/csp-spec.md` — the design spec. Its §5 table is six *behaviours*, not six routes:
  there are three routes, and §5's rows live inside R1 and R2.
