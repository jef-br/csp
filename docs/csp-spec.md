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

Six phases. The Shot Classifier is the new brain that everything else hangs off.

| Phase | Does | Status |
|---|---|---|
| **A · Preprocess + Shot Classifier (SC)** | Load, orient, flatten. Then analyse the *whole image* and decide the strategy per photo type. | `PLANNED` |
| **B · Detect** | Find the subject box (method chosen by SC). | `PARTIAL` |
| **C · Classify outcome** | Subject / salient-square / whole-frame. | `analyzers broken` |
| **D · Route** | Edge-touch pattern → crop strategy. | `analyzers broken` |
| **E · Crop & compose** | CoG/MSR square → background fill → resize. | `redesign` |
| **F · Save** | JPEG + ICC. | `BUILT` |

---

## 3 · Shot Classifier (SC) — the brain `PLANNED`

SC runs first and analyses the whole image (not just the background). It is cheap, so it does
the heavy analysis once and everyone downstream reuses it. Start small, grow it.

### 3.1 Owns / contains

- SLIC superpixels and the geodesic border prior (moved here from Detect — they're analysis, not detection).
- Global contrast analysis.
- Otsu-style bins for the dominant foreground and background colours.
- Histogram knee-normalisation (the post-CLAHE re-balance we discussed) to build a rebalanced *temp* image that aids binary-mask building. This is what used to be called the "rescue"; it's now just part of SC's standard analysis, so the rescue attempt is free when needed.

### 3.2 Behaves like a hysteresis

Analyse → run detection → compare point A (pre) vs point B (post) → conclude behaviour.
SC is allowed to "go back in time" and decide based on the difference, not a single snapshot.

### 3.3 Flat-background test `PLANNED`

Build a background-likeness binary image. If **one blob connects all four image edges and
encloses exactly one hole**, that's a clean background/subject split. Holes inside the hole
don't matter — a bbox ignores interior holes. Output is a **binary image** that can later be
combined with others via unions/intersections to make compound masks.

### 3.4 Triggers

SC is the same preprocessor that will trigger the Phase-2 human detector when the shot needs
it (people, head-to-toe framing). It also owns fixing the edge-intersection analyzers (§5)
that Detect's routing depends on.

> **Open:** exact SC output contract — which binary layers + scalars it hands downstream
> (bg-mask, subject-mask, contrast stats, dominant colours, saliency, human-signal).
> To be pinned as we build.

---

## 4 · Resolution & bbox policy `PLANNED`

- **Analysis size:** if the largest image dimension ≤ 1024 px, use the full image. If larger, resize to 1024 on the long side for analysis.
- **Bbox coordinates are always resolved at full resolution.** Analysis may run small for speed, but the final box edges are snapped on the full-size image. (This is the fix for the loose-box problem, P2.)

### 4.1 Edge-intersection detection `BROKEN today`

"Does the subject run off an image edge?" is decided per edge like this:

1. Cut a **4.2% ring** off every edge of the image.
2. Run edge detection on the ring (Canny, or a cross/gradient operator — whatever tests best).
3. If a lit edge-pixel **touches the image border**, that border is an intersected edge.

These four booleans feed the routing tree in §5. The tree logic is fine; the current
analyzers producing the booleans are what's failing.

---

## 5 · Detection outcomes & routing

Three outcomes: **Subject** (a real box), **Salient-square** (busy/detail shot → take the most
salient square), **Whole-frame** (no clean subject). Then routing keys on how many edges the
subject touches.

> **Data note.** Across 107 CiMini images today: 104 touch 0 edges, 1 touches 1, 1 touches 2,
> 0 touch 3, 1 touches 4. The 3-edge branch has no example. The tree isn't wrong — the
> analyzers feeding it (§4.1) are broken, so almost everything collapses to "0 edges".
> Fixing §4.1 is expected to redistribute this.

| edges | strategy |
|---|---|
| 0 | free-standing → CoG/MSR square (§6) |
| 1 | flush to that edge, centre the other axis |
| 2 opposite | fill that whole axis |
| 2 adjacent | flush into the shared corner |
| 3 | fill the boxed-in axis |
| 4 | fully bled → CoG/MSR square with extension (§6) |

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

| # | Symptom | Mechanism | Owner in target |
|---|---|---|---|
| P1 | Box creeps down into hand / pants / shadow | No shadow carve in the live (geodesic) path; connected low-contrast bleed | SC (contrast + shadow) & human detector |
| P2 | Box too loose | Detect at 480px + rescale padding | §4 full-res bbox |
| P3 | Flat shot under-detected, box cuts in | Geodesic approximates instead of subtracting a flat background | §3.3 flat-bg test |
| feet | Head-to-toe model loses feet (fb_02 #9) | Low-contrast feet vs border-connected floor → flood climbs in | human detector (Phase 2) |
| nonna | Real-life scene → whole-frame box → double stretch | No flat bg to isolate subject; geometry stretches to hit box+margin | §3 (isolate) + §6 (crop-in) |

---

## 8 · Decisions locked

- SC is the brain; owns SLIC + geodesic + contrast + Otsu bins + knee-normalisation; emits reusable binary layers. Start small.
- Rescue is not a separate stage — it's SC's standard contrast analysis.
- Analysis ≤1024; bbox always full-res; intersections via 4.2% ring + edge detection.
- Crop is CoG/MSR-anchored, real-pixels-first, ≤42% stretch.
- Edge-routing tree kept; its analyzers get fixed, not removed.
- Output 1:1 [800,2000], JPEG q90–95 4:4:4 sRGB.

---

## 9 · Open questions

- **Q1** · `LOSS_MAX` value (§6.4 ①).
- **Q2** · CoG definition — saliency-weighted centroid confirmed? (§6.4 ②)
- **Q3** · SC output contract — exact layers/scalars handed downstream (§3.4).
- **Q4** · SLIC patch count — likely reducible from 700; tune once SC's needs are known.
- **Q5** · Human detector interface — what signal it returns and how §6 consumes it (Phase 2).

---

## 10 · Build status

| Piece | State |
|---|---|
| Load / orient / flatten | `BUILT` |
| Shot Classifier | `PLANNED` |
| Flat-bg test | `PLANNED` |
| Full-res bbox + ring intersections | `BROKEN / PLANNED` |
| Shadow carve (geodesic path) | `LOST in migration` |
| CoG/MSR crop | `PLANNED` |
| Background fill / resize / save | `BUILT` |
| Human detector | `PHASE 2` |

---

*CSP source of truth · comment to change any line · I maintain it from your comments + chat*

*Source artifact: https://claude.ai/code/artifact/b3d70cd2-f493-4d01-992b-ace7709b5bc3*
