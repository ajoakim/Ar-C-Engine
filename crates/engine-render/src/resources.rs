//! Shared resource bookkeeping and packet validation used by every backend.

use std::collections::HashMap;

use crate::backend::{
    BufferDesc, BufferUsage, FramePacket, MAX_BUFFER_BYTES, MAX_DRAWS_PER_FRAME, PipelineDesc,
    REVERSE_Z_CLEAR_DEPTH,
};
use crate::error::RenderError;
use crate::handles::{BufferId, PipelineId, TextureId};

/// Recorded information about a live buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferInfo {
    /// Usage.
    pub usage: BufferUsage,
    /// Size in bytes.
    pub size: u64,
}

/// Tracks live handles. Ids are never reused, so stale ids stay invalid.
#[derive(Debug, Default)]
pub struct ResourceTable {
    next_buffer: u32,
    next_pipeline: u32,
    buffers: HashMap<BufferId, BufferInfo>,
    pipelines: HashMap<PipelineId, PipelineDesc>,
}

pub(crate) fn check_buffer_size(len: usize) -> Result<(), RenderError> {
    if len > MAX_BUFFER_BYTES {
        return Err(RenderError::BufferTooLarge {
            size: len,
            max: MAX_BUFFER_BYTES,
        });
    }
    Ok(())
}

impl ResourceTable {
    /// Empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a buffer, returning a fresh id.
    pub fn alloc_buffer(
        &mut self,
        desc: &BufferDesc,
        data: &[u8],
    ) -> Result<BufferId, RenderError> {
        if data.is_empty() {
            return Err(RenderError::EmptyBuffer);
        }
        check_buffer_size(data.len())?;
        let raw = self
            .next_buffer
            .checked_add(1)
            .ok_or(RenderError::HandleExhausted)?;
        self.next_buffer = raw;
        let id = BufferId::from_raw(raw);
        self.buffers.insert(
            id,
            BufferInfo {
                usage: desc.usage,
                size: data.len() as u64,
            },
        );
        Ok(id)
    }

    /// Remove a buffer.
    pub fn remove_buffer(&mut self, id: BufferId) -> Result<BufferInfo, RenderError> {
        self.buffers.remove(&id).ok_or(RenderError::InvalidHandle {
            kind: "buffer",
            id: id.raw(),
        })
    }

    /// Register a pipeline, returning a fresh id.
    pub fn alloc_pipeline(&mut self, desc: &PipelineDesc) -> Result<PipelineId, RenderError> {
        let raw = self
            .next_pipeline
            .checked_add(1)
            .ok_or(RenderError::HandleExhausted)?;
        self.next_pipeline = raw;
        let id = PipelineId::from_raw(raw);
        self.pipelines.insert(id, desc.clone());
        Ok(id)
    }

    /// Look up a buffer.
    pub fn buffer(&self, id: BufferId) -> Result<BufferInfo, RenderError> {
        self.buffers
            .get(&id)
            .copied()
            .ok_or(RenderError::InvalidHandle {
                kind: "buffer",
                id: id.raw(),
            })
    }

    /// Look up a pipeline.
    pub fn pipeline(&self, id: PipelineId) -> Result<&PipelineDesc, RenderError> {
        self.pipelines.get(&id).ok_or(RenderError::InvalidHandle {
            kind: "pipeline",
            id: id.raw(),
        })
    }

    /// Live buffer count.
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Live pipeline count.
    pub fn pipeline_count(&self) -> usize {
        self.pipelines.len()
    }

    /// Validate a frame packet against live resources and engine invariants.
    pub fn validate_packet(&self, packet: &FramePacket) -> Result<(), RenderError> {
        let mut total = 0usize;
        for pass in &packet.passes {
            if pass.color_target != TextureId::BACKBUFFER {
                return Err(RenderError::InvalidHandle {
                    kind: "texture",
                    id: pass.color_target.raw(),
                });
            }
            if let Some(d) = pass.depth_target
                && d != TextureId::DEPTH
            {
                return Err(RenderError::InvalidHandle {
                    kind: "texture",
                    id: d.raw(),
                });
            }
            if pass.clear_depth != REVERSE_Z_CLEAR_DEPTH {
                return Err(RenderError::InvalidDepthClear {
                    value: pass.clear_depth,
                });
            }
            if !pass.clear_color.iter().all(|c| c.is_finite()) {
                return Err(RenderError::NonFiniteTransform);
            }
            total = total.saturating_add(pass.draws.len());
            if total > MAX_DRAWS_PER_FRAME {
                return Err(RenderError::TooManyDraws {
                    count: total,
                    max: MAX_DRAWS_PER_FRAME,
                });
            }
            for d in &pass.draws {
                let pipe = self.pipeline(d.pipeline)?;
                let buf = self.buffer(d.mesh.vertex_buffer)?;
                if buf.usage != BufferUsage::Vertex {
                    return Err(RenderError::WrongBufferUsage {
                        id: d.mesh.vertex_buffer.raw(),
                    });
                }
                if d.mesh.vertex_count == 0 {
                    return Err(RenderError::EmptyMesh);
                }
                let required = u64::from(d.mesh.vertex_count) * pipe.vertex_layout.stride();
                if required > buf.size {
                    return Err(RenderError::MeshOutOfBounds {
                        buffer: d.mesh.vertex_buffer.raw(),
                        required,
                        size: buf.size,
                    });
                }
                if !d.mvp.is_finite() {
                    return Err(RenderError::NonFiniteTransform);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_size_limit_enforced() {
        assert!(check_buffer_size(MAX_BUFFER_BYTES).is_ok());
        assert!(matches!(
            check_buffer_size(MAX_BUFFER_BYTES + 1),
            Err(RenderError::BufferTooLarge { .. })
        ));
    }
}
