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
        mesh: MeshHandle {
            vertex_buffer: buf,
            vertex_count: count,
        },
        mvp: Mat4::IDENTITY,
    }
}

fn packet(index: u64, draws: Vec<DrawCommand>) -> FramePacket {
    FramePacket {
        frame_index: index,
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
