//! egui-wgpu painter for the debug overlay (feature `egui-overlay`, off by default).
//!
//! The overlay is read-only: it only paints tessellated egui output. The backend stages one
//! [`OverlayData`] per frame with `WgpuBackend::set_overlay`; the next submitted frame records a
//! final `LoadOp::Load` pass over the scene colour target before it is presented.

/// Tessellated egui output for one frame (see ADR-0018).
#[derive(Default)]
pub struct OverlayData {
    /// Clipped primitives from `egui::Context::tessellate`.
    pub primitives: Vec<egui::ClippedPrimitive>,
    /// Texture uploads/frees the painter must apply (font atlas).
    pub textures_delta: egui::TexturesDelta,
    /// Pixels per egui point (1.0 for the engine overlay).
    pub pixels_per_point: f32,
}

impl OverlayData {
    /// True when there is nothing to paint and nothing to upload.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.primitives.is_empty() && self.textures_delta.is_empty()
    }
}

/// Painter state owned by the backend.
#[derive(Default)]
pub(crate) struct OverlayState {
    renderer: Option<egui_wgpu::Renderer>,
    /// Colour format the current `renderer` was built for.
    renderer_format: Option<wgpu::TextureFormat>,
    /// Set when the renderer was rebuilt for a new format; read once via [`Self::take_rebuilt`].
    rebuilt: bool,
    pending: Option<OverlayData>,
}

/// True when the painter must be (re)built for `now`: never built, or built for another format.
fn needs_rebuild(built_for: Option<wgpu::TextureFormat>, now: wgpu::TextureFormat) -> bool {
    built_for != Some(now)
}

impl OverlayState {
    /// Stage `data` for the next frame. A frame that was never rendered keeps its texture
    /// uploads: deltas merge, primitives are replaced.
    pub(crate) fn stage(&mut self, data: OverlayData) {
        if data.is_empty() && self.pending.is_none() {
            return;
        }
        match self.pending.as_mut() {
            Some(p) => {
                p.textures_delta.append(data.textures_delta);
                p.primitives = data.primitives;
                p.pixels_per_point = data.pixels_per_point;
            }
            None => self.pending = Some(data),
        }
    }

    /// True once after the painter was rebuilt for a new format (the first build is not counted).
    pub(crate) fn take_rebuilt(&mut self) -> bool {
        std::mem::take(&mut self.rebuilt)
    }

    /// Record the overlay pass into `encoder` over `view`. Returns extra command buffers that
    /// must be submitted before the encoder's.
    pub(crate) fn record(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(mut data) = self.pending.take() else {
            return Vec::new();
        };
        // A rebuilt renderer has no font atlas (it was uploaded to the old one), so the frame
        // that rebuilds applies its texture deltas but paints nothing; the host is told via
        // `take_rebuilt` and re-sends the atlas, so the next frame paints normally.
        let mut skip_paint = false;
        if needs_rebuild(self.renderer_format, format) {
            skip_paint = self.renderer.is_some();
            self.rebuilt |= skip_paint;
            self.renderer = Some(egui_wgpu::Renderer::new(
                device,
                format,
                egui_wgpu::RendererOptions {
                    msaa_samples: 1,
                    ..egui_wgpu::RendererOptions::default()
                },
            ));
            self.renderer_format = Some(format);
        }
        let renderer = self
            .renderer
            .as_mut()
            .expect("overlay renderer was just built");
        for (id, deltas) in data.textures_delta.set.drain() {
            for delta in &deltas {
                renderer.update_texture(device, queue, id, delta);
            }
        }
        let mut extra = Vec::new();
        if !skip_paint && !data.primitives.is_empty() && size.0 > 0 && size.1 > 0 {
            let screen = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [size.0, size.1],
                pixels_per_point: if data.pixels_per_point.is_finite()
                    && data.pixels_per_point > 0.0
                {
                    data.pixels_per_point
                } else {
                    1.0
                },
            };
            extra = renderer.update_buffers(device, queue, encoder, &data.primitives, &screen);
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui-overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut pass = pass.forget_lifetime();
            renderer.render(&mut pass, &data.primitives, &screen);
        }
        for id in data.textures_delta.free.drain() {
            renderer.free_texture(&id);
        }
        extra
    }
}

impl Drop for OverlayState {
    fn drop(&mut self) {
        // egui panics when a TexturesDelta is dropped unapplied; a never-rendered frame is
        // intentionally discarded here.
        if let Some(p) = self.pending.as_mut() {
            p.textures_delta.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_rebuild_table() {
        let a = wgpu::TextureFormat::Rgba8UnormSrgb;
        let b = wgpu::TextureFormat::Bgra8UnormSrgb;
        assert!(needs_rebuild(None, a));
        assert!(needs_rebuild(Some(a), b));
        assert!(!needs_rebuild(Some(a), a));
    }
}
