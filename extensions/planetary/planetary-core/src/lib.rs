//! Planetary extension core (cube-sphere topology, terrain LOD). Nothing in the engine depends on it.
//!
//! Extension crate (tier 6). Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! Engine crates under crates/ must never depend on this crate.

pub mod face;
pub mod patch_id;

pub use face::{Edge, Face, FaceBasis};
pub use patch_id::{MAX_LEVEL, PatchId, PatchIdError};
pub mod adjacency;
pub use adjacency::{Neighbor, neighbor, neighbor_children};
pub mod mapping;
pub use mapping::{
    equal_angle_t, face_of_dir, is_valid_cells_per_edge, lattice_dir, locate, patch_center_dir,
    patch_corner_dirs, vertex_dir,
};
pub mod lod;
pub mod terrain;
pub use terrain::{
    BASE_LAYER, BaseLayer, DETAIL_LAYER, DetailLayer, LAYER_VERSION, LayerSeeds, MASK_LAYER,
    MaskLayer, PlanetTerrain, TerrainError, TerrainParams, TerrainSample, base_at, compose,
    detail_at, mask_at,
};
