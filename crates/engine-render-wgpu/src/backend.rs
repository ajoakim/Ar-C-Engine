//! `WgpuBackend`: the wgpu implementation of the engine `RenderBackend` trait.

use std::collections::HashMap;
use std::sync::Arc;

use engine_render::{
    BufferDesc, BufferId, FramePacket, PipelineDesc, PipelineId, RenderBackend, RenderError,
    RenderTarget, ResourceTable,
};
use wgpu::util::DeviceExt;

use crate::pipeline::{
    DEPTH_FORMAT, DRAW_UNIFORM_STRIDE, MVP_BYTES, create_bind_group_layout, create_render_pipeline,
    draw_uniform_offset, uniform_buffer_size,
};
use crate::target::Target;

fn backend_err(e: impl std::fmt::Display) -> RenderError {
    RenderError::Backend(e.to_string())
}

type TexView = (wgpu::Texture, wgpu::TextureView);

/// wgpu-backed renderer. Its public API exposes no wgpu types.
pub struct WgpuBackend {
    instance: wgpu::Instance,
    target: Option<Arc<dyn RenderTarget>>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    color_format: wgpu::TextureFormat,
    offscreen: Option<TexView>,
    depth: Option<TexView>,
    size: (u32, u32),
    table: ResourceTable,
    buffers: HashMap<BufferId, wgpu::Buffer>,
    pipelines: HashMap<PipelineId, wgpu::RenderPipeline>,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl WgpuBackend {
    /// Create a backend presenting to a window target.
    pub fn new(
        target: Arc<dyn RenderTarget>,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(Target(target.clone()))
            .map_err(backend_err)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(backend_err)?;
        let caps = surface.get_capabilities(&adapter);
        let color_format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| RenderError::Backend("surface reports no formats".into()))?;
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: color_format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            color_space: wgpu::SurfaceColorSpace::Auto,
            view_formats: vec![],
        };
        let mut backend = Self::with_device(
            instance,
            adapter,
            color_format,
            (width, height),
            Some(target),
            Some(surface),
            Some(config),
        )?;
        backend.reconfigure()?;
        tracing::info!(adapter = %backend.adapter_summary(), ?color_format, "wgpu backend ready");
        Ok(backend)
    }

    /// Create a backend rendering into an offscreen texture (tests, headless CI).
    pub fn new_offscreen(width: u32, height: u32) -> Result<Self, RenderError> {
        if width == 0 || height == 0 {
            return Err(RenderError::Backend(
                "offscreen size must be non-zero".into(),
            ));
        }
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(backend_err)?;
        let color_format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let mut backend = Self::with_device(
            instance,
            adapter,
            color_format,
            (width, height),
            None,
            None,
            None,
        )?;
        backend.reconfigure()?;
        tracing::info!(adapter = %backend.adapter_summary(), ?color_format, "wgpu offscreen backend ready");
        Ok(backend)
    }

    #[allow(clippy::too_many_arguments)]
    fn with_device(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        color_format: wgpu::TextureFormat,
        size: (u32, u32),
        target: Option<Arc<dyn RenderTarget>>,
        surface: Option<wgpu::Surface<'static>>,
        surface_config: Option<wgpu::SurfaceConfiguration>,
    ) -> Result<Self, RenderError> {
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(backend_err)?;
        if u64::from(device.limits().min_uniform_buffer_offset_alignment) > DRAW_UNIFORM_STRIDE {
            return Err(RenderError::Backend(
                "device uniform offset alignment exceeds draw uniform stride".into(),
            ));
        }
        let bind_group_layout = create_bind_group_layout(&device);
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("draw-uniforms"),
            size: uniform_buffer_size(),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw-uniform-group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(MVP_BYTES),
                }),
            }],
        });
        Ok(Self {
            instance,
            target,
            surface,
            surface_config,
            adapter,
            device,
            queue,
            color_format,
            offscreen: None,
            depth: None,
            size,
            table: ResourceTable::new(),
            buffers: HashMap::new(),
            pipelines: HashMap::new(),
            bind_group_layout,
            uniform_buffer,
            bind_group,
        })
    }

    /// Human-readable adapter description: "name (backend)".
    pub fn adapter_summary(&self) -> String {
        let info = self.adapter.get_info();
        format!("{} ({:?})", info.name, info.backend)
    }

    fn make_texture(
        &self,
        label: &str,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> TexView {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: self.size.0,
                height: self.size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// (Re)create size-dependent resources. No-op for zero-sized targets.
    fn reconfigure(&mut self) -> Result<(), RenderError> {
        let (w, h) = self.size;
        if w == 0 || h == 0 {
            return Ok(());
        }
        if let (Some(surface), Some(config)) = (&self.surface, &mut self.surface_config) {
            config.width = w;
            config.height = h;
            surface.configure(&self.device, config);
        } else {
            self.offscreen = Some(self.make_texture(
                "offscreen-color",
                self.color_format,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            ));
        }
        self.depth = Some(self.make_texture(
            "depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        ));
        Ok(())
    }

    fn recreate_surface(&mut self) -> Result<(), RenderError> {
        if let Some(target) = &self.target {
            let surface = self
                .instance
                .create_surface(Target(target.clone()))
                .map_err(backend_err)?;
            self.surface = Some(surface);
            self.reconfigure()?;
        }
        Ok(())
    }

    fn render(
        &self,
        packet: &FramePacket,
        color_view: &wgpu::TextureView,
    ) -> Result<(), RenderError> {
        let mut draw_index = 0usize;
        // Upload all per-draw uniforms first (bounded by validate_packet).
        for pass in &packet.passes {
            for d in &pass.draws {
                let cols = d.mvp.to_cols_array();
                self.queue.write_buffer(
                    &self.uniform_buffer,
                    draw_uniform_offset(draw_index),
                    bytemuck::cast_slice(&cols),
                );
                draw_index += 1;
            }
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        let mut index = 0usize;
        for pass in &packet.passes {
            let depth_view = if pass.depth_target.is_some() {
                Some(
                    &self
                        .depth
                        .as_ref()
                        .ok_or_else(|| RenderError::Backend("depth target missing".into()))?
                        .1,
                )
            } else {
                None
            };
            let [r, g, b, a] = pass.clear_color;
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&pass.name),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(r),
                            g: f64::from(g),
                            b: f64::from(b),
                            a: f64::from(a),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: depth_view.map(|view| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(pass.clear_depth),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for d in &pass.draws {
                let pipeline = self
                    .pipelines
                    .get(&d.pipeline)
                    .ok_or_else(|| RenderError::Backend("pipeline missing".into()))?;
                let buffer = self
                    .buffers
                    .get(&d.mesh.vertex_buffer)
                    .ok_or_else(|| RenderError::Backend("buffer missing".into()))?;
                let offset = u32::try_from(draw_uniform_offset(index))
                    .map_err(|_| RenderError::Backend("uniform offset overflow".into()))?;
                rp.set_pipeline(pipeline);
                rp.set_bind_group(0, &self.bind_group, &[offset]);
                rp.set_vertex_buffer(0, buffer.slice(..));
                rp.draw(0..d.mesh.vertex_count, 0..1);
                index += 1;
            }
        }
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }
}

impl RenderBackend for WgpuBackend {
    fn create_buffer(&mut self, desc: &BufferDesc, data: &[u8]) -> Result<BufferId, RenderError> {
        let id = self.table.alloc_buffer(desc, data)?;
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(desc.label),
                contents: data,
                usage: wgpu::BufferUsages::VERTEX,
            });
        self.buffers.insert(id, buffer);
        Ok(id)
    }

    fn destroy_buffer(&mut self, id: BufferId) -> Result<(), RenderError> {
        self.table.remove_buffer(id)?;
        if let Some(buffer) = self.buffers.remove(&id) {
            buffer.destroy();
        }
        Ok(())
    }

    fn create_pipeline(&mut self, desc: &PipelineDesc) -> Result<PipelineId, RenderError> {
        // Build the GPU pipeline first so a failure leaves no table entry.
        let pipeline = create_render_pipeline(
            &self.device,
            desc,
            self.color_format,
            &self.bind_group_layout,
        )?;
        let id = self.table.alloc_pipeline(desc)?;
        self.pipelines.insert(id, pipeline);
        Ok(id)
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.size = (width, height);
        if width == 0 || height == 0 {
            return;
        }
        if let Err(e) = self.reconfigure() {
            tracing::warn!(error = %e, "wgpu resize failed");
        }
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }

    fn submit(&mut self, packet: &FramePacket) -> Result<(), RenderError> {
        self.table.validate_packet(packet)?;
        if self.size.0 == 0 || self.size.1 == 0 {
            return Ok(());
        }
        if self.surface.is_none() {
            let view = match &self.offscreen {
                Some((_, view)) => view.clone(),
                None => return Ok(()),
            };
            return self.render(packet, &view);
        }
        let acquired = match &self.surface {
            Some(surface) => surface.get_current_texture(),
            None => return Ok(()),
        };
        match acquired {
            wgpu::CurrentSurfaceTexture::Success(frame) => {
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                self.render(packet, &view)?;
                self.queue.present(frame);
                Ok(())
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                self.render(packet, &view)?;
                self.queue.present(frame);
                self.reconfigure()
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => Ok(()),
            wgpu::CurrentSurfaceTexture::Outdated => self.reconfigure(),
            wgpu::CurrentSurfaceTexture::Lost => self.recreate_surface(),
            wgpu::CurrentSurfaceTexture::Validation => {
                Err(RenderError::Backend("surface validation error".into()))
            }
        }
    }
}
