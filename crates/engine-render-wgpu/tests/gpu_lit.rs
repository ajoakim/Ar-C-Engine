//! GPU readback tests for the lit/textured/indexed path. Ignored by default (CI has no adapter).

use engine_render::{
    BufferDesc, BufferId, BufferUsage, DirectionalLight, DistanceFog, DrawCommand, FramePacket,
    Mat3, Mat4, MeshHandle, PassPacket, PipelineDesc, PipelineId, RenderBackend, ShaderId,
    TextureDesc, TextureId, VertexLayout, VertexPosColor, VertexPosNormalUv,
    VertexPosNormalUvMorph, encode_indices, encode_lit_morph_vertices, encode_lit_vertices,
    encode_vertices,
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

// ---- morph layout and distance fog (07.1-21) ----

fn morph_pipeline(b: &mut WgpuBackend) -> PipelineId {
    b.create_pipeline(&PipelineDesc {
        label: "lit-morph",
        shader: ShaderId::LitTextured,
        vertex_layout: VertexLayout::PosNormalUvMorph,
    })
    .expect("morph pipeline")
}

fn index_quad(b: &mut WgpuBackend) -> BufferId {
    b.create_buffer(
        &BufferDesc {
            label: "ib",
            usage: BufferUsage::Index,
        },
        &encode_indices(&[0, 1, 2, 0, 2, 3]),
    )
    .expect("ib")
}

/// Quad like `quad`, with a constant uv when `uv` is Some.
fn quad_uv(b: &mut WgpuBackend, x0: f32, x1: f32, z: f32, uv: Option<[f32; 2]>) -> MeshHandle {
    let v = |x: f32, y: f32, c: [f32; 2]| VertexPosNormalUv {
        position: [x, y, z],
        normal: [0.0, 0.0, 1.0],
        uv: uv.unwrap_or(c),
    };
    let verts = [
        v(x0, -0.5, [0.0, 1.0]),
        v(x1, -0.5, [1.0, 1.0]),
        v(x1, 0.5, [1.0, 0.0]),
        v(x0, 0.5, [0.0, 0.0]),
    ];
    let vb = b
        .create_buffer(
            &BufferDesc {
                label: "vb",
                usage: BufferUsage::Vertex,
            },
            &encode_lit_vertices(&verts),
        )
        .expect("vb");
    let ib = index_quad(b);
    MeshHandle::new(VertexLayout::PosNormalUv, vb, 4).with_indices(ib, 6)
}

/// Morph quad: base corners as `quad_uv`, second corners shifted by `shift`, second uv `muv`.
fn morph_quad(
    b: &mut WgpuBackend,
    x0: f32,
    x1: f32,
    z: f32,
    uv: Option<[f32; 2]>,
    shift: [f32; 3],
    muv: Option<[f32; 2]>,
) -> MeshHandle {
    let v = |x: f32, y: f32, c: [f32; 2]| VertexPosNormalUvMorph {
        position: [x, y, z],
        normal: [0.0, 0.0, 1.0],
        uv: uv.unwrap_or(c),
        morph_position: [x + shift[0], y + shift[1], z + shift[2]],
        morph_uv: muv.unwrap_or(c),
    };
    let verts = [
        v(x0, -0.5, [0.0, 1.0]),
        v(x1, -0.5, [1.0, 1.0]),
        v(x1, 0.5, [1.0, 0.0]),
        v(x0, 0.5, [0.0, 0.0]),
    ];
    let vb = b
        .create_buffer(
            &BufferDesc {
                label: "mvb",
                usage: BufferUsage::Vertex,
            },
            &encode_lit_morph_vertices(&verts),
        )
        .expect("mvb");
    let ib = index_quad(b);
    MeshHandle::new(VertexLayout::PosNormalUvMorph, vb, 4).with_indices(ib, 6)
}

/// 2x2 texture: (0,0) red, (1,0) green, (0,1) blue, (1,1) white.
fn four_texel_texture(b: &mut WgpuBackend) -> TextureId {
    let data: [u8; 16] = [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    b.create_texture(
        &TextureDesc {
            label: "four",
            width: 2,
            height: 2,
        },
        &data,
    )
    .expect("texture")
}

fn with_morph(mut d: DrawCommand, morph: f32) -> DrawCommand {
    d.morph = morph;
    d
}

fn with_model(mut d: DrawCommand, model: Mat4) -> DrawCommand {
    d.model = model;
    d
}

fn fogged(mut p: FramePacket, fog: DistanceFog) -> FramePacket {
    p.fog = fog;
    p
}

fn fnv(buf: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in buf {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

const ORANGE: [f32; 4] = [1.0, 0.75, 0.5, 1.0];

#[test]
#[ignore = "requires a GPU adapter; run with cargo test -p engine-render-wgpu -- --ignored"]
fn gpu_morph_zero_is_bit_identical_to_pos_normal_uv() {
    let mut b = backend();
    let lit = lit_pipeline(&mut b);
    let mp = morph_pipeline(&mut b);
    let tex = four_texel_texture(&mut b);
    let plain = quad_uv(&mut b, -0.5, 0.5, 0.5, None);
    let base = render(&mut b, &packet(vec![draw(lit, plain, Some(tex), ORANGE)]));
    println!("plain hash {:016x}", fnv(&base));
    // The second position and uv differ; morph 0 must not show them.
    let morphed = morph_quad(
        &mut b,
        -0.5,
        0.5,
        0.5,
        None,
        [0.25, 0.125, 0.125],
        Some([0.5, 0.5]),
    );
    let got = render(
        &mut b,
        &packet(vec![with_morph(draw(mp, morphed, Some(tex), ORANGE), 0.0)]),
    );
    assert_eq!(base, got, "morph 0 differs from the plain pipeline");
    // Same second position and uv: any morph is identical (dyadic values are exact).
    let same = morph_quad(&mut b, -0.5, 0.5, 0.5, None, [0.0; 3], None);
    for m in [0.0, 0.25, 0.5, 1.0] {
        let got = render(
            &mut b,
            &packet(vec![with_morph(draw(mp, same, Some(tex), ORANGE), m)]),
        );
        assert_eq!(base, got, "identical second stream differs at morph {m}");
    }
}

#[test]
#[ignore = "requires a GPU adapter; run with cargo test -p engine-render-wgpu -- --ignored"]
fn gpu_morph_one_draws_at_second_position_and_uv() {
    let mut b = backend();
    let lit = lit_pipeline(&mut b);
    let mp = morph_pipeline(&mut b);
    let tex = four_texel_texture(&mut b);
    // The plain pipeline draws the quad already shifted by (0.25, 0, 0).
    let target = quad_uv(&mut b, -0.25, 0.75, 0.5, None);
    let want = render(
        &mut b,
        &packet(vec![draw(lit, target, Some(tex), [1.0; 4])]),
    );
    let morphed = morph_quad(&mut b, -0.5, 0.5, 0.5, None, [0.25, 0.0, 0.0], None);
    let got = render(
        &mut b,
        &packet(vec![with_morph(
            draw(mp, morphed, Some(tex), [1.0; 4]),
            1.0,
        )]),
    );
    assert_eq!(want, got, "morph 1 is not at the second position");
    let half = render(
        &mut b,
        &packet(vec![with_morph(
            draw(mp, morphed, Some(tex), [1.0; 4]),
            0.5,
        )]),
    );
    assert_ne!(half, got);
    // A constant uv selects one texel; morph 1 follows the second uv.
    let uvq = morph_quad(
        &mut b,
        -0.5,
        0.5,
        0.5,
        Some([0.25, 0.25]),
        [0.0; 3],
        Some([0.75, 0.25]),
    );
    let at0 = render(
        &mut b,
        &packet(vec![with_morph(draw(mp, uvq, Some(tex), [1.0; 4]), 0.0)]),
    );
    let at1 = render(
        &mut b,
        &packet(vec![with_morph(draw(mp, uvq, Some(tex), [1.0; 4]), 1.0)]),
    );
    let c0 = pixel(&at0, 32, 32);
    let c1 = pixel(&at1, 32, 32);
    println!("uv morph 0={c0:?} 1={c1:?}");
    assert!(
        c0[0] > 150 && c0[1] < 60,
        "morph 0 should sample red: {c0:?}"
    );
    assert!(
        c1[1] > 150 && c1[0] < 60,
        "morph 1 should sample green: {c1:?}"
    );
}

#[test]
#[ignore = "requires a GPU adapter; run with cargo test -p engine-render-wgpu -- --ignored"]
fn gpu_fog_density_zero_is_bit_identical() {
    let mut b = backend();
    let lit = lit_pipeline(&mut b);
    let tex = four_texel_texture(&mut b);
    let q = quad_uv(&mut b, -0.5, 0.5, 0.5, None);
    let far = Mat4::from_translation(glam::Vec3::new(0.0, 0.0, 100.0));
    let plain = packet(vec![with_model(draw(lit, q, Some(tex), ORANGE), far)]);
    let base = render(&mut b, &plain);
    println!("fog-off hash {:016x}", fnv(&base));
    // A fog colour with density 0 changes nothing.
    let off = fogged(
        plain,
        DistanceFog {
            color: [1.0, 0.0, 1.0],
            density: 0.0,
        },
    );
    assert_eq!(base, render(&mut b, &off));
}

#[test]
#[ignore = "requires a GPU adapter; run with cargo test -p engine-render-wgpu -- --ignored"]
fn gpu_fog_saturates_and_is_monotone_in_distance() {
    let mut b = backend();
    let lit = lit_pipeline(&mut b);
    let q = quad_uv(&mut b, -0.5, 0.5, 0.5, Some([0.25, 0.25]));
    let at = |b: &mut WgpuBackend, dist: f32, density: f32| {
        let m = Mat4::from_translation(glam::Vec3::new(0.0, 0.0, dist));
        let p = fogged(
            packet(vec![with_model(
                draw(lit, q, None, [1.0, 0.0, 0.0, 1.0]),
                m,
            )]),
            DistanceFog {
                color: [0.5, 0.25, 0.75],
                density,
            },
        );
        let buf = render(b, &p);
        pixel(&buf, 32, 32)
    };
    let none = at(&mut b, 0.0, 0.0);
    let near = at(&mut b, 2.0, 0.2);
    let far = at(&mut b, 20.0, 0.2);
    let sat = at(&mut b, 1.0, 1e3);
    let sat_far = at(&mut b, 1.0e3, 1e3);
    println!("none={none:?} near={near:?} far={far:?} sat={sat:?} sat_far={sat_far:?}");
    for c in 0..3 {
        assert!(sat[c].abs_diff(sat_far[c]) <= 1, "{sat:?} vs {sat_far:?}");
    }
    // Saturation reaches the fog colour (blue > green > red as in [0.5, 0.25, 0.75]... in
    // encoded order the blue channel is the largest and the green the smallest).
    assert!(
        sat[2] > sat[0] && sat[0] > sat[1],
        "not the fog colour: {sat:?}"
    );
    // The red surface moves toward the fog colour with distance.
    assert!(none[0] >= near[0] && near[0] >= far[0]);
    assert!(none[1] <= near[1] && near[1] <= far[1]);
    assert!(none[2] <= near[2] && near[2] <= far[2]);
    assert!(near[2] > none[2] && far[2] > near[2]);
}
