//! Pipeline construction helpers and fixed layout constants.

use engine_render::{
    DirectionalLight, DrawCommand, MAX_DRAWS_PER_FRAME, PipelineDesc, RenderError, ShaderId,
    VertexLayout,
};

pub(crate) const SHADER_LIT_TEXTURED: &str = include_str!("shaders/lit_textured.wgsl");
pub(crate) const SHADER_UNLIT_VERTEX_COLOR: &str = include_str!("shaders/unlit_vertex_color.wgsl");

/// Byte stride between per-draw uniform slots (at least wgpu's default dynamic offset alignment).
pub const DRAW_UNIFORM_STRIDE: u64 = 256;

/// Depth format used for all depth targets.
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Size of the per-frame light uniform.
pub(crate) const FRAME_UNIFORM_BYTES: u64 = 48;
/// Size of the per-draw uniform (mvp, model, normal columns, base color).
pub(crate) const DRAW_UNIFORM_BYTES: u64 = 192;

/// Per-frame light uniform (group 0).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FrameUniform {
    pub light_dir: [f32; 4],
    pub light_color: [f32; 4],
    pub ambient: [f32; 4],
}

impl FrameUniform {
    pub(crate) fn from_light(l: &DirectionalLight) -> Self {
        let d = l.direction.normalize_or_zero();
        Self {
            light_dir: [d.x, d.y, d.z, 0.0],
            light_color: [
                l.color[0] * l.intensity,
                l.color[1] * l.intensity,
                l.color[2] * l.intensity,
                0.0,
            ],
            ambient: [l.ambient[0], l.ambient[1], l.ambient[2], 0.0],
        }
    }
}

/// Per-draw uniform (group 1, dynamic offset).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DrawUniform {
    pub mvp: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    pub normal: [[f32; 4]; 3],
    pub base_color: [f32; 4],
}

impl DrawUniform {
    pub(crate) fn from_draw(d: &DrawCommand) -> Self {
        let n = d.normal;
        Self {
            mvp: d.mvp.to_cols_array_2d(),
            model: d.model.to_cols_array_2d(),
            normal: [
                [n.x_axis.x, n.x_axis.y, n.x_axis.z, 0.0],
                [n.y_axis.x, n.y_axis.y, n.y_axis.z, 0.0],
                [n.z_axis.x, n.z_axis.y, n.z_axis.z, 0.0],
            ],
            base_color: d.base_color,
        }
    }
}

pub(crate) fn draw_uniform_offset(index: usize) -> u64 {
    index as u64 * DRAW_UNIFORM_STRIDE
}

pub(crate) fn uniform_buffer_size() -> u64 {
    MAX_DRAWS_PER_FRAME as u64 * DRAW_UNIFORM_STRIDE
}

pub(crate) struct BindGroupLayouts {
    pub frame: wgpu::BindGroupLayout,
    pub draw: wgpu::BindGroupLayout,
    pub texture: wgpu::BindGroupLayout,
}

pub(crate) fn create_bind_group_layouts(device: &wgpu::Device) -> BindGroupLayouts {
    let uniform = |vis: wgpu::ShaderStages, dynamic: bool, size: u64| wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: vis,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: wgpu::BufferSize::new(size),
        },
        count: None,
    };
    let frame = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frame-uniform-layout"),
        entries: &[uniform(
            wgpu::ShaderStages::FRAGMENT,
            false,
            FRAME_UNIFORM_BYTES,
        )],
    });
    let draw = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("draw-uniform-layout"),
        entries: &[uniform(
            wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            true,
            DRAW_UNIFORM_BYTES,
        )],
    });
    let texture = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("texture-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    BindGroupLayouts {
        frame,
        draw,
        texture,
    }
}

pub(crate) fn cull_mode_for(shader: ShaderId) -> Option<wgpu::Face> {
    match shader {
        ShaderId::LitTextured => Some(wgpu::Face::Back),
        _ => None,
    }
}

pub(crate) fn uses_texture(shader: ShaderId) -> bool {
    matches!(shader, ShaderId::LitTextured)
}

pub(crate) fn vertex_attributes(
    layout: VertexLayout,
) -> Result<(u64, Vec<wgpu::VertexAttribute>), RenderError> {
    let attr = |format, offset, shader_location| wgpu::VertexAttribute {
        format,
        offset,
        shader_location,
    };
    match layout {
        VertexLayout::PosColor => {
            let o = layout.attribute_offsets();
            Ok((
                layout.stride(),
                vec![
                    attr(wgpu::VertexFormat::Float32x3, o[0], 0),
                    attr(wgpu::VertexFormat::Float32x3, o[1], 1),
                ],
            ))
        }
        VertexLayout::PosNormalUv => {
            let o = layout.attribute_offsets();
            Ok((
                layout.stride(),
                vec![
                    attr(wgpu::VertexFormat::Float32x3, o[0], 0),
                    attr(wgpu::VertexFormat::Float32x3, o[1], 1),
                    attr(wgpu::VertexFormat::Float32x2, o[2], 2),
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
    layouts: &BindGroupLayouts,
) -> Result<wgpu::RenderPipeline, RenderError> {
    let source = match desc.shader {
        ShaderId::UnlitVertexColor => SHADER_UNLIT_VERTEX_COLOR,
        ShaderId::LitTextured => SHADER_LIT_TEXTURED,
        #[allow(unreachable_patterns)]
        _ => return Err(RenderError::Backend("unsupported shader".into())),
    };
    let (stride, attributes) = vertex_attributes(desc.vertex_layout)?;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(desc.label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let group_layouts: Vec<Option<&wgpu::BindGroupLayout>> = if uses_texture(desc.shader) {
        vec![
            Some(&layouts.frame),
            Some(&layouts.draw),
            Some(&layouts.texture),
        ]
    } else {
        vec![Some(&layouts.frame), Some(&layouts.draw)]
    };
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(desc.label),
        bind_group_layouts: &group_layouts,
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
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: cull_mode_for(desc.shader),
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil_state()),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_pos_color_attributes() {
        let (stride, attrs) = vertex_attributes(VertexLayout::PosColor).unwrap();
        assert_eq!(stride, 24);
        assert_eq!(attrs.len(), 2);
        assert_eq!(attrs[0].shader_location, 0);
        assert_eq!(attrs[0].offset, 0);
        assert_eq!(attrs[0].format, wgpu::VertexFormat::Float32x3);
        assert_eq!(attrs[1].shader_location, 1);
        assert_eq!(attrs[1].offset, 12);
        assert_eq!(attrs[1].format, wgpu::VertexFormat::Float32x3);
    }

    #[test]
    fn pipeline_pos_normal_uv_attributes() {
        let (stride, attrs) = vertex_attributes(VertexLayout::PosNormalUv).unwrap();
        assert_eq!(stride, 32);
        assert_eq!(attrs.len(), 3);
        let want = [
            (0, wgpu::VertexFormat::Float32x3),
            (12, wgpu::VertexFormat::Float32x3),
            (24, wgpu::VertexFormat::Float32x2),
        ];
        for (i, (off, fmt)) in want.into_iter().enumerate() {
            assert_eq!(attrs[i].shader_location, i as u32);
            assert_eq!(attrs[i].offset, off);
            assert_eq!(attrs[i].format, fmt);
        }
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
        assert!(SHADER_UNLIT_VERTEX_COLOR.contains("@group(1) @binding(0)"));
    }

    #[test]
    fn uniform_sizes_and_offsets() {
        use std::mem::{offset_of, size_of};
        assert_eq!(size_of::<FrameUniform>() as u64, FRAME_UNIFORM_BYTES);
        assert_eq!(size_of::<DrawUniform>() as u64, DRAW_UNIFORM_BYTES);
        assert_eq!(offset_of!(DrawUniform, model), 64);
        assert_eq!(offset_of!(DrawUniform, normal), 128);
        assert_eq!(offset_of!(DrawUniform, base_color), 176);
        const { assert!(DRAW_UNIFORM_BYTES <= DRAW_UNIFORM_STRIDE) };
    }

    #[test]
    fn draw_uniform_packs_normal_columns() {
        use engine_render::{
            BufferDesc, BufferUsage, Mat3, Mat4, MeshHandle, NullBackend, RenderBackend,
        };
        let mut be = NullBackend::new();
        let pipeline = be
            .create_pipeline(&PipelineDesc {
                label: "p",
                shader: ShaderId::UnlitVertexColor,
                vertex_layout: VertexLayout::PosColor,
            })
            .unwrap();
        let vb = be
            .create_buffer(
                &BufferDesc {
                    label: "b",
                    usage: BufferUsage::Vertex,
                },
                &[0u8; 72],
            )
            .unwrap();
        let a = glam::Vec3::new(1.0, 2.0, 3.0);
        let b = glam::Vec3::new(4.0, 5.0, 6.0);
        let c = glam::Vec3::new(7.0, 8.0, 9.0);
        let d = DrawCommand {
            pipeline,
            mesh: MeshHandle::new(VertexLayout::PosColor, vb, 3),
            mvp: Mat4::IDENTITY,
            model: Mat4::from_scale(glam::Vec3::splat(2.0)),
            normal: Mat3::from_cols(a, b, c),
            base_color: [0.1, 0.2, 0.3, 0.4],
            texture: None,
        };
        let u = DrawUniform::from_draw(&d);
        assert_eq!(u.normal[0], [1.0, 2.0, 3.0, 0.0]);
        assert_eq!(u.normal[1], [4.0, 5.0, 6.0, 0.0]);
        assert_eq!(u.normal[2], [7.0, 8.0, 9.0, 0.0]);
        assert_eq!(u.base_color, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(u.mvp, Mat4::IDENTITY.to_cols_array_2d());
        assert_eq!(u.model, d.model.to_cols_array_2d());
    }

    #[test]
    fn frame_uniform_from_light() {
        let l = DirectionalLight {
            direction: glam::Vec3::new(0.0, -2.0, 0.0),
            color: [1.0, 0.5, 0.25],
            intensity: 2.0,
            ambient: [0.1, 0.2, 0.3],
        };
        let u = FrameUniform::from_light(&l);
        assert_eq!(u.light_dir, [0.0, -1.0, 0.0, 0.0]);
        assert_eq!(&u.light_color[..3], &[2.0, 1.0, 0.5]);
        assert_eq!(u.ambient, [0.1, 0.2, 0.3, 0.0]);
    }

    #[test]
    fn lit_shader_source_bindings() {
        for needle in [
            "fn vs_main",
            "fn fs_main",
            "@group(0) @binding(0)",
            "@group(1) @binding(0)",
            "@group(2) @binding(0)",
            "@group(2) @binding(1)",
            "textureSample",
            "max(dot(",
        ] {
            assert!(SHADER_LIT_TEXTURED.contains(needle), "missing {needle}");
        }
    }

    #[test]
    fn unlit_shader_uses_group_1() {
        assert!(SHADER_UNLIT_VERTEX_COLOR.contains("@group(1) @binding(0)"));
        assert!(!SHADER_UNLIT_VERTEX_COLOR.contains("@group(0)"));
    }

    #[test]
    fn cull_mode_per_shader() {
        assert_eq!(cull_mode_for(ShaderId::LitTextured), Some(wgpu::Face::Back));
        assert_eq!(cull_mode_for(ShaderId::UnlitVertexColor), None);
    }
}
