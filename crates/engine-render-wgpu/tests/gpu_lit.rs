//! GPU readback tests for the lit/textured/indexed path. Ignored by default (CI has no adapter).

use engine_render::{
    BufferDesc, BufferId, BufferUsage, DirectionalLight, DistanceFog, DrawCommand, FramePacket,
    Mat3, Mat4, MeshHandle, PassPacket, PipelineDesc, PipelineId, RenderBackend, ShaderId,
    TextureDesc, TextureId, VertexLayout, VertexPosColor, VertexPosNormalUv, encode_indices,
    encode_lit_vertices, encode_vertices,
};
use engine_render_wgpu::WgpuBackend;

const W: usize = 64;

fn backend() -> WgpuBackend {
    let b = WgpuBackend::new_offscreen(W as u32, W as u32).expect("offscreen backend");
    println!("adapter: {}", b.adapter_summary());
    b
}

fn lit_pipeline(b: &mut WgpuBackend) -> PipelineId {
    b.create_pipeline(&PipelineDesc {
        label: "lit",
        shader: ShaderId::LitTextured,
        vertex_layout: VertexLayout::PosNormalUv,
    })
    .expect("lit pipeline")
}

fn light() -> DirectionalLight {
    DirectionalLight {
        direction: glam::Vec3::new(0.0, 0.0, -1.0),
        color: [1.0; 3],
        intensity: 1.0,
        ambient: [0.1; 3],
    }
}

/// Quad in clip space (mvp = identity), CCW seen with y up.
fn quad(b: &mut WgpuBackend, x0: f32, x1: f32, z: f32, normal: [f32; 3]) -> MeshHandle {
    let v = |x: f32, y: f32, uv: [f32; 2]| VertexPosNormalUv {
        position: [x, y, z],
        normal,
        uv,
    };
    let verts = [
        v(x0, -0.5, [0.0, 1.0]),
        v(x1, -0.5, [1.0, 1.0]),
        v(x1, 0.5, [1.0, 0.0]),
        v(x0, 0.5, [0.0, 0.0]),
    ];
    let vb: BufferId = b
        .create_buffer(
            &BufferDesc {
                label: "vb",
                usage: BufferUsage::Vertex,
            },
            &encode_lit_vertices(&verts),
        )
        .expect("vb");
    let ib = b
        .create_buffer(
            &BufferDesc {
                label: "ib",
                usage: BufferUsage::Index,
            },
            &encode_indices(&[0, 1, 2, 0, 2, 3]),
        )
        .expect("ib");
    MeshHandle::new(VertexLayout::PosNormalUv, vb, 4).with_indices(ib, 6)
}

fn texture(b: &mut WgpuBackend, rgba: [u8; 4]) -> TextureId {
    let data: Vec<u8> = rgba.iter().copied().cycle().take(16).collect();
    b.create_texture(
        &TextureDesc {
            label: "tex",
            width: 2,
            height: 2,
        },
        &data,
    )
    .expect("texture")
}

fn draw(
    pipeline: PipelineId,
    mesh: MeshHandle,
    texture: Option<TextureId>,
    base_color: [f32; 4],
) -> DrawCommand {
    DrawCommand {
        pipeline,
        mesh,
        mvp: Mat4::IDENTITY,
        model: Mat4::IDENTITY,
        normal: Mat3::IDENTITY,
        base_color,
        texture,
        morph: 0.0,
    }
}

fn packet(draws: Vec<DrawCommand>) -> FramePacket {
    FramePacket {
        frame_index: 0,
        light: light(),
        fog: DistanceFog::default(),
        passes: vec![PassPacket {
            name: "main".to_string(),
            color_target: TextureId::BACKBUFFER,
            depth_target: Some(TextureId::DEPTH),
            clear_color: [0.0, 0.0, 0.0, 1.0],
            clear_depth: 0.0,
            draws,
        }],
    }
}

fn render(b: &mut WgpuBackend, p: &FramePacket) -> Vec<u8> {
    b.submit(p).expect("submit");
    let buf = b.read_offscreen_rgba8().expect("readback");
    assert_eq!(buf.len(), W * W * 4);
    buf
}

fn pixel(buf: &[u8], x: usize, y: usize) -> [u8; 4] {
    let i = (y * W + x) * 4;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

#[test]
#[ignore = "requires a GPU adapter; run with cargo test -p engine-render-wgpu -- --ignored"]
fn gpu_lit_side_brighter_than_shaded_side() {
    let mut b = backend();
    let pipe = lit_pipeline(&mut b);
    let white = texture(&mut b, [255; 4]);
    let left = quad(&mut b, -0.9, -0.1, 0.5, [0.0, 0.0, 1.0]);
    let right = quad(&mut b, 0.1, 0.9, 0.5, [0.0, 0.0, -1.0]);
    let p = packet(vec![
        draw(pipe, left, Some(white), [1.0; 4]),
        draw(pipe, right, Some(white), [1.0; 4]),
    ]);
    let buf = render(&mut b, &p);
    let lit = pixel(&buf, 16, 32);
    let shaded = pixel(&buf, 48, 32);
    println!("lit={lit:?} shaded={shaded:?}");
    assert!(lit[0] > shaded[0] + 60);
    assert!(shaded[0] > 20);
    assert_eq!(pixel(&buf, 1, 1), [0, 0, 0, 255]);
}

#[test]
#[ignore = "requires a GPU adapter; run with cargo test -p engine-render-wgpu -- --ignored"]
fn gpu_texture_tints_output() {
    let mut b = backend();
    let pipe = lit_pipeline(&mut b);
    let red = texture(&mut b, [255, 0, 0, 255]);
    let q = quad(&mut b, -0.9, 0.9, 0.5, [0.0, 0.0, 1.0]);
    let p = packet(vec![draw(pipe, q, Some(red), [1.0; 4])]);
    let buf = render(&mut b, &p);
    let c = pixel(&buf, 32, 32);
    println!("center={c:?}");
    assert!(c[0] > 200 && c[1] < 30 && c[2] < 30);
}

#[test]
#[ignore = "requires a GPU adapter; run with cargo test -p engine-render-wgpu -- --ignored"]
fn gpu_default_white_texture_when_none() {
    let mut b = backend();
    let pipe = lit_pipeline(&mut b);
    let q = quad(&mut b, -0.9, 0.9, 0.5, [0.0, 0.0, 1.0]);
    let p = packet(vec![draw(pipe, q, None, [0.0, 1.0, 0.0, 1.0])]);
    let buf = render(&mut b, &p);
    let c = pixel(&buf, 32, 32);
    println!("center={c:?}");
    assert!(c[1] > 200 && c[0] < 30);
}

#[test]
#[ignore = "requires a GPU adapter; run with cargo test -p engine-render-wgpu -- --ignored"]
fn gpu_reverse_z_nearer_wins() {
    let mut b = backend();
    let pipe = lit_pipeline(&mut b);
    let white = texture(&mut b, [255; 4]);
    let near = quad(&mut b, -0.9, 0.9, 0.8, [0.0, 0.0, 1.0]);
    let far = quad(&mut b, -0.9, 0.9, 0.2, [0.0, 0.0, 1.0]);
    let p = packet(vec![
        draw(pipe, near, Some(white), [1.0, 0.0, 0.0, 1.0]),
        draw(pipe, far, Some(white), [0.0, 1.0, 0.0, 1.0]),
    ]);
    let buf = render(&mut b, &p);
    let c = pixel(&buf, 32, 32);
    println!("center={c:?}");
    assert!(c[0] > 200 && c[1] < 30);
}

#[test]
#[ignore = "requires a GPU adapter; run with cargo test -p engine-render-wgpu -- --ignored"]
fn gpu_indexed_and_unindexed_mix() {
    let mut b = backend();
    let lit = lit_pipeline(&mut b);
    let unlit = b
        .create_pipeline(&PipelineDesc {
            label: "unlit",
            shader: ShaderId::UnlitVertexColor,
            vertex_layout: VertexLayout::PosColor,
        })
        .expect("unlit pipeline");
    let white = texture(&mut b, [255; 4]);
    // Lit quad on the right half, unlit triangle on the left half.
    let q = quad(&mut b, 0.1, 0.9, 0.5, [0.0, 0.0, 1.0]);
    let c = [1.0, 1.0, 1.0];
    let tri = [
        VertexPosColor {
            position: [-0.5, 0.5, 0.5],
            color: c,
        },
        VertexPosColor {
            position: [-0.9, -0.5, 0.5],
            color: c,
        },
        VertexPosColor {
            position: [-0.1, -0.5, 0.5],
            color: c,
        },
    ];
    let tb = b
        .create_buffer(
            &BufferDesc {
                label: "tri",
                usage: BufferUsage::Vertex,
            },
            &encode_vertices(&tri),
        )
        .expect("tri");
    let tri_mesh = MeshHandle::new(VertexLayout::PosColor, tb, 3);
    let p = packet(vec![
        draw(unlit, tri_mesh, None, [1.0; 4]),
        draw(lit, q, Some(white), [1.0; 4]),
    ]);
    let buf = render(&mut b, &p);
    let tri_px = pixel(&buf, 16, 40);
    let quad_px = pixel(&buf, 48, 32);
    println!("tri={tri_px:?} quad={quad_px:?}");
    assert_ne!(tri_px, [0, 0, 0, 255]);
    assert_ne!(quad_px, [0, 0, 0, 255]);
}
