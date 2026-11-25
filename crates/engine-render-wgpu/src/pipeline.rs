//! Pipeline construction helpers and fixed layout constants.

use engine_render::{MAX_DRAWS_PER_FRAME, PipelineDesc, RenderError, ShaderId, VertexLayout};

pub(crate) const SHADER_UNLIT_VERTEX_COLOR: &str = include_str!("shaders/unlit_vertex_color.wgsl");

/// Byte stride between per-draw uniform slots (at least wgpu's default dynamic offset alignment).
pub const DRAW_UNIFORM_STRIDE: u64 = 256;

/// Depth format used for all depth targets.
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Size of one mat4x4<f32> uniform.
pub(crate) const MVP_BYTES: u64 = 64;

pub(crate) fn draw_uniform_offset(index: usize) -> u64 {
    index as u64 * DRAW_UNIFORM_STRIDE
}

pub(crate) fn uniform_buffer_size() -> u64 {
    MAX_DRAWS_PER_FRAME as u64 * DRAW_UNIFORM_STRIDE
}

pub(crate) fn vertex_attributes(
    layout: VertexLayout,
) -> Result<(u64, [wgpu::VertexAttribute; 2]), RenderError> {
    match layout {
        VertexLayout::PosColor => {
            let offsets = layout.attribute_offsets();
            Ok((
                layout.stride(),
                [
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: offsets[0],
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: offsets[1],
                        shader_location: 1,
                    },
                ],
            ))
        }
        #[allow(unreachable_patterns)]
        _ => Err(RenderError::Backend("unsupported vertex layout".into())),
    }
}

/// Reverse-Z depth state: larger depth is nearer, so `Greater` passes.
pub(crate) fn depth_stencil_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::Greater),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

pub(crate) fn create_render_pipeline(
    device: &wgpu::Device,
    desc: &PipelineDesc,
    color_format: wgpu::TextureFormat,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> Result<wgpu::RenderPipeline, RenderError> {
    let source = match desc.shader {
        ShaderId::UnlitVertexColor => SHADER_UNLIT_VERTEX_COLOR,
        #[allow(unreachable_patterns)]
        _ => return Err(RenderError::Backend("unsupported shader".into())),
    };
    let (stride, attributes) = vertex_attributes(desc.vertex_layout)?;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(desc.label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(desc.label),
        bind_group_layouts: &[Some(bind_group_layout)],
        immediate_size: 0,
    });
    Ok(
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(desc.label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: stride,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil_state()),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        }),
    )
}

pub(crate) fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("draw-uniform-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(MVP_BYTES),
            },
            count: None,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_pos_color_attributes() {
        let (stride, attrs) = vertex_attributes(VertexLayout::PosColor).unwrap();
        assert_eq!(stride, 24);
        assert_eq!(attrs[0].shader_location, 0);
        assert_eq!(attrs[0].offset, 0);
        assert_eq!(attrs[0].format, wgpu::VertexFormat::Float32x3);
        assert_eq!(attrs[1].shader_location, 1);
        assert_eq!(attrs[1].offset, 12);
        assert_eq!(attrs[1].format, wgpu::VertexFormat::Float32x3);
    }

    #[test]
    fn pipeline_draw_uniform_offsets() {
        assert_eq!(draw_uniform_offset(0), 0);
        assert_eq!(draw_uniform_offset(1), 256);
        assert_eq!(draw_uniform_offset(1023), 261_888);
        assert_eq!(uniform_buffer_size(), MAX_DRAWS_PER_FRAME as u64 * 256);
    }

    #[test]
    fn pipeline_depth_state_is_reverse_z() {
        let d = depth_stencil_state();
        assert_eq!(d.format, wgpu::TextureFormat::Depth32Float);
        assert_eq!(d.depth_compare, Some(wgpu::CompareFunction::Greater));
        assert_eq!(d.depth_write_enabled, Some(true));
    }

    #[test]
    fn pipeline_shader_source_has_entry_points() {
        assert!(SHADER_UNLIT_VERTEX_COLOR.contains("fn vs_main"));
        assert!(SHADER_UNLIT_VERTEX_COLOR.contains("fn fs_main"));
        assert!(SHADER_UNLIT_VERTEX_COLOR.contains("@group(0) @binding(0)"));
    }
}
