//! GPU smoke test. Ignored by default (CI has no adapter).

use engine_render::{
    BufferDesc, BufferUsage, DirectionalLight, DrawCommand, FramePacket, Mat3, Mat4, MeshHandle,
    PassPacket, PipelineDesc, RenderBackend, ShaderId, TextureId, VertexLayout, VertexPosColor,
    encode_vertices,
};
use engine_render_wgpu::WgpuBackend;

#[test]
#[ignore = "requires a GPU adapter; run with cargo test -p engine-render-wgpu -- --ignored"]
fn gpu_offscreen_triangle_submits() {
    let mut backend = WgpuBackend::new_offscreen(64, 64).expect("offscreen backend");
    println!("adapter: {}", backend.adapter_summary());
    let pipeline = backend
        .create_pipeline(&PipelineDesc {
            label: "unlit",
            shader: ShaderId::UnlitVertexColor,
            vertex_layout: VertexLayout::PosColor,
        })
        .expect("pipeline");
    let verts = [
        VertexPosColor {
            position: [0.0, 0.5, 0.5],
            color: [1.0, 0.0, 0.0],
        },
        VertexPosColor {
            position: [-0.5, -0.5, 0.5],
            color: [0.0, 1.0, 0.0],
        },
        VertexPosColor {
            position: [0.5, -0.5, 0.5],
            color: [0.0, 0.0, 1.0],
        },
    ];
    let buffer = backend
        .create_buffer(
            &BufferDesc {
                label: "tri",
                usage: BufferUsage::Vertex,
            },
            &encode_vertices(&verts),
        )
        .expect("buffer");
    let packet = FramePacket {
        frame_index: 0,
        light: DirectionalLight::default(),
        passes: vec![PassPacket {
            name: "main".to_string(),
            color_target: TextureId::BACKBUFFER,
            depth_target: Some(TextureId::DEPTH),
            clear_color: [0.0, 0.0, 0.0, 1.0],
            clear_depth: 0.0,
            draws: vec![DrawCommand {
                pipeline,
                mesh: MeshHandle::new(VertexLayout::PosColor, buffer, 3),
                mvp: Mat4::IDENTITY,
                model: Mat4::IDENTITY,
                normal: Mat3::IDENTITY,
                base_color: [1.0; 4],
                texture: None,
            }],
        }],
    };
    backend.submit(&packet).expect("submit 1");
    backend.submit(&packet).expect("submit 2");
    backend.resize(32, 32);
    backend.submit(&packet).expect("submit after resize");
    backend.resize(0, 0);
    backend.submit(&packet).expect("submit zero size skipped");
}
