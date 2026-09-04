//! Shot classifier: decides which processor route an image takes. Starts small — today it only
//! tells a normal subject apart from the frame-filling salient-square fallback — and grows as more
//! shot types (packshot, detail shot, ghost, flatlay, ...) get their own criteria and routes.

pub mod detect;

mod clahe;
mod enhance;
mod imgmath;
mod imgutil;
mod saliency;
mod segment;
mod superpixel;

pub use detect::detect as classify;
