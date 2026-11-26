use engine_render::*;

fn verts(n: usize) -> Vec<u8> {
    encode_vertices(&vec![VertexPosColor::default(); n])
}

fn setup() -> (NullBackend, BufferId, PipelineId) {
    let mut b = NullBackend::new();
    let buf = b
        .create_buffer(
            &BufferDesc {
                label: "vb",
                usage: BufferUsage::Vertex,
            },
            &verts(3),
        )
        .unwrap();
    let pipe = pipeline(&mut b);
    (b, buf, pipe)
}

fn pipeline(b: &mut NullBackend) -> PipelineId {
    b.create_pipeline(&PipelineDesc {
        label: "p",
        shader: ShaderId::UnlitVertexColor,
        vertex_layout: VertexLayout::PosColor,
    })
    .unwrap()
}

fn draw(pipeline: PipelineId, buf: BufferId, count: u32) -> DrawCommand {
    DrawCommand {
        pipeline,
        mesh: MeshHandle::new(VertexLayout::PosColor, buf, count),
        mvp: Mat4::IDENTITY,
        model: Mat4::IDENTITY,
        normal: Mat3::IDENTITY,
        base_color: [1.0; 4],
        texture: None,
    }
}

fn packet(index: u64, draws: Vec<DrawCommand>) -> FramePacket {
    FramePacket {
        frame_index: index,
        light: DirectionalLight::default(),
        passes: vec![PassPacket {
            name: "main".into(),
            color_target: TextureId::BACKBUFFER,
            depth_target: Some(TextureId::DEPTH),
            clear_color: [0.0, 0.0, 0.0, 1.0],
            clear_depth: REVERSE_Z_CLEAR_DEPTH,
            draws,
        }],
    }
}

#[test]
fn null_backend_records_submitted_frame() {
    let (mut b, buf, pipe) = setup();
    assert_eq!(b.submit(&packet(7, vec![draw(pipe, buf, 3)])), Ok(()));
    assert_eq!(b.frames().len(), 1);
    assert_eq!(b.submitted_count(), 1);
    assert_eq!(b.last_frame().unwrap().frame_index, 7);
    assert_eq!(b.buffer_count(), 1);
    assert_eq!(b.pipeline_count(), 1);
}

#[test]
fn null_backend_rejects_unknown_pipeline() {
    let (mut b, buf, _) = setup();
    let mut other = NullBackend::new();
    pipeline(&mut other);
    let foreign = pipeline(&mut other);
    assert_eq!(
        b.submit(&packet(0, vec![draw(foreign, buf, 3)])),
        Err(RenderError::InvalidHandle {
            kind: "pipeline",
            id: 2
        })
    );
}

#[test]
fn null_backend_rejects_destroyed_buffer() {
    let (mut b, buf, pipe) = setup();
    b.destroy_buffer(buf).unwrap();
    assert!(matches!(
        b.submit(&packet(0, vec![draw(pipe, buf, 3)])),
        Err(RenderError::InvalidHandle { kind: "buffer", .. })
    ));
    assert!(matches!(
        b.destroy_buffer(buf),
        Err(RenderError::InvalidHandle { .. })
    ));
}

#[test]
fn null_backend_rejects_mesh_out_of_bounds() {
    let (mut b, buf, pipe) = setup();
    assert!(matches!(
        b.submit(&packet(0, vec![draw(pipe, buf, 4)])),
        Err(RenderError::MeshOutOfBounds {
            required: 96,
            size: 72,
            ..
        })
    ));
    assert_eq!(
        b.submit(&packet(0, vec![draw(pipe, buf, 0)])),
        Err(RenderError::EmptyMesh)
    );
}

#[test]
fn null_backend_rejects_empty_and_oversized_buffers() {
    let mut b = NullBackend::new();
    let desc = BufferDesc {
        label: "vb",
        usage: BufferUsage::Vertex,
    };
    assert_eq!(b.create_buffer(&desc, &[]), Err(RenderError::EmptyBuffer));
    assert_eq!(b.buffer_count(), 0);
}

#[test]
fn null_backend_rejects_forward_z_clear() {
    let (mut b, buf, pipe) = setup();
    let mut p = packet(0, vec![draw(pipe, buf, 3)]);
    p.passes[0].clear_depth = 1.0;
    assert_eq!(
        b.submit(&p),
        Err(RenderError::InvalidDepthClear { value: 1.0 })
    );
}

#[test]
fn null_backend_rejects_non_finite_mvp() {
    let (mut b, buf, pipe) = setup();
    let mut d = draw(pipe, buf, 3);
    d.mvp.x_axis.x = f32::NAN;
    assert_eq!(
        b.submit(&packet(0, vec![d])),
        Err(RenderError::NonFiniteTransform)
    );
}

#[test]
fn null_backend_rejects_too_many_draws() {
    let (mut b, buf, pipe) = setup();
    let draws = vec![draw(pipe, buf, 3); MAX_DRAWS_PER_FRAME + 1];
    assert!(matches!(
        b.submit(&packet(0, draws)),
        Err(RenderError::TooManyDraws { .. })
    ));
}

#[test]
fn null_backend_rejects_unknown_texture_target() {
    let (mut b, buf, pipe) = setup();
    let mut p = packet(0, vec![draw(pipe, buf, 3)]);
    p.passes[0].depth_target = Some(TextureId::BACKBUFFER);
    assert!(matches!(
        b.submit(&p),
        Err(RenderError::InvalidHandle {
            kind: "texture",
            ..
        })
    ));
    let mut p = packet(0, vec![draw(pipe, buf, 3)]);
    p.passes[0].color_target = TextureId::DEPTH;
    assert!(matches!(
        b.submit(&p),
        Err(RenderError::InvalidHandle {
            kind: "texture",
            ..
        })
    ));
}

#[test]
fn null_backend_failed_submit_records_nothing() {
    let (mut b, buf, pipe) = setup();
    b.submit(&packet(0, vec![draw(pipe, buf, 3)])).unwrap();
    assert!(b.submit(&packet(1, vec![draw(pipe, buf, 9)])).is_err());
    assert_eq!(b.frames().len(), 1);
    assert_eq!(b.submitted_count(), 1);
}

#[test]
fn null_backend_caps_recorded_frames() {
    let (mut b, buf, pipe) = setup();
    for i in 0..300 {
        b.submit(&packet(i, vec![draw(pipe, buf, 3)])).unwrap();
    }
    assert_eq!(b.frames().len(), 256);
    assert_eq!(b.submitted_count(), 300);
    assert_eq!(b.frames()[0].frame_index, 44);
}

#[test]
fn null_backend_resize_updates_size() {
    let mut b = NullBackend::new();
    assert_eq!(b.size(), (1280, 720));
    b.resize(0, 0);
    assert_eq!(b.size(), (0, 0));
    b.resize(800, 600);
    assert_eq!(b.size(), (800, 600));
}

// ---- textures, index buffers, layouts (05-03) ----

fn lit_vertices(n: usize) -> Vec<u8> {
    encode_lit_vertices(&vec![VertexPosNormalUv::default(); n])
}

fn index_buffer(b: &mut NullBackend, indices: &[u32]) -> BufferId {
    b.create_buffer(
        &BufferDesc {
            label: "ib",
            usage: BufferUsage::Index,
        },
        &encode_indices(indices),
    )
    .unwrap()
}

fn lit_pipeline(b: &mut NullBackend) -> PipelineId {
    b.create_pipeline(&PipelineDesc {
        label: "lit",
        shader: ShaderId::LitTextured,
        vertex_layout: VertexLayout::PosNormalUv,
    })
    .unwrap()
}

fn lit_buffer(b: &mut NullBackend, n: usize) -> BufferId {
    b.create_buffer(
        &BufferDesc {
            label: "lvb",
            usage: BufferUsage::Vertex,
        },
        &lit_vertices(n),
    )
    .unwrap()
}

fn tex_desc(w: u32, h: u32) -> TextureDesc {
    TextureDesc {
        label: "t",
        width: w,
        height: h,
    }
}

#[test]
fn null_texture_ids_start_at_3_and_are_never_reused() {
    let mut b = NullBackend::new();
    let a = b.create_texture(&tex_desc(1, 1), &[0; 4]).unwrap();
    let c = b.create_texture(&tex_desc(1, 1), &[0; 4]).unwrap();
    assert_eq!((a.raw(), c.raw()), (3, 4));
    assert_eq!(b.texture_count(), 2);
    b.destroy_texture(a).unwrap();
    assert_eq!(b.texture_count(), 1);
    let d = b.create_texture(&tex_desc(1, 1), &[0; 4]).unwrap();
    assert_eq!(d.raw(), 5);
}

#[test]
fn null_texture_rejects_bad_input() {
    let mut b = NullBackend::new();
    assert!(matches!(
        b.create_texture(&tex_desc(0, 1), &[]),
        Err(RenderError::InvalidTexture(_))
    ));
    assert!(matches!(
        b.create_texture(&tex_desc(8193, 1), &[0; 4]),
        Err(RenderError::InvalidTexture(_))
    ));
    assert!(matches!(
        b.create_texture(&tex_desc(2, 2), &[0; 15]),
        Err(RenderError::InvalidTexture(_))
    ));
    assert_eq!(b.texture_count(), 0);
}

#[test]
fn null_destroy_texture_twice_errors() {
    let mut b = NullBackend::new();
    let t = b.create_texture(&tex_desc(1, 1), &[0; 4]).unwrap();
    assert_eq!(b.destroy_texture(t), Ok(()));
    assert!(matches!(
        b.destroy_texture(t),
        Err(RenderError::InvalidHandle {
            kind: "texture",
            ..
        })
    ));
    assert!(matches!(
        b.destroy_texture(TextureId::BACKBUFFER),
        Err(RenderError::InvalidHandle { .. })
    ));
}

#[test]
fn null_submit_rejects_layout_mismatch() {
    let mut b = NullBackend::new();
    let vb = lit_buffer(&mut b, 3);
    let pipe = pipeline(&mut b);
    let d = DrawCommand {
        pipeline: pipe,
        mesh: MeshHandle::new(VertexLayout::PosNormalUv, vb, 3),
        mvp: Mat4::IDENTITY,
        model: Mat4::IDENTITY,
        normal: Mat3::IDENTITY,
        base_color: [1.0; 4],
        texture: None,
    };
    assert!(matches!(
        b.submit(&packet(0, vec![d])),
        Err(RenderError::LayoutMismatch { .. })
    ));
}

#[test]
fn null_submit_rejects_bad_index_buffers() {
    let mut b = NullBackend::new();
    let vb = lit_buffer(&mut b, 4);
    let pipe = lit_pipeline(&mut b);
    let mk = |ib: BufferId, count: u32| DrawCommand {
        pipeline: pipe,
        mesh: MeshHandle::new(VertexLayout::PosNormalUv, vb, 4).with_indices(ib, count),
        mvp: Mat4::IDENTITY,
        model: Mat4::IDENTITY,
        normal: Mat3::IDENTITY,
        base_color: [1.0; 4],
        texture: None,
    };
    // Index buffer created with Vertex usage.
    let wrong = b
        .create_buffer(
            &BufferDesc {
                label: "x",
                usage: BufferUsage::Vertex,
            },
            &encode_indices(&[0, 1, 2]),
        )
        .unwrap();
    assert!(matches!(
        b.submit(&packet(0, vec![mk(wrong, 3)])),
        Err(RenderError::WrongBufferUsage { .. })
    ));
    let ib = index_buffer(&mut b, &[0, 1, 2]);
    assert!(matches!(
        b.submit(&packet(0, vec![mk(ib, 4)])),
        Err(RenderError::InvalidIndices(_))
    ));
    assert!(matches!(
        b.submit(&packet(0, vec![mk(ib, 6)])),
        Err(RenderError::MeshOutOfBounds { .. })
    ));
}

#[test]
fn null_submit_accepts_indexed_lit_mesh() {
    let mut b = NullBackend::new();
    let vb = lit_buffer(&mut b, 4);
    let ib = index_buffer(&mut b, &[0, 1, 2, 2, 1, 3]);
    let pipe = lit_pipeline(&mut b);
    let d = DrawCommand {
        pipeline: pipe,
        mesh: MeshHandle::new(VertexLayout::PosNormalUv, vb, 4).with_indices(ib, 6),
        mvp: Mat4::IDENTITY,
        model: Mat4::IDENTITY,
        normal: Mat3::IDENTITY,
        base_color: [1.0; 4],
        texture: None,
    };
    assert_eq!(b.submit(&packet(0, vec![d])), Ok(()));
}

fn shared_handle(ib: BufferId, count: u32, max_index: u32) -> IndexBufferHandle {
    IndexBufferHandle {
        buffer: ib,
        count,
        max_index,
    }
}

fn range_draw(pipe: PipelineId, mesh: MeshHandle) -> DrawCommand {
    DrawCommand {
        pipeline: pipe,
        mesh,
        mvp: Mat4::IDENTITY,
        model: Mat4::IDENTITY,
        normal: Mat3::IDENTITY,
        base_color: [1.0; 4],
        texture: None,
    }
}

#[test]
fn shared_index_buffer_range_draws() {
    let mut b = NullBackend::new();
    let vb1 = lit_buffer(&mut b, 4);
    let vb2 = lit_buffer(&mut b, 4);
    let ib = index_buffer(&mut b, &[0, 1, 2, 2, 1, 3]);
    let h = shared_handle(ib, 6, 3);
    let pipe = lit_pipeline(&mut b);
    let m1 = MeshHandle::new(VertexLayout::PosNormalUv, vb1, 4)
        .with_index_range(&h, 0, 3)
        .unwrap();
    let m2 = MeshHandle::new(VertexLayout::PosNormalUv, vb2, 4)
        .with_index_range(&h, 3, 3)
        .unwrap();
    assert_eq!(
        m2.index.map(|i| (i.first, i.count, i.shared)),
        Some((3, 3, true))
    );
    let p = packet(0, vec![range_draw(pipe, m1), range_draw(pipe, m2)]);
    assert_eq!(b.submit(&p), Ok(()));
}

#[test]
fn range_out_of_bounds_rejected() {
    let mut b = NullBackend::new();
    let vb = lit_buffer(&mut b, 4);
    let ib = index_buffer(&mut b, &[0, 1, 2, 2, 1, 3]);
    let h = shared_handle(ib, 6, 3);
    let m = MeshHandle::new(VertexLayout::PosNormalUv, vb, 4);
    assert!(m.with_index_range(&h, 6, 3).is_err());
    assert!(m.with_index_range(&h, 3, 6).is_err());
    assert!(m.with_index_range(&h, 0, 0).is_err());
    assert!(m.with_index_range(&h, 0, 4).is_err());
    assert!(m.with_index_range(&h, u32::MAX, 3).is_err());
    // A forged handle with an oversized range is caught at submit.
    let pipe = lit_pipeline(&mut b);
    let big = shared_handle(ib, 12, 3);
    let bad = m.with_index_range(&big, 6, 6).unwrap();
    assert!(matches!(
        b.submit(&packet(0, vec![range_draw(pipe, bad)])),
        Err(RenderError::MeshOutOfBounds { .. })
    ));
}

#[test]
fn max_index_beyond_vertex_count_rejected() {
    let mut b = NullBackend::new();
    let vb = lit_buffer(&mut b, 3);
    let ib = index_buffer(&mut b, &[0, 1, 3]);
    let pipe = lit_pipeline(&mut b);
    let m = MeshHandle::new(VertexLayout::PosNormalUv, vb, 3);
    assert!(m.with_index_range(&shared_handle(ib, 3, 3), 0, 3).is_err());
    // A handle that understates max_index is still caught at submit.
    let forged = m.with_index_range(&shared_handle(ib, 3, 2), 0, 3).unwrap();
    assert_eq!(
        b.submit(&packet(0, vec![range_draw(pipe, forged)])),
        Err(RenderError::InvalidIndices("index exceeds vertex count"))
    );
}
