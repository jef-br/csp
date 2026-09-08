# CSP — Design Spec & Source of Truth

*living document · maintained by Claude from your comments + chat · v1 · 2026-08-26*

This is the single place the target CSP behaviour is written down. It supersedes the flow
diagrams for "what it should do". Status tags mark reality vs intent. Comment on any line;
I fold changes back in here.

**Legend:** `BUILT` · `BROKEN` · `PLANNED` · `CONFIRM`

---

## 1 · Principles & hard invariants

- **KPI:** repositioning accuracy at high speed.
- **Product is never distorted.** Only background is ever stretched.
- **Real pixels first.** Prefer cropping into the real image over inventing pixels; stretch only when a real-pixel square can't be had.
- **Max background stretch = 42%** on any side. Past that, it's the "impossible" case → fallback.
- **Output:** 1:1 square, side ∈ [800, 2000] px.
- **Encode:** JPEG q90–q95, 4:4:4 (no chroma subsampling), embedded sRGB ICC. `BUILT`
- **Failures** leave the input untouched (never deleted), never crash the batch.

---

## 2 · Pipeline overview

Five stages. The Shot Classifier decides which route an image takes; everything downstream hangs
off that one decision.

| Stage | Does | Status |
|---|---|---|
| **A · Load** | Decode, apply EXIF orientation, read the embedded ICC profile. | `BUILT` |
| **B · Preprocess** | Flatten alpha onto white, colour-manage to sRGB, derive the working-resolution copy. | `BUILT` |
| **C · Classify** | Segment the working copy, then decide per edge whether the subject reaches it. Emits the EIX verdict — or no verdict. | `BUILT` (behind the `birefnet` feature) |
| **D · Dispatch** | EIX verdict → one of three routes (§5). | `BUILT` |
| **E · Route** | R1/R2 shape the square; R3 frames it safely. | `R3 BUILT` · `R1/R2 STUB` |
| **F · Export** | Size envelope, then JPEG + ICC, then delete the source on success. | `BUILT` |

"Detect" and "classify outcome" are no longer separate phases. Segmentation produces the mask and
the edge verdict in one pass, so there is no intermediate box to find and then classify.

---

## 3 · Shot Classifier (SC) — the brain `BUILT`

SC runs on the working-resolution copy and answers one question: **which image edges does the
subject actually reach?** That verdict is what routing keys on (§5).

A bounding box cannot answer it reliably — a box can touch an edge while the silhouette does not,
and vice versa for a diagonal pose. So the answer comes from segmentation, not geometry.

### 3.1 How it works

BiRefNet (dichotomous image segmentation, ONNX) produces one foreground mask for the whole image —
no boxes, no classes, no NMS. The edge verdict is then taken in two passes:

- **Pass 1 · gate.** Scan only a `GATE_MARGIN_PX`-wide band along each edge of the mask. A hit means
  "worth checking precisely", *not* "touches": the mask is upsampled from the model's internal
  resolution and is not trusted for the verdict.
- **Pass 2 · refine.** For each gated edge, intersect the mask with the border band, crop that region
  at full resolution, and build a trimap — interior foreground, exterior background, a ring around
  the boundary marked unknown. Classify the unknown ring by colour distance to sampled foreground
  and background. The refined mask, not the raw mask, answers touching or not.

Pass 2 does real work at every scale. When the working copy equals the original there is no
upsampling error left to correct, but the colour matting on the unknown ring runs identically.

### 3.2 Output contract

`ShotClassification { touches_edges: Vec<Edge>, refinements: Vec<EdgeRefinement> }` — or **no
verdict**, when the model is unavailable or inference failed on that image.

Those are different states and they route differently (§5). This answers Q3.

### 3.3 What SC no longer owns

SLIC superpixels, the geodesic border prior, CLAHE, histogram knee-normalisation, Otsu bins, the
flat-background blob test and the pre/post hysteresis compare were the classical detector's
machinery. They were deleted with it. Segmentation replaces all of them — do not reintroduce them
piecemeal.

### 3.4 Triggers

SC remains the place a Phase-2 human detector would hook in (people, head-to-toe framing).

---

## 4 · Resolution & bbox policy `BUILT`

- **Working size:** if the largest dimension is ≤ `WORKING_SIZE` (1024 px), the image is used as-is;
  larger images are reduced to 1024 on the long side. Never upscaled — upscaling would invent detail
  the model then reads as real.
- **Boundaries are resolved at full resolution.** Segmentation runs on the working copy for speed,
  but pass 2 re-decides the boundary against full-resolution pixels. Both resolutions travel together
  as `Prepared { original, working }` for exactly this reason. (This is the fix for the loose-box
  problem, P2.)

### 4.1 Edge-intersection detection `BUILT`

Replaced. The 4.2% ring + Canny scheme is gone — it was the analyzer that collapsed 97% of images to
"touches no edge". Edge intersection is now the gate/refine result described in §3.1: a decision per
edge taken on the segmentation mask and confirmed at full resolution.

---

## 5 · Detection outcomes & routing `BUILT`

Three routes, keyed on the edge-intersection (EIX) verdict:

| Route | Verdict | Strategy |
|---|---|---|
| **R1** · Center & Stretch | EIX `0000` — reaches no edge | free-standing → CoG/MSR square (§6) |
| **R2** · CropSquare | EIX set and non-zero | crop into real pixels, anchored on the edges touched |
| **R3** · Fallback | EIX not set — no verdict | whole image, uncropped, centred on a square canvas |

**Three routes, six behaviours.** The table below is not a competing route list — it is the *inside*
of R1 and R2. Choosing among these is a behaviour, not a routing decision, so it never reaches route
selection.

| edges | behaviour | lives in |
|---|---|---|
| 0 | free-standing → CoG/MSR square (§6) | R1 |
| 1 | flush to that edge, centre the other axis | R2 |
| 2 opposite | fill that whole axis | R2 |
| 2 adjacent | flush into the shared corner | R2 |
| 3 | fill the boxed-in axis | R2 |
| 4 | fully bled → CoG/MSR square with extension (§6) | R2 |

R3 has no row here. It is the answer to "we don't know", and it exists because a model failure must
not be silently processed as a clean free-standing shot.

> **Data note.** The old analyzers collapsed 113 of 116 CiMini images to "0 edges". Re-measured over
> 112 images with segmentation: **84** touch 0 edges, **12** touch 1, **12** touch 2, **2** touch 3,
> **2** touch 4. The 3-edge branch, which previously had no example anywhere, now has two.

---

## 6 · Crop & compose — CoG / MSR square `REDESIGN`

Replaces the naive "box + margin, stretch if it overflows". Anchors the square on where the
salient mass actually sits, prefers real pixels, and only stretches background within the 42% cap.

### 6.1 Terms

- **MSR** — Most Salient Region: the bounding box of the thresholded saliency heatmap. `[mx,my,mw,mh]`.
- **CoG** — saliency-*weighted* centroid of the MSR, `(cx,cy)`. (Weighted, so a brighter sub-region pulls it — this is why the CoG can sit off the geometric centre.)
- **W,H** — full image size. **margin** = 4.2% of the square side.

### 6.2 Algorithm

```python
# 1. Can a real-pixel square hold the whole MSR?
S_need   = max(mw, mh) * (1 + margin)      # square big enough to keep all of MSR
S_real   = min(W, H)                       # largest square of real pixels

if S_need <= S_real:
    # crop in — no new pixels. Slide a S_need square to centre on CoG, clamp inside image.
    S = S_need;  extend = 0                 # nonna & ~all cases land here
else:
    # 2. A real square would clip the MSR. Only grow if it clips too much.
    S_clip = S_real
    keep   = msr_fraction_kept_by(square=S_clip centred on CoG)
    if keep >= (1 - LOSS_MAX):
        S = S_clip;  extend = 0             # acceptable clip, just crop in
    else:
        S = S_need                          # keep the whole MSR -> must add canvas
        extend = S - S_free                 # S_free = image size on the FREE axis

# 3. On the CONSTRAINED axis (where S <= image): slide the square to centre on CoG,
#    clamp to [0, dim-S].
# 4. On the FREE axis: split the needed `extend` by CoG bias.
p        = cog_fraction_on_free_axis        # 0 = at min edge, 1 = at max edge
ext_min  = round(p * extend)                # side the CoG is NEAR gets less new canvas
ext_max  = extend - ext_min                 # side the CoG faces gets more
crop_free_start = -ext_min                  # crop overhangs the image by ext_min / ext_max

# 5. FILL the overhangs by stretching the background band on each side toward the edge.
#    stretch_side = (band + ext_side) / band  must be <= 1.42,
#    else this is the impossible case.
```

### 6.3 Diagram

```
        ext_min                            ext_max
     (small — CoG                       (large — CoG
       is near)                          faces here)
    <------->                          <------------->
    +--------+---------------+----------------------+   dashed = target square
    |::::::::|  image  W x H |::::::::::::::::::::::|   shaded = new canvas,
    |::::::::|   +-------+   |::::::::::::::::::::::|   filled by stretching
    |::::::::|   |  MSR  |   |::::::::::::::::::::::|   background <= 42%
    |::::::::|   | (*)CoG|   |::::::::::::::::::::::|
    |::::::::|   +-------+   |::::::::::::::::::::::|
    +--------+---------------+----------------------+
```

### 6.4 Worked example `CONFIRM the numbers`

From your comment, re-run so it's internally consistent (rounding at 4.2% margin):

```
image      2000 x 3000  (portrait)          W=2000 H=3000
MSR        [x 300, y 560, w 800, h 2400]    mw=800 mh=2400
CoG        (cx, cy)  — weighted, left-biased on x
S_need   = 2400 * 1.042           = 2501    # round -> 2500
S_real   = min(2000,3000)         = 2000
2500 > 2000  and a 2000-square clips >LOSS_MAX of the MSR  -> grow & extend
free axis = X (width);  extend = 2500 - 2000 = 500 px  (25% of width)
p (CoG on X) ~ 0.21  ->  ext_min ~ 105 (left) , ext_max ~ 395 (right)
crop     = [x -105, y 500, 2500, 2500]      # y fits in real pixels; x overhangs 105 / 395
fill     left band ~400px stretches ~1.26x , right band ~800px stretches ~1.37x
         -> both <= 1.42  OK
```

> **CONFIRM:**
> 1. `LOSS_MAX` — you said **20%** in the rule but used **~10–17%** in the example. Pick one (default set to 20%).
> 2. The CoG number in your example (`x:200`) sits left of the MSR's own left edge (300); with a saliency-*weighted* CoG that's possible, but confirm it's weighted and not a slip.
> 3. margin applied to the square side (4.2%) vs to a single dimension — I used the square side.

### 6.5 Fill & resize `mostly BUILT`

- **Fill:** stretch only the background bands outward to the square; product band copied untouched. Grow anchored on the CoG of the MSR (not the geometric centre).
- **Resize:** uniform scale so the side lands in [800, 2000].

---

## 7 · Problem map

| # | Symptom | Original mechanism | Status |
|---|---|---|---|
| P1 | Box creeps down into hand / pants / shadow | No shadow carve in the live geodesic path; connected low-contrast bleed | Mechanism gone with the geodesic path. Segmentation does not carve shadow because it does not include it. Re-test rather than assume. |
| P2 | Box too loose | Detection at 480px, box rescaled up with the padding baked in | Addressed — §4: boundaries confirmed at full resolution |
| P3 | Flat shot under-detected, box cuts in | Geodesic approximates the background instead of subtracting it | Mechanism gone — no geodesic path |
| feet | Head-to-toe model loses feet (fb_02 #9) | Low-contrast feet vs border-connected floor → flood-fill climbs in | Open. The flood-fill that caused it is gone; needs re-testing against segmentation |
| nonna | Real-life scene → whole-frame box → double stretch | No flat background to isolate the subject; geometry stretched to hit box+margin | Open until R1 lands — §6's crop-in-first is the fix |

---

## 8 · Decisions locked

- SC is the brain, and it is segmentation-based: a BiRefNet mask plus a two-pass gate/refine edge
  verdict. The SLIC / geodesic / CLAHE / Otsu machinery is deleted, not paused.
- **"No verdict" is a first-class outcome**, distinct from "touches no edge". It routes to R3.
- Working size ≤1024; boundaries always confirmed at full resolution.
- Crop is CoG/MSR-anchored, real-pixels-first, ≤42% stretch. (§6, unchanged.)
- Three routes, six behaviours: the edge table lives *inside* R1 and R2, never above them.
- Output 1:1 [800,2000], JPEG q90–95 4:4:4 sRGB. The envelope is enforced in the exporter — which
  every route ends at — so no route can bypass it.
- Ships as one hardened exe with no install. Currently unmet: ONNX Runtime and the model sit beside
  the exe. A temporary shortfall, not a change of intent.

---

## 9 · Open questions

- **Q1** · `LOSS_MAX` value (§6.4 ①).
- **Q2** · CoG definition — saliency-weighted centroid confirmed? (§6.4 ②)
- **Q3** · ~~SC output contract~~ — **answered**, see §3.2.
- **Q4** · ~~SLIC patch count~~ — **moot**, SLIC is gone.
- **Q5** · Human detector interface — what signal it returns and how §6 consumes it (Phase 2).
- **Q6** · R2's five behaviours (§5) each need implementing, and each needs its own acceptance case.
- **Q7** · Small-source policy. The envelope upscales anything under 800px with no cap; the old
  whole-image 1.42× cap is gone. Decide whether small sources should be capped, padded, or refused.

---

## 10 · Build status

| Piece | State |
|---|---|
| Load / orient / flatten / sRGB | `BUILT` |
| Working-resolution copy | `BUILT` |
| Shot Classifier — BiRefNet + gate/refine | `BUILT` behind the `birefnet` feature |
| Edge intersections | `BUILT` |
| Route selection (R1/R2/R3) | `BUILT` |
| R3 · Fallback | `BUILT` |
| R1 · Center & Stretch | `STUB` |
| R2 · CropSquare | `STUB` |
| CoG/MSR crop (§6) | `PLANNED` — lands with R1 |
| Output size envelope | `BUILT` — in the exporter |
| Save JPEG + ICC | `BUILT` |
| Single hardened exe, no install | `NOT MET` — see `docs/ARCHITECTURE.md` §5 |
| Human detector | `PHASE 2` |

---

*CSP source of truth · comment to change any line · I maintain it from your comments + chat*

*Source artifact: https://claude.ai/code/artifact/b3d70cd2-f493-4d01-992b-ace7709b5bc3*
