//! Backend-agnostic render scene, extraction, render graph and backend trait. Contains no GPU API types.
//!
//! Tier 3 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

mod backend;
mod camera;
mod error;
mod graph;
mod handles;
mod material;
mod null;
mod resources;
mod scene;

pub use backend::{
    BufferDesc, BufferUsage, DrawCommand, FramePacket, MAX_BUFFER_BYTES, MAX_DRAWS_PER_FRAME,
    PassPacket, PipelineDesc, REVERSE_Z_CLEAR_DEPTH, RenderBackend, RenderTarget, ShaderId,
    VertexLayout, VertexPosColor, VertexPosNormalUv, encode_indices, encode_lit_vertices,
    encode_vertices, validate_indices,
};
pub use camera::Camera;
pub use error::RenderError;
pub use glam::{Mat3, Mat4, Quat};
pub use graph::{
    BACKBUFFER, CompiledGraph, DEPTH, GraphError, MAIN_PASS, PassDesc, PipelineSet, RenderGraph,
};
pub use handles::{BufferId, MeshHandle, MeshIndices, PipelineId, TextureId};
pub use material::{
    DirectionalLight, MAX_MATERIALS, MAX_TEXTURE_DIMENSION, Material, MaterialId, MaterialTable,
    TextureDesc,
};
pub use null::NullBackend;
pub use resources::{BufferInfo, ResourceTable, TextureInfo, validate_texture_data};
pub use scene::{DrawItem, RenderItem, RenderScene, extract};
