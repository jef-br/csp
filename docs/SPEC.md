# CSP Design Spec

CONFIRM needed on: (1) LOSS_MAX value — stated as 20% in the rule but the worked example implies ~10–17%; (2) whether CoG is confirmed saliency-weighted (the example's CoG x=200 sits left of the MSR's own left edge x=300, which is only possible if weighted); (3) whether the 4.2% margin applies to the square's side or to a single dimension.

Fill & resize [mostly BUILT]:
- Fill: stretch only background bands outward to the square; product band copied untouched; growth anchored on the MSR's CoG, not the geometric centre.
- Resize: uniform scale so the side lands in [800, 2000].

## Problem map
| # | Symptom | Mechanism | Owner in target design |
|---|---|---|---|
| P1 | Box creeps down into hand / pants / shadow | No shadow carve in the live (geodesic) detection path; connected low-contrast bleed | SC (contrast + shadow) & human detector |
| P2 | Box too loose | Detection runs at 480px + rescale padding | Resolution & bbox policy (full-res bbox) |
| P3 | Flat shot under-detected, box cuts into product | Geodesic approximates instead of cleanly subtracting a flat background | SC flat-background test |
| feet | Head-to-toe model loses feet (e.g. jumpsuit-on-wood-floor case) | Low-contrast feet against a border-connected floor -> flood climbs in | Human detector (Phase 2) |
| nonna | Real-life street-scene photo -> whole-frame box -> double-axis background stretch | No flat background to isolate the subject; old geometry stretched to hit box+margin | SC (isolate subject) + CoG/MSR crop (crop-in first) |

---

## All decisions made in this conversation

1. Rust, single statically-linked Windows exe, no install, no side-car files, no PRISM references anywhere in the binary.
2. Repo: github.com/jef-br/csp.
3. Detection analysis resolution: 1024 px cap (full image if smaller).
4. Repositioning/sizing spec: product never independently stretched; only background grows; final square uniformly scaled so its side lands in [800, 2000]; max whole-image upscale +42%.
5. Output encoding: JPEG q90–95 (not fixed at q95 — "not 100% but q90/95"), no chroma subsampling (4:4:4), embedded sRGB ICC profile, 4.2% margin.
6. Failed images are left in the input folder untouched, never deleted; successfully processed images are deleted from input after saving to output.
7. Human/face detection: classical, pure-Rust, built from scratch — no ONNX, no Haar XML, no external model asset.
8. Anti-reverse-engineering: pure-Rust hardening only (strip symbols, no PDB, fat LTO, 1 codegen unit, panic=abort, encrypted string literals). No external commercial packer.
9. The human detector is the "secret sauce": planned to be externalized (Phase 2, deferred) — fetched at runtime as a WASM module, signature-verified, run in-memory via a pure-Rust WASM runtime, with graceful degrade to geometry-only routing if the fetch/verify fails.
10. Segmentation approach: superpixel (SLIC) + geodesic border-connected background prior, replacing the earlier chroma-plane/texture detector. This is the currently live detection path.
11. Low-contrast rescue: bilateral denoise -> Otsu split into bright zone vs shadow -> per-zone mu +/- k*sigma stretch -> feathered blend back in. Detection-only, fallback-only (fires only on a total miss). Texture-based detection was explicitly evaluated and rejected as a primary signal (risk of reintroducing floor/background pollution, confirmed by the grey-on-grey shorts case).
12. Code review of the existing tool found and confirmed: the old chroma/texture/CLAHE/Canny detector (`build_foreground_mask` and its shadow-carve `suppress_shadow`) is compiled into the binary but never called by the live pipeline — only reachable from debug tooling. This is a real regression: shadow suppression was lost when the detector was swapped to the geodesic approach, and is the root cause of the "box creeps into shadow" problem (P1).
13. `run_pass` / `struct Pass` in detect.rs are fully dead code (unreachable from anywhere, including debug tools) — confirmed via build warnings.
14. The nonna test case (OMB-E180-BV_6.jpg, 1624x2080) was measured directly: current detection returns a box covering 93% of the frame (near-whole-frame), which routes to center-and-stretch and produces a double-axis background stretch. Root cause confirmed as whole-frame detection on a real-life (non-flat) background, not a margin-overflow geometry bug as first hypothesized.
15. Fix direction for the nonna case, agreed: prefer a real-pixel crop-in over background stretch whenever a real-pixel square is achievable; only stretch background when even the largest real-pixel square can't be used. This must not disturb the existing separate "tiny subject" background-grow behavior (which exists so a small product can still reach the 800px minimum output size within the 42% upscale cap) — the two are triggered by different conditions and must stay independent.
16. The edge-touch routing tree (0/1/2-opposite/2-adjacent/3/4 edges) is confirmed correct in its logic and is being kept, not removed. What's broken is the edge-intersection *analyzers* that feed it (measured: 104/107 test images register as 0-edge today, most likely due to analyzer failure rather than true geometry).
17. New edge-intersection method decided: cut a 4.2% border ring off each image edge, run edge detection (Canny or similar) on the ring, and treat a lit pixel touching the image border as evidence of an intersected edge.
18. New resolution policy decided: if the image's largest dimension is <=1024px, analyze at full size; otherwise resize to 1024px on the long side for analysis. Final bounding-box coordinates must always be resolved at full image resolution regardless of analysis resolution (this is the fix for the "loose box" problem, P2).
19. Shot Classifier (SC) approved as a new preprocessing stage — "start small." It is the single front-of-pipeline analysis brain, and is the same component that will trigger the Phase-2 human detector when a shot needs it.
20. SC will own/absorb: SLIC superpixel generation, the geodesic border prior, global contrast analysis, Otsu-style dominant fg/bg colour binning, and the histogram knee-normalization step. These move out of the detector and into SC.
21. The former "low-contrast rescue" step is reclassified: it is not a separate fallback stage but simply part of SC's standard (always-run) analysis, since the analysis is cheap. This gives the rescue "for free" whenever it's needed, and produces more reusable data along the way.
22. SC's behavior model is explicitly hysteresis-like: analyze, run detection, compare the before/after state, and draw a conclusion from the comparison rather than from a single snapshot.
23. Flat-background detection rule decided: build a binary background-likeness image; if exactly one blob connects all four image edges and encloses exactly one hole (interior holes/blobs within that hole don't matter for a bounding box), that is a confirmed flat background, and the hole is the subject. Output of this test is itself a binary layer, meant to be combined with other binary layers via union/intersection later.
24. The crop/compose stage is being redesigned around a saliency-weighted "Center of Gravity of the Most Salient Region" (CoG/MSR), replacing the older box+margin+stretch-if-overflow approach. Core intent, confirmed: prefer a real-pixel square that contains the whole MSR; if that's not achievable within an acceptable loss threshold, grow the canvas; split any needed canvas growth asymmetrically based on which side the CoG is biased toward (the side the CoG faces gets more new canvas, the side it's near gets less); fill added canvas by stretching background, capped at the existing 42% limit. The underlying mechanism/intent is confirmed correct by the user even though the first worked-example numbers had inconsistencies.
25. Decision on how to keep this design synchronized going forward: a single living markdown/HTML "spec" document is the source of truth, maintained by Claude from a combination of (a) artifact comments and (b) plain conversational instructions in chat. Inline text edits and sticky notes made directly in a separate "editable" HTML artifact copy do NOT reach Claude (browser-local only) and are not a reliable communication channel — comments and chat are.
26. Working-session norms established by explicit user correction: Claude must not ask multiple stacked questions across turns, must not produce large unstructured prose that doesn't converge on a decision, and must not attempt to unilaterally drive/decide the design direction — the user is the decision-maker and Claude's role is to support with analysis, options, and implementation, not to lead.
27. Diagram format correction: decision-tree branch labels must use explicit if/else framing naming the condition and destination (e.g. "if true -> / else v"), not bare "yes/no" against an ambiguously-worded question.

## Options still on the table / open questions

1. LOSS_MAX threshold for the CoG/MSR crop algorithm (the max acceptable fraction of the MSR that can be clipped by a real-pixel square before the canvas must be grown instead) — stated as 20% in the rule but the user's own worked example implies something closer to 10-17%. Needs a single confirmed value.
2. Whether the CoG is definitely a saliency-weighted centroid (as opposed to a plain geometric centroid) — needs confirmation, since the user's worked example only makes sense under a weighted definition.
3. Whether the 4.2% margin in the CoG/MSR algorithm applies to the final square's side length or to a single axis/dimension independently — needs to be pinned down.
4. Exact SC (Shot Classifier) output contract: which binary mask layers and which scalar values it hands downstream to Detect/Route/Crop (e.g. background mask, subject mask, contrast statistics, dominant colours, saliency map, human-presence signal) — not yet specified.
5. SLIC superpixel count: currently 700, suspected to be reducible, but decided to tune it only once SC's own requirements (which may also consume the superpixels for contrast binning) are known, rather than in isolation.
6. Human detector interface: what exact signal it returns (e.g. bounding region, confidence, head/body split) and how the crop/compose stage should consume that signal — deferred to Phase 2 work.
7. Best edge-detection operator for the 4.2%-ring edge-intersection method — Canny was suggested as a default but is explicitly framed as "or whatever is best suited," i.e. still an open implementation choice to be benchmarked.
8. Whether/when to bring back the externalized WASM-fetched "secret sauce" human detector work (Phase 2) and the anti-reverse-engineering hardening pass (Phase 3) — both were explicitly deferred pending the detection-accuracy redesign (SC, CoG/MSR crop, full-res bbox) being completed and validated first.
