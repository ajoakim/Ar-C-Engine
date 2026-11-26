//! Planetary terrain streaming, GPU mesh lifecycle, effective LOD cut and planetary camera.
//!
//! Extension crate (tier 7). Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! Engine crates under crates/ must never depend on this crate.

// The contract mandates `SdkError` (a large type) as the error of fallible constructors.
#![allow(clippy::result_large_err)]

pub mod camera;
pub use camera::{CameraParams, PlanetaryCamera, TAN_HALF_FOV_Y};
pub mod churn;
pub use churn::{ChurnCounts, ChurnFlags, ChurnTracker, count_overlaps};
pub mod effective;
pub use effective::{EffectiveCut, effective_cut, naive_cut};
pub mod requests;
pub use requests::{
    plan_requests, plan_requests_scored, priority_for_level, request_score, stale_requests,
    wanted_closure,
};
pub mod key;
pub use key::{CellKey, PatchKey};
pub mod palette;
pub use palette::{
    DEBUG_PALETTE_H, PALETTE_H, PALETTE_H_V2, PALETTE_W, debug_palette_rgba8, palette_rgba8,
    palette_rgba8_v2,
};
pub mod source;
pub use source::{CellSource, cell_payload_estimate};
pub mod uploader;
pub use uploader::{GpuUploader, HeldCell};
pub mod system;
pub use system::{
    BudgetLimits, DRAW_MARGIN, StreamBudget, TerrainConfig, TerrainReport, TerrainSystem,
};

use engine_sdk::streaming::AssetClass;

/// Default world seed for the demo planet.
pub const DEMO_SEED: u64 = 0x0007_5EED_2026_0001;
/// Asset class of the always-wanted coarse levels.
pub const BASE_CLASS: AssetClass = AssetClass(0);
/// Asset class of detail levels.
pub const DETAIL_CLASS: AssetClass = AssetClass(1);
