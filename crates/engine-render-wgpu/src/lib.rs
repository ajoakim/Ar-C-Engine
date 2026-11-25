//! wgpu render backend. The only crate allowed to depend on wgpu.
//!
//! Tier 4 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! No other crate may depend on wgpu.

mod backend;
mod pipeline;
mod target;

pub use backend::WgpuBackend;
pub use pipeline::DRAW_UNIFORM_STRIDE;
