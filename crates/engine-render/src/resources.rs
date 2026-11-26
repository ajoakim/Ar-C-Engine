//! Shared resource bookkeeping and packet validation used by every backend.

use std::collections::HashMap;

use crate::backend::{
    BufferDesc, BufferUsage, FramePacket, MAX_BUFFER_BYTES, MAX_DRAWS_PER_FRAME, PipelineDesc,
    REVERSE_Z_CLEAR_DEPTH,
};
use crate::error::RenderError;
use crate::handles::{BufferId, PipelineId, TextureId};
use crate::material::{MAX_TEXTURE_DIMENSION, TextureDesc};

/// Recorded information about a live sampled texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureInfo {
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
}

/// Validate a texture description against its RGBA8 data length.
pub fn validate_texture_data(desc: &TextureDesc, len: usize) -> Result<(), RenderError> {
    if desc.width == 0 || desc.height == 0 {
        return Err(RenderError::InvalidTexture("texture has a zero dimension"));
    }
    if desc.width > MAX_TEXTURE_DIMENSION || desc.height > MAX_TEXTURE_DIMENSION {
        return Err(RenderError::InvalidTexture(
            "texture dimension exceeds 8192",
        ));
    }
    let expected = (desc.width as usize)
        .checked_mul(desc.height as usize)
        .and_then(|p| p.checked_mul(4))
        .ok_or(RenderError::InvalidTexture("texture size overflows"))?;
    if len != expected {
        return Err(RenderError::InvalidTexture(
            "texture data length must be width*height*4",
        ));
    }
    check_buffer_size(len)
}

/// Recorded information about a live buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferInfo {
    /// Usage.
    pub usage: BufferUsage,
    /// Size in bytes.
    pub size: u64,
    /// Largest u32 value for index buffers.
    pub max_index: Option<u32>,
}

/// Tracks live handles. Ids are never reused, so stale ids stay invalid.
#[derive(Debug, Default)]
pub struct ResourceTable {
    next_buffer: u32,
    next_pipeline: u32,
    next_texture: u32,
    textures: HashMap<TextureId, TextureInfo>,
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
        let max_index = if desc.usage == BufferUsage::Index {
            if !data.len().is_multiple_of(4) {
                return Err(RenderError::InvalidIndices(
                    "index data length must be a multiple of 4",
                ));
            }
            data.chunks_exact(4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .max()
        } else {
            None
        };
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
                max_index,
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

    /// Register a sampled texture, returning a fresh id (>= 3, never reused).
    pub fn alloc_texture(
        &mut self,
        desc: &TextureDesc,
        data: &[u8],
    ) -> Result<TextureId, RenderError> {
        validate_texture_data(desc, data.len())?;
        let raw = self
            .next_texture
            .max(TextureId::FIRST_SAMPLED_RAW - 1)
            .checked_add(1)
            .ok_or(RenderError::HandleExhausted)?;
        self.next_texture = raw;
        let id = TextureId::from_raw(raw);
        self.textures.insert(
            id,
            TextureInfo {
                width: desc.width,
                height: desc.height,
            },
        );
        Ok(id)
    }

    /// Remove a sampled texture.
    pub fn remove_texture(&mut self, id: TextureId) -> Result<TextureInfo, RenderError> {
        self.textures.remove(&id).ok_or(RenderError::InvalidHandle {
            kind: "texture",
            id: id.raw(),
        })
    }

    /// Look up a sampled texture.
    pub fn texture(&self, id: TextureId) -> Result<TextureInfo, RenderError> {
        self.textures
            .get(&id)
            .copied()
            .ok_or(RenderError::InvalidHandle {
                kind: "texture",
                id: id.raw(),
            })
    }

    /// Live sampled texture count.
    pub fn texture_count(&self) -> usize {
        self.textures.len()
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
        packet.light.validate()?;
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
                if pipe.vertex_layout != d.mesh.layout {
                    return Err(RenderError::LayoutMismatch {
                        pipeline: d.pipeline.raw(),
                    });
                }
                let required = u64::from(d.mesh.vertex_count) * d.mesh.layout.stride();
                if required > buf.size {
                    return Err(RenderError::MeshOutOfBounds {
                        buffer: d.mesh.vertex_buffer.raw(),
                        required,
                        size: buf.size,
                    });
                }
                if let Some(ix) = d.mesh.index {
                    let ib = self.buffer(ix.buffer)?;
                    if ib.usage != BufferUsage::Index {
                        return Err(RenderError::WrongBufferUsage {
                            id: ix.buffer.raw(),
                        });
                    }
                    if ix.count == 0 || !ix.count.is_multiple_of(3) {
                        return Err(RenderError::InvalidIndices(
                            "index count must be a positive multiple of 3",
                        ));
                    }
                    if ib.max_index.is_some_and(|m| m >= d.mesh.vertex_count) {
                        return Err(RenderError::InvalidIndices("index exceeds vertex count"));
                    }
                    let required = (u64::from(ix.first) + u64::from(ix.count)) * 4;
                    if required > ib.size {
                        return Err(RenderError::MeshOutOfBounds {
                            buffer: ix.buffer.raw(),
                            required,
                            size: ib.size,
                        });
                    }
                }
                if let Some(t) = d.texture {
                    if t.is_surface_target() {
                        return Err(RenderError::InvalidHandle {
                            kind: "texture",
                            id: t.raw(),
                        });
                    }
                    self.texture(t)?;
                }
                if !d.mvp.is_finite()
                    || !d.model.is_finite()
                    || !d.normal.is_finite()
                    || !d.base_color.iter().all(|c| c.is_finite())
                {
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
