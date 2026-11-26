//! The render backend boundary: trait, descriptors and frame packets.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::error::RenderError;
use crate::handles::{BufferId, MeshHandle, PipelineId, TextureId};
use crate::material::{DirectionalLight, DistanceFog, TextureDesc};

/// Anything that exposes raw-window-handle 0.6 window and display handles.
pub trait RenderTarget: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static {}
impl<T: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static> RenderTarget for T {}

/// Buffer usage.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferUsage {
    /// Vertex data.
    Vertex,
    /// Index data (u32).
    Index,
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
    /// Lit, textured shader (directional light, base color texture).
    LitTextured,
}

/// Vertex layouts.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VertexLayout {
    /// Position (3xf32) followed by color (3xf32).
    PosColor,
    /// Position (3xf32), normal (3xf32), uv (2xf32).
    PosNormalUv,
    /// Position, normal, uv (as `PosNormalUv`) followed by a second position (3xf32) and a
    /// second uv (2xf32) that a per-draw blend factor interpolates toward.
    PosNormalUvMorph,
}

impl VertexLayout {
    /// Bytes per vertex.
    pub fn stride(self) -> u64 {
        match self {
            VertexLayout::PosColor => 24,
            VertexLayout::PosNormalUv => 32,
            VertexLayout::PosNormalUvMorph => 52,
        }
    }

    /// Byte offsets of each attribute.
    pub fn attribute_offsets(self) -> &'static [u64] {
        match self {
            VertexLayout::PosColor => &[0, 12],
            VertexLayout::PosNormalUv => &[0, 12, 24],
            VertexLayout::PosNormalUvMorph => &[0, 12, 24, 32, 44],
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

/// Vertex with position, normal and texture coordinates.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VertexPosNormalUv {
    /// Position.
    pub position: [f32; 3],
    /// Unit normal.
    pub normal: [f32; 3],
    /// Texture coordinates.
    pub uv: [f32; 2],
}

/// Encode lit vertices as 32 bytes each (position, normal, uv; little-endian f32).
pub fn encode_lit_vertices(vertices: &[VertexPosNormalUv]) -> Vec<u8> {
    let mut out = Vec::with_capacity(vertices.len() * 32);
    for v in vertices {
        for f in v.position.iter().chain(v.normal.iter()).chain(v.uv.iter()) {
            out.extend_from_slice(&f.to_le_bytes());
        }
    }
    out
}

/// Vertex with a second position and uv to blend toward.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VertexPosNormalUvMorph {
    /// Position at blend 0.
    pub position: [f32; 3],
    /// Unit normal (used at every blend value).
    pub normal: [f32; 3],
    /// Texture coordinates at blend 0.
    pub uv: [f32; 2],
    /// Position at blend 1.
    pub morph_position: [f32; 3],
    /// Texture coordinates at blend 1.
    pub morph_uv: [f32; 2],
}

/// Encode morph vertices as 52 bytes each (position, normal, uv, second position, second uv;
/// little-endian f32).
pub fn encode_lit_morph_vertices(vertices: &[VertexPosNormalUvMorph]) -> Vec<u8> {
    let mut out = Vec::with_capacity(vertices.len() * 52);
    for v in vertices {
        for f in v
            .position
            .iter()
            .chain(v.normal.iter())
            .chain(v.uv.iter())
            .chain(v.morph_position.iter())
            .chain(v.morph_uv.iter())
        {
            out.extend_from_slice(&f.to_le_bytes());
        }
    }
    out
}

/// Encode u32 indices as 4 little-endian bytes each.
pub fn encode_indices(indices: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(indices.len() * 4);
    for i in indices {
        out.extend_from_slice(&i.to_le_bytes());
    }
    out
}

/// Validate an index list against a vertex count.
pub fn validate_indices(indices: &[u32], vertex_count: u32) -> Result<(), RenderError> {
    if indices.is_empty() {
        return Err(RenderError::InvalidIndices("index list is empty"));
    }
    if !indices.len().is_multiple_of(3) {
        return Err(RenderError::InvalidIndices(
            "index count is not a multiple of 3",
        ));
    }
    if u32::try_from(indices.len()).is_err() {
        return Err(RenderError::InvalidIndices("too many indices"));
    }
    if indices.iter().any(|&i| i >= vertex_count) {
        return Err(RenderError::InvalidIndices("index out of range"));
    }
    Ok(())
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
    /// Camera-relative model matrix.
    pub model: glam::Mat4,
    /// Normal matrix (inverse-transpose of the model's 3x3).
    pub normal: glam::Mat3,
    /// Linear RGBA base color factor.
    pub base_color: [f32; 4],
    /// Optional sampled base color texture (None = backend default white).
    pub texture: Option<TextureId>,
    /// Blend factor in [0, 1] toward the second position and uv of a morph layout (0 = none).
    pub morph: f32,
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
    /// Directional light in world axes.
    pub light: DirectionalLight,
    /// Distance fog (density 0 disables).
    pub fog: DistanceFog,
    /// Passes in order.
    pub passes: Vec<PassPacket>,
}

/// Backend-agnostic rendering interface over engine-owned handles.
pub trait RenderBackend {
    /// Create a buffer with initial data.
    fn create_buffer(&mut self, desc: &BufferDesc, data: &[u8]) -> Result<BufferId, RenderError>;
    /// Destroy a buffer.
    fn destroy_buffer(&mut self, id: BufferId) -> Result<(), RenderError>;
    /// Create a sampled RGBA8 (sRGB) texture; `rgba8.len()` must equal width*height*4.
    fn create_texture(
        &mut self,
        desc: &TextureDesc,
        rgba8: &[u8],
    ) -> Result<TextureId, RenderError>;
    /// Destroy a sampled texture (surface targets cannot be destroyed).
    fn destroy_texture(&mut self, id: TextureId) -> Result<(), RenderError>;
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

    #[test]
    fn vertex_layout_pos_normal_uv_stride_32() {
        assert_eq!(VertexLayout::PosNormalUv.stride(), 32);
        assert_eq!(VertexLayout::PosNormalUv.attribute_offsets(), &[0, 12, 24]);
    }

    #[test]
    fn vertex_layout_morph_stride_52() {
        assert_eq!(VertexLayout::PosNormalUvMorph.stride(), 52);
        assert_eq!(
            VertexLayout::PosNormalUvMorph.attribute_offsets(),
            &[0, 12, 24, 32, 44]
        );
    }

    #[test]
    fn encode_lit_morph_vertices_52_bytes_in_order() {
        let v = [
            VertexPosNormalUvMorph {
                position: [1.0, 2.0, 3.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.25, 0.75],
                morph_position: [4.0, 5.0, 6.0],
                morph_uv: [0.5, 0.125],
            },
            VertexPosNormalUvMorph::default(),
        ];
        let b = encode_lit_morph_vertices(&v);
        assert_eq!(b.len(), 104);
        assert_eq!(&b[0..4], &1.0f32.to_le_bytes());
        assert_eq!(&b[24..28], &0.25f32.to_le_bytes());
        assert_eq!(&b[32..36], &4.0f32.to_le_bytes());
        assert_eq!(&b[40..44], &6.0f32.to_le_bytes());
        assert_eq!(&b[44..48], &0.5f32.to_le_bytes());
        assert_eq!(&b[48..52], &0.125f32.to_le_bytes());
    }

    #[test]
    fn encode_lit_vertices_32_bytes_each_and_encode_indices_le() {
        let v = [
            VertexPosNormalUv {
                position: [1.0, 2.0, 3.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.25, 0.75],
            },
            VertexPosNormalUv::default(),
        ];
        let bytes = encode_lit_vertices(&v);
        assert_eq!(bytes.len(), 64);
        assert_eq!(&bytes[24..28], &0.25f32.to_le_bytes());
        assert_eq!(&bytes[28..32], &0.75f32.to_le_bytes());
        let ib = encode_indices(&[1, 2, 3]);
        assert_eq!(ib.len(), 12);
        assert_eq!(&ib[0..4], &1u32.to_le_bytes());
        assert_eq!(&ib[8..12], &3u32.to_le_bytes());
    }

    #[test]
    fn validate_indices_cases() {
        assert!(matches!(
            validate_indices(&[], 3),
            Err(RenderError::InvalidIndices(_))
        ));
        assert!(matches!(
            validate_indices(&[0, 1], 3),
            Err(RenderError::InvalidIndices(_))
        ));
        assert!(matches!(
            validate_indices(&[0, 1, 3], 3),
            Err(RenderError::InvalidIndices(_))
        ));
        assert!(validate_indices(&[0, 1, 2], 3).is_ok());
    }
}
