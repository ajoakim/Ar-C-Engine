//! Engine-owned opaque resource handles.

/// Handle to a backend buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(u32);

/// Handle to a backend pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PipelineId(u32);

/// Handle to a backend texture (only surface-owned textures exist in P0).
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

    /// Raw numeric value.
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// A drawable mesh: a vertex buffer and the number of vertices to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshHandle {
    /// Buffer holding vertices.
    pub vertex_buffer: BufferId,
    /// Number of vertices to draw.
    pub vertex_count: u32,
}
