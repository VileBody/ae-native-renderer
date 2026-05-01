//! Precomp evaluation skeleton.
//!
//! v0: render nested composition into offscreen canvas, then composite as a normal layer.
//! v1: cache precomp frames.
//! v2: implement controlled collapse transformations.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollapseMode {
    RasterizeFirst,
    CollapseSupportedVectors,
}
