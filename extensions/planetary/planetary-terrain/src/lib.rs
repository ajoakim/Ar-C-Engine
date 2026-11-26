//! Planetary terrain streaming, GPU mesh lifecycle, effective LOD cut and planetary camera.
//!
//! Extension crate (tier 7). Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! Engine crates under crates/ must never depend on this crate.

// The contract mandates `SdkError` (a large type) as the error of fallible constructors.
#![allow(clippy::result_large_err)]

pub mod camera;
pub use camera::{CameraParams, PlanetaryCamera, TAN_HALF_FOV_Y};
pub mod effective;
pub use effective::{EffectiveCut, effective_cut, naive_cut};
pub mod requests;
pub use requests::{plan_requests, priority_for_level, stale_requests, wanted_closure};
pub mod key;
pub use key::PatchKey;
pub mod palette;
pub use palette::{PALETTE_H, PALETTE_W, palette_rgba8};
pub mod source;
pub use source::PatchSource;
pub mod uploader;
pub use uploader::{GpuUploader, HeldPatch};
pub mod system;
pub use system::{TerrainConfig, TerrainReport, TerrainSystem};

use engine_sdk::streaming::AssetClass;

/// Default world seed for the demo planet.
pub const DEMO_SEED: u64 = 0x0007_5EED_2026_0001;
/// Asset class of the always-wanted coarse levels.
pub const BASE_CLASS: AssetClass = AssetClass(0);
/// Asset class of detail levels.
pub const DETAIL_CLASS: AssetClass = AssetClass(1);
