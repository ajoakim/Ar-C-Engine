//! The render backend boundary: trait, descriptors and frame packets.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::error::RenderError;
use crate::handles::{BufferId, MeshHandle, PipelineId, TextureId};

/// Anything that exposes raw-window-handle 0.6 window and display handles.
pub trait RenderTarget: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static {}
impl<T: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static> RenderTarget for T {}

/// Buffer usage.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferUsage {
    /// Vertex data.
    Vertex,
}

/// Buffer description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferDesc {
    /// Debug label.
    pub label: &'static str,
    /// Usage.
    pub usage: BufferUsage,
}

/// Built-in shaders.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderId {
    /// Unlit vertex-color shader.
    UnlitVertexColor,
}

/// Vertex layouts.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VertexLayout {
    /// Position (3xf32) followed by color (3xf32).
    PosColor,
}

impl VertexLayout {
    /// Bytes per vertex.
    pub fn stride(self) -> u64 {
        match self {
            VertexLayout::PosColor => 24,
        }
    }

    /// Byte offsets of each attribute.
    pub fn attribute_offsets(self) -> &'static [u64] {
        match self {
            VertexLayout::PosColor => &[0, 12],
        }
    }
}

/// Pipeline description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineDesc {
    /// Debug label.
    pub label: &'static str,
    /// Shader.
    pub shader: ShaderId,
    /// Vertex layout.
    pub vertex_layout: VertexLayout,
}

/// Vertex with position and color.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VertexPosColor {
    /// Position.
    pub position: [f32; 3],
    /// Linear RGB color.
    pub color: [f32; 3],
}

/// Encode vertices as 24 bytes each (position then color, little-endian f32).
///
/// GPU backends assume little-endian hosts.
pub fn encode_vertices(vertices: &[VertexPosColor]) -> Vec<u8> {
    let mut out = Vec::with_capacity(vertices.len() * 24);
    for v in vertices {
        for f in v.position.iter().chain(v.color.iter()) {
            out.extend_from_slice(&f.to_le_bytes());
        }
    }
    out
}

/// Depth clear value under reverse-Z (ADR-0001).
pub const REVERSE_Z_CLEAR_DEPTH: f32 = 0.0;
/// Maximum draws per frame.
pub const MAX_DRAWS_PER_FRAME: usize = 1024;
/// Maximum bytes in one buffer.
pub const MAX_BUFFER_BYTES: usize = 256 * 1024 * 1024;

/// One draw.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawCommand {
    /// Pipeline.
    pub pipeline: PipelineId,
    /// Mesh.
    pub mesh: MeshHandle,
    /// Model-view-projection matrix.
    pub mvp: glam::Mat4,
}

/// One render pass.
#[derive(Debug, Clone, PartialEq)]
pub struct PassPacket {
    /// Pass name.
    pub name: String,
    /// Color target.
    pub color_target: TextureId,
    /// Optional depth target.
    pub depth_target: Option<TextureId>,
    /// Clear color.
    pub clear_color: [f32; 4],
    /// Clear depth (must be 0.0, reverse-Z).
    pub clear_depth: f32,
    /// Draws.
    pub draws: Vec<DrawCommand>,
}

/// One frame of work.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FramePacket {
    /// Frame index.
    pub frame_index: u64,
    /// Passes in order.
    pub passes: Vec<PassPacket>,
}

/// Backend-agnostic rendering interface over engine-owned handles.
pub trait RenderBackend {
    /// Create a buffer with initial data.
    fn create_buffer(&mut self, desc: &BufferDesc, data: &[u8]) -> Result<BufferId, RenderError>;
    /// Destroy a buffer.
    fn destroy_buffer(&mut self, id: BufferId) -> Result<(), RenderError>;
    /// Create a pipeline.
    fn create_pipeline(&mut self, desc: &PipelineDesc) -> Result<PipelineId, RenderError>;
    /// Resize the surface (0 allowed for minimized windows).
    fn resize(&mut self, width: u32, height: u32);
    /// Current surface size.
    fn size(&self) -> (u32, u32);
    /// Validate and execute a frame packet.
    fn submit(&mut self, packet: &FramePacket) -> Result<(), RenderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_vertices_is_24_bytes_per_vertex() {
        let v = [
            VertexPosColor {
                position: [1.0, 2.0, 3.0],
                color: [0.0, 0.5, 1.0],
            },
            VertexPosColor::default(),
        ];
        let bytes = encode_vertices(&v);
        assert_eq!(bytes.len(), 48);
        assert_eq!(&bytes[0..4], &1.0f32.to_le_bytes());
        assert_eq!(&bytes[12..16], &0.0f32.to_le_bytes());
        assert_eq!(&bytes[16..20], &0.5f32.to_le_bytes());
    }

    #[test]
    fn vertex_layout_pos_color_stride_24() {
        assert_eq!(VertexLayout::PosColor.stride(), 24);
        assert_eq!(VertexLayout::PosColor.attribute_offsets(), &[0, 12]);
    }
}
