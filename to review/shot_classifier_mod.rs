//! Shot classifier: decides which processor route an image takes. Starts small — today it only
//! tells a normal subject apart from the frame-filling salient-square fallback — and grows as more
//! shot types (packshot, detail shot, ghost, flatlay, ...) get their own criteria and routes.

pub mod shotclassifier;

// detect.rs and its helpers (clahe, enhance, imgmath, imgutil, saliency, segment, superpixel)
// moved to `to review/` — unused by the new shotclassifier.rs entry point.
