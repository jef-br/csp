# Diagrams

WYSIWYG diagrams for CSP, edited as draw.io (diagrams.net) files — free, no account,
no server.

## One-time setup

VS Code extension recommendation lives in `.vscode/extensions.json` (gitignored like the
rest of `.vscode/`, so it's local-only — open this folder in VS Code and accept the
"install recommended extensions" prompt, or install manually: search **Draw.io
Integration** (`hediet.vscode-drawio`) in the Extensions view.

## Creating a diagram

`Ctrl+Shift+P` → **Draw.io: New Diagram** → save it here as `<name>.drawio.svg` (not
plain `.drawio`). That extension matters:

- It's a real SVG, so GitHub renders it as an image in the repo view and PR diffs —
  nothing extra needed on GitHub's side.
- The full diagram source is embedded in the SVG's own metadata, so reopening the same
  file in VS Code drops you back into the drag-and-drop editor. One file, no export step.

## Editing

Open the `.drawio.svg` file in VS Code — it opens straight into the draw.io canvas.
Drag shapes, draw connectors, resize, recolor, same as the web app.

## Grouping — use containers, not just proximity

For things that belong together conceptually, use an actual **container** shape (draw a
rectangle, tick "Container" in the format panel) rather than just placing shapes near
each other. Dropping a shape into a container re-parents it in the XML
(`parent="<container id>"`), so the grouping is explicit in the file, not just implied
by pixel position. Give the container a title and it reads as a labeled group visually
too.

## Where this sits relative to the existing Mermaid diagrams

`docs/ARCHITECTURE.md` already carries four Mermaid diagrams (class map, pipeline flow,
detection internals, geometry routing) with a maintenance rule tying nodes to actual
files — that stays as-is for now. Mermaid is still the right tool when the diagram is
simple enough that auto-layout from text is fine and you want it inline with the prose
it documents.

Reach for a `.drawio.svg` here instead when manual layout actually matters — spatial
grouping via containers, non-tree layouts, anything fiddly enough that hand-placing
beats fighting Mermaid's auto-layout. Migrating any of the four existing diagrams to
draw.io is a separate decision, not done as part of this setup.

## Naming

kebab-case, one diagram per concern, e.g. `pipeline-overview.drawio.svg`.

## Asking Claude to look at a diagram

Point Claude at the file path (or attach it). It reads the embedded XML directly for
the exact node/edge/container structure, and can render the SVG to a PNG when it needs
to check the visual layout too — same file, no separate format needed for either side.

## CSP-Analyzer: tagging a diagram Rectangle and naming its debug image

`src/bin/csp_analyzer.rs` (a dev-only replay harness, see its own doc comment) writes one
intermediate image per internal decision point it replays. Each of those images is tied to
a specific Rectangle inside a Container on `docs/diagrams/JBA2B.drawio.svg` — the detailed
per-decision flowchart, not the four coarser `class-map`/`pipeline-flow`/
`detection-internals`/`geometry-routing` diagrams. Whenever a new snapshot moment is added:

1. Find that Rectangle in its Container on `JBA2B.drawio.svg` and prepend its text with a
   one-word, ≤15-char title, bright green (`#00CC00`) and bold. Edit **both** layers or the
   tag is incomplete: the compressed mxGraph source in the SVG's `content=` attribute (the
   real editable source — decode/edit/recompress it, don't just touch the visible SVG, or
   the tag is lost next time the file is reopened in the VS Code drawio editor) and the
   rendered SVG's duplicate text (the `<div>` inside `<foreignObject>` *and* the `<text>`
   fallback — GitHub's SVG sanitizer may not render `foreignObject`, so the fallback is what
   shows there). If no single existing rectangle covers the stage (branches merge or
   diverge), tag every terminal rectangle that could produce that snapshot with the same
   tag rather than inventing a new box.
2. Name the output file `<original stem>___<moduleindex>_<substep>_<TAG>.png`, so sorting
   filenames alphabetically reproduces real pipeline order:
   - `moduleindex` — one digit-word pair per Container, fixed regardless of what
     csp_analyzer currently snapshots: `0-app`, `1-core`, `2-prep` (Preprocessor),
     `3-class` (Shot Classifier), `4-proc` (Processor), `5-expo` (Exporter). The digit
     exists because alphabetizing the words themselves gets the order wrong (e.g. `class` <
     `prep` < `proc` alphabetically, but Preprocessor actually runs before Shot Classifier).
   - `substep` — a 2-digit number gapped by 10s (`10`, `20`, `30`, ...) reflecting real
     execution order *within* that Container. The gap leaves room to insert a step later
     (e.g. `15`) without renumbering its neighbors. Same reasoning as `moduleindex`: the
     TAG words won't reliably alphabetize in execution order on their own (`DETECT` <
     `SEGMASK` alphabetically, but the segmentation mask is actually computed first).
   - `TAG` — exactly the green-bold tag from step 1, verbatim, no extra prefix.
3. `csp_analyzer` never creates per-image subfolders — output mirrors input's structure
   exactly (flat directory in, flat directory out), so the filename alone (not folder
   placement) must carry both the pipeline order and the diagram traceability.
4. If the value the diagram points to isn't reachable from `src/bin/csp_analyzer.rs` today
   (a private module or function), add a small `pub fn debug_*` where the value is computed,
   then a thin wrapper in `src/core/shot_classifier/detect.rs` that handles the downscale/
   upscale dance (mirroring `debug_segment`, which already does exactly this for
   `segment::foreground_mask`) — this is the established pattern for every existing
   `debug_*` helper, so new ones should route through `detect.rs` the same way rather than
   exposing a new module as `pub`. This does touch existing core files; get an explicit OK
   first if that constraint hasn't already been relaxed for the session.

Current mapping (as of this writing): `2-prep_10_PREPROC`, `3-class_02_SLIC`,
`3-class_05_Geodesics`, `3-class_10_SEGMASK`, `3-class_20_DETECT`, `4-proc_10_FILL`,
`4-proc_20_FINAL`. Note `Geodesics` breaks the otherwise-all-caps tag style — it was authored
directly on the diagram rather than by this convention's usual all-caps pattern, and step 2 above
is "verbatim," so the code and filename keep that exact casing rather than normalizing it.
