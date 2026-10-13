# CSP work state

Read this first. Keep it short. Update it at the end of each work session.
Last updated: 2026-10-13.

## Decided
* The 384 int8 model stays. 512 and 448 are out of scope.
* Backdrop trim before BiRefNet, on every image:
  * 1: trim by the 4 corner colours.
  * 2: trim by the edge colours of that result.
  * 3: grow the box back by 10-20% (more when more was trimmed).
  * The mask is pasted back into the full frame.
  * How it works: `docs/ARCHITECTURE.md`, section "Backdrop trim".
* If the trimmed pass finds nothing, retry once on the whole frame.
* The 5% rule (`MASK_MIN_COVERAGE`) is the user's call. Never change it unasked.

## Rejected
* Raw-colour retry (adds another path).
* Blind zoom-to-65% retry.
* Recalibrating the int8 model (384 fp32 is no better).

## Open (waiting for the user)
* Rules added to the trim without being asked. Keep or remove?
  * Widen the crop when the mask touches a cut edge.
  * Keep step 1's box if step 2 removes everything.
  * Use the whole frame if the trim leaves under 2% of it, or a side under 24 px.
* Six images get a real mask on the trimmed pass (4.3-4.9% of the frame), then lose it:
  825_det2, 834_det1 to 834_det4, 840_det2.
  The 5% rule and the whole-frame retry throw it away. What should happen?
* Eight images where the model finds nothing: 847_det0, 579_det4, 579_det5, 648_det5, 653_det4, 759_det0, 759_det1, 812_det5.
  Root cause not known yet. The answer is all-or-nothing, not a near miss.

## State of the repo
* The trim is implemented but not committed.
  * New: `src/core/preprocessor/trim.rs`
  * Changed: `src/core/mod.rs`, `src/core/config.rs`, `src/core/trace.rs`, `src/core/preprocessor/mod.rs`
  * Docs: `docs/ARCHITECTURE.md`, `docs/diagrams/JBA2B.drawio.svg`
* `models/birefnet_lite_384.onnx` (fp32, gitignored) is a leftover test file.
