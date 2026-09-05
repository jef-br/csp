# CLAUDE.md
Project notes for any Claude session working in this repo.

## Diagrams
* `docs\diagrams\JBA2B.drawio.svg` is a drawio diagram with which to communicate code structure and decisions.
  Access it via the diagram-sync skill.


## Maintenance rule
Still applies, now against the draw.io files: one node per module, one node per `struct`/`enum`.
Add a module or a public function → patch the node in the same commit.
A node with no matching file means the diagram drifted.