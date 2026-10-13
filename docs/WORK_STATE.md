# CSP work state

Read this first. Keep it short. Update it at the end of each work session.
Last updated: 2026-10-13.

## Decided
* The 384 int8 model stays. 512 and 448 are out of scope.
* The preprocessor ends with one function, `preprocessor::optimizer::optimize`. It does two steps, and
  its output is the only image BiRefNet sees (`Prepared::model_input`, cut from `working` at `Prepared::crop`):
  * 1: colour balance (Levels, once per image).
  * 2: trim, spec'd by the user on 2026-10-13: 16 border samples (4 per edge at 0/25/50/75%) give the
    backdrop colour range (min/max per channel); gaps closed with a 2x2 window (`TRIM_CLOSE_K`);
    box around the product; squared to its largest side; margin = larger of 10 px and 10% of that
    side; the square is centred on the product and slid inside the frame (margin squeezed if the frame
    is too narrow; if the product is longer than the frame's short side the margin drops to 10 px and
    the crop is cut at the frame, not square); crop.
  * Nothing but backdrop left: `optimize` returns `Err`; the file stays in INPUT and counts as failed.
  * The model's mask is pasted back into a full working-size canvas by `CroppedModel`
    (`shot_classifier::segmentation`), so the gate, refinement and routes are unchanged.
* `working` and `original` stay true colour. BGC/FGC are sampled from `working`.
* The 5% rule (`MASK_MIN_COVERAGE`) is the user's call. Never change it unasked. It is the only 5% rule.
* No retries, no guards, no fallbacks anywhere in the trim. Add none without asking.
* Goal now: improve results by refining the preprocessor, not by fine-tuning the model.

## Rejected
* Raw-colour retry (adds another path).
* Blind zoom-to-65% retry.
* Recalibrating the int8 model (384 fp32 is no better).

## Parked: the first trim (code in commit `cc525bd`, not in the tree)
Replaced on 2026-10-13 by the trim above. Its extra rules (widen on a cut edge, keep step 1's box,
whole-frame guard, whole-frame retry, tolerance numbers) were not asked for and are not in the code.

## Open
* Eight images where the model finds nothing: 847_det0, 579_det4, 579_det5, 648_det5, 653_det4,
  759_det0, 759_det1, 812_det5. All-or-nothing, not a near miss. Findings for 579_det5 (scratch tools only):
  * Encoder and bottleneck look the same whether the model finds the garment or not. The runs split in
    the decoder (D3 to D1). int8 is already 20-30% off fp32 in the encoder.
  * fp32 finds it on the raw trimmed crop only; int8 never. Not proven: strong stripe contrast
    (the optimiser deepens it) stops the decoder treating the garment as one object.
* First run of the new trim on the 9 images in `JB core img batch` (old vs new exe, scratch): 8 give the
  same edge tags and colours. IMG_2619_indigo loses its mask (25.6% -> 0%): the crop (327x919 of
  672x1000) is found empty by the model, so BGC/FGC drop out; EIX stays 0000. On 99218793_det1 a soft
  gradient backdrop falls outside the sampled colour range, so the crop stays wide (836x965 of 1000x1000).
* Idea for later (user, 2026-10-13): BiRefNet applies ImageNet normalisation itself, so the image gets
  normalised twice (colour balance, then ImageNet). Possible last-resort counter-move. Not started.
* Next experiment, scratch only, on 102454579_det5.jpg: change one input at a time (blend raw/optimised,
  crop margin, light blur, backdrop brightness) and read int8's best score.

## State of the repo
* Branch `refined-single-384-model`. The trim removal and the new trim are not committed.
* `models/birefnet_lite_384.onnx` (fp32, gitignored) is a leftover test file.
