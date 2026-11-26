//! Planetary terrain streaming, GPU mesh lifecycle, effective LOD cut and planetary camera.
//!
//! Extension crate (tier 7). Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! Engine crates under crates/ must never depend on this crate.

pub mod camera;
pub use camera::{CameraParams, PlanetaryCamera, TAN_HALF_FOV_Y};
pub mod effective;
pub use effective::{EffectiveCut, effective_cut, naive_cut};
