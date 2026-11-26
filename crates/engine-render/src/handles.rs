//! Engine-owned opaque resource handles.

use crate::backend::VertexLayout;

/// Handle to a backend buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(u32);

/// Handle to a backend pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PipelineId(u32);

/// Handle to a backend texture (surface targets 1 and 2, sampled textures from 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureId(u32);

impl BufferId {
    /// Raw numeric value.
    pub fn raw(self) -> u32 {
        self.0
    }
    pub(crate) fn from_raw(v: u32) -> Self {
        Self(v)
    }
}

impl PipelineId {
    /// Raw numeric value.
    pub fn raw(self) -> u32 {
        self.0
    }
    pub(crate) fn from_raw(v: u32) -> Self {
        Self(v)
    }
}

impl TextureId {
    /// Surface-owned color backbuffer.
    pub const BACKBUFFER: TextureId = TextureId(1);
    /// Surface-owned depth buffer.
    pub const DEPTH: TextureId = TextureId(2);
    /// First raw id handed out for sampled textures.
    pub const FIRST_SAMPLED_RAW: u32 = 3;

    /// Raw numeric value.
    pub fn raw(self) -> u32 {
        self.0
    }

    pub(crate) fn from_raw(v: u32) -> Self {
        Self(v)
    }

    /// True for the surface-owned targets (backbuffer, depth).
    pub fn is_surface_target(self) -> bool {
        self.0 < Self::FIRST_SAMPLED_RAW
    }
}

/// Index buffer reference of an indexed mesh (u32 indices).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshIndices {
    /// Buffer holding indices.
    pub buffer: BufferId,
    /// Number of indices to draw.
    pub count: u32,
}

/// A drawable mesh: a vertex buffer, its layout and an optional index buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshHandle {
    /// Vertex layout of the vertex buffer.
    pub layout: VertexLayout,
    /// Buffer holding vertices.
    pub vertex_buffer: BufferId,
    /// Number of vertices in the buffer.
    pub vertex_count: u32,
    /// Optional index buffer.
    pub index: Option<MeshIndices>,
}

impl MeshHandle {
    /// Non-indexed mesh.
    pub fn new(layout: VertexLayout, vertex_buffer: BufferId, vertex_count: u32) -> Self {
        Self {
            layout,
            vertex_buffer,
            vertex_count,
            index: None,
        }
    }

    /// Attach an index buffer.
    #[must_use]
    pub fn with_indices(mut self, buffer: BufferId, count: u32) -> Self {
        self.index = Some(MeshIndices { buffer, count });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_handle_constructors() {
        let b = BufferId::from_raw(1);
        let ib = BufferId::from_raw(2);
        let m = MeshHandle::new(VertexLayout::PosColor, b, 3);
        assert_eq!(m.index, None);
        let m = m.with_indices(ib, 6);
        assert_eq!(
            m.index,
            Some(MeshIndices {
                buffer: ib,
                count: 6
            })
        );
    }
}
