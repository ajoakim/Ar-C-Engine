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
pub mod cell_payload;
pub mod hex;
pub mod lod;
pub mod mesh;
pub mod morph;
pub use cell_payload::{
    CELL_HEADER_LEN, CELL_PAYLOAD_MAGIC, CELL_PAYLOAD_VERSION, CELL_VERTEX_BYTES, PayloadError,
    decode_cell_payload, encode_cell_payload, encode_cell_payload_v3,
};
pub use mesh::{
    PatchMesh, SKIRT_MARGIN, border_positions_f64, build_patch, build_patch_cancellable,
    grid_indices, index_count, skirt_depth, vertex_count, vertex_position_f64,
};
pub mod topology;
pub use hex::HexSphere;
pub use topology::{CellMesh, IndexVariant, Piece, Topology, cube::CubeSphere};
pub mod terrain;
pub use terrain::{
    BASE_LAYER, BaseLayer, DETAIL_LAYER, DetailLayer, LAYER_VERSION, LayerSeeds, MASK_LAYER,
    MaskLayer, PlanetTerrain, TerrainError, TerrainParams, TerrainSample, base_at, compose,
    detail_at, mask_at,
};
