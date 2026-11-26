//! REND-04: lit, textured, indexed packets validated end-to-end on the NullBackend.

use engine_math::{FrameRegistry, Vec3, WorldPos};
use engine_render::*;

struct Rig {
    be: NullBackend,
    set: PipelineSet,
    mesh: MeshHandle,
    tex: TextureId,
}

fn quad() -> (Vec<VertexPosNormalUv>, Vec<u32>) {
    let n = [0.0, 0.0, 1.0];
    let v = |p: [f32; 3], uv: [f32; 2]| VertexPosNormalUv {
        position: p,
        normal: n,
        uv,
    };
    (
        vec![
            v([-1.0, -1.0, 0.0], [0.0, 1.0]),
            v([1.0, -1.0, 0.0], [1.0, 1.0]),
            v([1.0, 1.0, 0.0], [1.0, 0.0]),
            v([-1.0, 1.0, 0.0], [0.0, 0.0]),
        ],
        vec![0, 1, 2, 0, 2, 3],
    )
}

fn upload_lit(b: &mut NullBackend, verts: &[VertexPosNormalUv], idx: &[u32]) -> MeshHandle {
    let vb = b
        .create_buffer(
            &BufferDesc {
                label: "vb",
                usage: BufferUsage::Vertex,
            },
            &encode_lit_vertices(verts),
        )
        .unwrap();
    let ib = b
        .create_buffer(
            &BufferDesc {
                label: "ib",
                usage: BufferUsage::Index,
            },
            &encode_indices(idx),
        )
        .unwrap();
    MeshHandle::new(VertexLayout::PosNormalUv, vb, 4).with_indices(ib, 6)
}

fn rig() -> Rig {
    let mut be = NullBackend::new();
    let lit = be
        .create_pipeline(&PipelineDesc {
            label: "lit",
            shader: ShaderId::LitTextured,
            vertex_layout: VertexLayout::PosNormalUv,
        })
        .unwrap();
    let unlit = be
        .create_pipeline(&PipelineDesc {
            label: "unlit",
            shader: ShaderId::UnlitVertexColor,
            vertex_layout: VertexLayout::PosColor,
        })
        .unwrap();
    let mut set = PipelineSet::new();
    set.insert(VertexLayout::PosNormalUv, lit);
    set.insert(VertexLayout::PosColor, unlit);
    let (v, i) = quad();
    let mesh = upload_lit(&mut be, &v, &i);
    let tex = be
        .create_texture(
            &TextureDesc {
                label: "t",
                width: 2,
                height: 2,
            },
            &[255; 16],
        )
        .unwrap();
    Rig { be, set, mesh, tex }
}

fn light() -> DirectionalLight {
    DirectionalLight {
        direction: Vec3::new(0.0, -1.0, 0.0),
        color: [1.0; 3],
        intensity: 2.0,
        ambient: [0.1; 3],
    }
}

fn material(tex: TextureId) -> Material {
    Material {
        base_color: [0.8, 0.2, 0.2, 1.0],
        base_color_texture: Some(tex),
    }
}

fn pos(f: &FrameRegistry, x: f64, y: f64, z: f64) -> WorldPos {
    WorldPos::from_xyz(f.root(), x, y, z).unwrap()
}

fn lit_packet(r: &Rig) -> FramePacket {
    let f = FrameRegistry::new();
    let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
    let mut mt = MaterialTable::new();
    let id = mt.add(material(r.tex)).unwrap();
    let item = RenderItem::new(r.mesh, pos(&f, 0.0, 0.0, -5.0)).with_material(id);
    let scene = extract(&f, &cam, 1.5, &[item], 3, &mt, &light()).unwrap();
    RenderGraph::main_p0()
        .compile()
        .unwrap()
        .build_frame_packet(&scene, &r.set, [0.0, 0.0, 0.0, 1.0])
        .unwrap()
}

#[test]
fn lit_packet_end_to_end() {
    let mut r = rig();
    let pk = lit_packet(&r);
    assert_eq!(r.be.submit(&pk), Ok(()));
    let last = r.be.last_frame().unwrap();
    assert_eq!(last.light, light());
    let pass = &last.passes[0];
    assert_eq!(pass.draws[0].texture, Some(r.tex));
    assert_eq!(pass.draws[0].base_color, [0.8, 0.2, 0.2, 1.0]);
    assert_eq!(pass.clear_depth, 0.0);
    assert_eq!(pass.depth_target, Some(TextureId::DEPTH));
}

#[test]
fn lit_packet_rejects_destroyed_texture() {
    let mut r = rig();
    let pk = lit_packet(&r);
    r.be.destroy_texture(r.tex).unwrap();
    assert!(matches!(
        r.be.submit(&pk),
        Err(RenderError::InvalidHandle {
            kind: "texture",
            ..
        })
    ));
}

#[test]
fn lit_packet_rejects_surface_texture_on_draw() {
    let mut r = rig();
    let mut pk = lit_packet(&r);
    pk.passes[0].draws[0].texture = Some(TextureId::BACKBUFFER);
    assert_eq!(
        r.be.submit(&pk),
        Err(RenderError::InvalidHandle {
            kind: "texture",
            id: 1
        })
    );
}

#[test]
fn lit_packet_rejects_non_finite() {
    let mut r = rig();
    let mut pk = lit_packet(&r);
    pk.passes[0].draws[0].base_color[2] = f32::NAN;
    assert_eq!(r.be.submit(&pk), Err(RenderError::NonFiniteTransform));

    let mut pk = lit_packet(&r);
    pk.passes[0].draws[0].normal.x_axis.x = f32::NAN;
    assert_eq!(r.be.submit(&pk), Err(RenderError::NonFiniteTransform));

    let mut pk = lit_packet(&r);
    pk.light.intensity = -1.0;
    assert!(matches!(
        r.be.submit(&pk),
        Err(RenderError::InvalidLight(_))
    ));
}

#[test]
fn lit_packet_missing_pipeline_for_layout() {
    let r = rig();
    let f = FrameRegistry::new();
    let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
    let item = RenderItem::new(r.mesh, pos(&f, 0.0, 0.0, -5.0));
    let scene = extract(
        &f,
        &cam,
        1.0,
        &[item],
        0,
        &MaterialTable::new(),
        &DirectionalLight::default(),
    )
    .unwrap();
    let mut only_unlit = PipelineSet::new();
    only_unlit.insert(
        VertexLayout::PosColor,
        r.set.get(VertexLayout::PosColor).unwrap(),
    );
    let res = RenderGraph::main_p0()
        .compile()
        .unwrap()
        .build_frame_packet(&scene, &only_unlit, [0.0; 4]);
    assert!(matches!(res, Err(RenderError::MissingPipeline(_))));
}

#[test]
fn lit_packet_precision_far_from_origin() {
    let r = rig();
    let f = FrameRegistry::new();
    let cam = Camera::new(pos(&f, 1e6, 2.0, 5.0));
    let item = RenderItem::new(r.mesh, pos(&f, 1e6, 0.0, 0.0));
    let scene = extract(
        &f,
        &cam,
        1.0,
        &[item],
        0,
        &MaterialTable::new(),
        &DirectionalLight::default(),
    )
    .unwrap();
    let d = &scene.draws[0];
    assert_eq!(d.model.w_axis.truncate(), Vec3::new(0.0, -2.0, -5.0));
    assert!(d.normal.is_finite());
}

#[test]
fn lit_packet_mixed_layouts_use_matching_pipelines() {
    let mut r = rig();
    let unlit_buf =
        r.be.create_buffer(
            &BufferDesc {
                label: "c",
                usage: BufferUsage::Vertex,
            },
            &encode_vertices(&[VertexPosColor::default(); 3]),
        )
        .unwrap();
    let unlit_mesh = MeshHandle::new(VertexLayout::PosColor, unlit_buf, 3);
    let f = FrameRegistry::new();
    let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
    let items = [
        RenderItem::new(unlit_mesh, pos(&f, 0.0, 0.0, -3.0)),
        RenderItem::new(r.mesh, pos(&f, 0.0, 0.0, -4.0)),
    ];
    let scene = extract(
        &f,
        &cam,
        1.0,
        &items,
        0,
        &MaterialTable::new(),
        &DirectionalLight::default(),
    )
    .unwrap();
    let pk = RenderGraph::main_p0()
        .compile()
        .unwrap()
        .build_frame_packet(&scene, &r.set, [0.0; 4])
        .unwrap();
    let d = &pk.passes[0].draws;
    assert_eq!(d[0].pipeline, r.set.get(VertexLayout::PosColor).unwrap());
    assert_eq!(d[1].pipeline, r.set.get(VertexLayout::PosNormalUv).unwrap());
    assert_ne!(d[0].pipeline, d[1].pipeline);
    assert_eq!(r.be.submit(&pk), Ok(()));
}
