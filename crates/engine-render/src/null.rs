//! Headless backend that validates and records frames.

use std::collections::VecDeque;

use crate::backend::{BufferDesc, FramePacket, PipelineDesc, RenderBackend};
use crate::error::RenderError;
use crate::handles::{BufferId, PipelineId, TextureId};
use crate::material::TextureDesc;
use crate::resources::ResourceTable;

/// Headless backend for tests; keeps a bounded history of submitted frames.
#[derive(Debug)]
pub struct NullBackend {
    table: ResourceTable,
    frames: VecDeque<FramePacket>,
    submitted: u64,
    size: (u32, u32),
}

impl NullBackend {
    /// Maximum retained frames.
    pub const MAX_RECORDED_FRAMES: usize = 256;

    /// New backend with a 1280x720 surface.
    pub fn new() -> Self {
        Self::with_size(1280, 720)
    }

    /// New backend with the given surface size.
    pub fn with_size(w: u32, h: u32) -> Self {
        Self {
            table: ResourceTable::new(),
            frames: VecDeque::new(),
            submitted: 0,
            size: (w, h),
        }
    }

    /// Retained frames, oldest first.
    pub fn frames(&self) -> Vec<&FramePacket> {
        self.frames.iter().collect()
    }

    /// Most recent frame.
    pub fn last_frame(&self) -> Option<&FramePacket> {
        self.frames.back()
    }

    /// Total accepted submits.
    pub fn submitted_count(&self) -> u64 {
        self.submitted
    }

    /// Live buffers.
    pub fn buffer_count(&self) -> usize {
        self.table.buffer_count()
    }

    /// Live sampled textures.
    pub fn texture_count(&self) -> usize {
        self.table.texture_count()
    }

    /// Live pipelines.
    pub fn pipeline_count(&self) -> usize {
        self.table.pipeline_count()
    }
}

impl Default for NullBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderBackend for NullBackend {
    fn create_buffer(&mut self, desc: &BufferDesc, data: &[u8]) -> Result<BufferId, RenderError> {
        self.table.alloc_buffer(desc, data)
    }

    fn destroy_buffer(&mut self, id: BufferId) -> Result<(), RenderError> {
        self.table.remove_buffer(id).map(|_| ())
    }

    fn create_texture(
        &mut self,
        desc: &TextureDesc,
        rgba8: &[u8],
    ) -> Result<TextureId, RenderError> {
        self.table.alloc_texture(desc, rgba8)
    }

    fn destroy_texture(&mut self, id: TextureId) -> Result<(), RenderError> {
        self.table.remove_texture(id).map(|_| ())
    }

    fn create_pipeline(&mut self, desc: &PipelineDesc) -> Result<PipelineId, RenderError> {
        self.table.alloc_pipeline(desc)
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.size = (width, height);
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }

    fn submit(&mut self, packet: &FramePacket) -> Result<(), RenderError> {
        self.table.validate_packet(packet)?;
        tracing::trace!(
            frame = packet.frame_index,
            passes = packet.passes.len(),
            "null submit"
        );
        if self.frames.len() >= Self::MAX_RECORDED_FRAMES {
            self.frames.pop_front();
        }
        self.frames.push_back(packet.clone());
        self.submitted += 1;
        Ok(())
    }
}
