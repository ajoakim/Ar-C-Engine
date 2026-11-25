//! Backend-agnostic render scene, extraction, render graph and backend trait. Contains no GPU API types.
//!
//! Tier 3 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

mod backend;
mod error;
mod handles;

pub use backend::{
    BufferDesc, BufferUsage, DrawCommand, FramePacket, MAX_BUFFER_BYTES, MAX_DRAWS_PER_FRAME,
    PassPacket, PipelineDesc, REVERSE_Z_CLEAR_DEPTH, RenderBackend, RenderTarget, ShaderId,
    VertexLayout, VertexPosColor, encode_vertices,
};
pub use error::RenderError;
pub use glam::{Mat4, Quat};
pub use handles::{BufferId, MeshHandle, PipelineId, TextureId};
