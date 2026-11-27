//! wgpu render backend. The only crate allowed to depend on wgpu.
//!
//! Tier 4 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! No other crate may depend on wgpu.

mod backend;
mod pipeline;
mod present;
mod target;

pub use backend::WgpuBackend;
pub use pipeline::DRAW_UNIFORM_STRIDE;
pub use present::{
    FrameLatency, PresentConfig, PresentInfo, PresentModeSetting, PresentParseError, Resolved,
    resolve_present_mode,
};
