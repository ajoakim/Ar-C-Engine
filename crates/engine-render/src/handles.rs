//! Engine-owned opaque resource handles.

use crate::backend::VertexLayout;
use crate::error::RenderError;

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
    /// First index to draw.
    pub first: u32,
    /// Number of indices to draw.
    pub count: u32,
    /// True when the buffer is shared between meshes and owned by the app.
    pub shared: bool,
}

/// A static u32 index buffer that several meshes can draw ranges of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndexBufferHandle {
    /// Buffer holding indices.
    pub buffer: BufferId,
    /// Number of indices in the buffer.
    pub count: u32,
    /// Largest index value stored in the buffer.
    pub max_index: u32,
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
        self.index = Some(MeshIndices {
            buffer,
            first: 0,
            count,
            shared: false,
        });
        self
    }

    /// Attach a range of a shared index buffer.
    pub fn with_index_range(
        mut self,
        ib: &IndexBufferHandle,
        first: u32,
        count: u32,
    ) -> Result<Self, RenderError> {
        if count == 0 || !count.is_multiple_of(3) {
            return Err(RenderError::InvalidIndices(
                "index count must be a positive multiple of 3",
            ));
        }
        let end = u64::from(first) + u64::from(count);
        if end > u64::from(ib.count) {
            return Err(RenderError::InvalidIndices("index range out of bounds"));
        }
        if ib.max_index >= self.vertex_count {
            return Err(RenderError::InvalidIndices(
                "shared index buffer references vertices beyond this mesh",
            ));
        }
        self.index = Some(MeshIndices {
            buffer: ib.buffer,
            first,
            count,
            shared: true,
        });
        Ok(self)
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
                first: 0,
                count: 6,
                shared: false
            })
        );
    }
}
