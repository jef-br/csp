# CLAUDE.md

Project notes for any Claude session working in this repo.

## Diagrams

CSP has moved off Mermaid onto **draw.io (diagrams.net)** for all diagrams — free,
WYSIWYG, real drag-and-drop node/connector editing, and it renders on GitHub as
`docs/diagrams/*.drawio.svg`. See `docs/diagrams/README.md` for the full convention
(file format, naming, containers for spatial grouping).

`docs/ARCHITECTURE.md`'s four diagrams are now `.drawio.svg` files embedded by image
reference, generated from the original Mermaid content (containers = former `namespace`
blocks; dashed/solid/diamond-ended lines = former `..>`/`-->`/`*--`):

- `docs/diagrams/class-map.drawio.svg`
- `docs/diagrams/pipeline-flow.drawio.svg`
- `docs/diagrams/detection-internals.drawio.svg`
- `docs/diagrams/geometry-routing.drawio.svg`

Layout was auto-generated (columns per namespace, straight-line edges) — functional and
accurate, not hand-tuned. Treat first opens as a cleanup pass: drag boxes/edges into
place as needed, the structure (nodes, edges, containers) is correct.

VS Code extension: **Draw.io Integration** — installed.
https://marketplace.visualstudio.com/items?itemName=hediet.vscode-drawio

When a Claude session is asked to look at a `.drawio.svg`: read the embedded mxGraph
XML directly for exact node/edge/container structure (containers show up as
`parent="<container id>"` on their children); render the SVG to PNG as well if the
visual layout itself needs checking, not just the structure.

## Open todos — abandoning Mermaid

1. Migrate or retire the Mermaid diagram in `MD/cspflow.md` (gitignored local notes,
   largely superseded by `docs/ARCHITECTURE.md`'s pipeline-flow / detection-internals
   diagrams) — fold into `docs/diagrams/` or drop it.

## Maintenance rule

Still applies, now against the draw.io files: one node per module, one node per
`struct`/`enum`. Add a module or a public function → patch the node in the same commit.
A node with no matching file means the diagram drifted.
