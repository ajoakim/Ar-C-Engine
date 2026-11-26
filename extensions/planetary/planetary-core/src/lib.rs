//! Planetary extension core (cube-sphere topology, terrain LOD). Nothing in the engine depends on it.
//!
//! Extension crate (tier 6). Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! Engine crates under crates/ must never depend on this crate.

pub mod face;
pub mod patch_id;

pub use face::{Edge, Face, FaceBasis};
pub use patch_id::{MAX_LEVEL, PatchId, PatchIdError};
