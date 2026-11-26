//! WORLD-03 precision proof: camera-relative rendering at planetary distance.

use engine_math::{FrameRegistry, Vec3, WorldPos};
use engine_render::{
    BufferDesc, BufferUsage, Camera, DirectionalLight, MaterialTable, MeshHandle, NullBackend,
    PipelineDesc, PipelineSet, RenderBackend, RenderGraph, RenderItem, ShaderId, VertexLayout,
    VertexPosColor, encode_vertices, extract,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const R: f64 = 6.4e6;
const SUB_MM: f64 = 1e-3;
const ASPECT: f32 = 16.0 / 9.0;

fn pos(f: &FrameRegistry, x: f64, y: f64, z: f64) -> Result<WorldPos, engine_math::MathError> {
    WorldPos::from_xyz(f.root(), x, y, z)
}

fn dummy_mesh(be: &mut NullBackend) -> Result<MeshHandle, engine_render::RenderError> {
    let data = encode_vertices(&[VertexPosColor::default(); 3]);
    let buf = be.create_buffer(
        &BufferDesc {
            label: "tri",
            usage: BufferUsage::Vertex,
        },
        &data,
    )?;
    Ok(MeshHandle::new(VertexLayout::PosColor, buf, 3))
}

#[test]
fn precision_naive_f32_cast_loses_submeter() {
    let mut max_err = 0.0f64;
    for k in 0..1000 {
        let x = R + f64::from(k) * 1e-3;
        let err = ((x as f32) as f64 - x).abs();
        max_err = max_err.max(err);
    }
    assert!(max_err >= 0.2, "naive max error {max_err}");
}

#[test]
fn precision_camera_relative_within_sub_mm() -> TestResult {
    let f = FrameRegistry::new();
    let cam = Camera::new(pos(&f, 0.0, R, 0.0)?);
    let mut items = Vec::new();
    let mut exact = Vec::new();
    for ix in -8..=8 {
        for iz in -8..=8 {
            for dy in [-3.7, 0.0, 2.25] {
                let (dx, dz) = (f64::from(ix) * 12.5, f64::from(iz) * 12.5);
                items.push(RenderItem::new(
                    MeshHandle::new(VertexLayout::PosColor, dummy_buffer()?, 3),
                    pos(&f, dx, R + dy, dz)?,
                ));
                exact.push([dx, dy, dz]);
            }
        }
    }
    let scene = extract(
        &f,
        &cam,
        ASPECT,
        &items,
        0,
        &MaterialTable::new(),
        &DirectionalLight::default(),
    )?;
    assert_eq!(scene.draws.len(), exact.len());
    for (d, e) in scene.draws.iter().zip(&exact) {
        let t = d.model.w_axis.truncate();
        for (got, want) in [t.x, t.y, t.z].into_iter().zip(e) {
            let err = (f64::from(got) - want).abs();
            assert!(err < SUB_MM, "err {err}");
            assert!(err < 1e-5, "err {err}");
        }
    }
    Ok(())
}

fn dummy_buffer() -> Result<engine_render::BufferId, engine_render::RenderError> {
    let mut be = NullBackend::new();
    Ok(dummy_mesh(&mut be)?.vertex_buffer)
}

#[test]
fn precision_no_frame_to_frame_jitter() -> TestResult {
    let f = FrameRegistry::new();
    let item = RenderItem::new(
        MeshHandle::new(VertexLayout::PosColor, dummy_buffer()?, 3),
        pos(&f, 0.0, R, -5.0)?,
    );
    let mut prev: Option<f64> = None;
    let mut naive_max = 0.0f64;
    for k in 0..1000u32 {
        let step = f64::from(k) * 1e-4;
        let cam = Camera::new(pos(&f, step, R, 0.0)?);
        let scene = extract(
            &f,
            &cam,
            ASPECT,
            &[item],
            u64::from(k),
            &MaterialTable::new(),
            &DirectionalLight::default(),
        )?;
        let rel_x = f64::from(scene.draws[0].model.w_axis.x);
        assert!((rel_x - (-step)).abs() < 1e-5);
        if let Some(p) = prev {
            assert!(((rel_x - p) - (-1e-4)).abs() < 1e-5, "jump at frame {k}");
        }
        prev = Some(rel_x);
        let naive = f64::from((R + step) as f32 - R as f32);
        naive_max = naive_max.max((naive - step).abs());
    }
    assert!(naive_max >= 0.05, "naive max error {naive_max}");
    Ok(())
}

fn ndc_for(
    cam_pos: WorldPos,
    item_pos: WorldPos,
    f: &FrameRegistry,
) -> Result<Vec3, Box<dyn std::error::Error>> {
    let item = RenderItem::new(
        MeshHandle::new(VertexLayout::PosColor, dummy_buffer()?, 3),
        item_pos,
    );
    let scene = extract(
        f,
        &Camera::new(cam_pos),
        ASPECT,
        &[item],
        0,
        &MaterialTable::new(),
        &DirectionalLight::default(),
    )?;
    Ok((scene.view_proj * scene.draws[0].model).project_point3(Vec3::new(0.1, 0.1, 0.0)))
}

#[test]
fn precision_projected_ndc_matches_camera_at_origin() -> TestResult {
    let f = FrameRegistry::new();
    let a = ndc_for(pos(&f, 0.0, 0.0, 0.0)?, pos(&f, 0.3, 0.2, -5.0)?, &f)?;
    let b = ndc_for(pos(&f, R, 0.0, 0.0)?, pos(&f, R + 0.3, 0.2, -5.0)?, &f)?;
    assert!((a.x - b.x).abs() < 1e-6);
    assert!((a.y - b.y).abs() < 1e-6);
    assert!((a.z - b.z).abs() < 1e-6);
    Ok(())
}

#[test]
fn precision_full_frame_on_null_backend() -> TestResult {
    let mut be = NullBackend::new();
    let mesh = dummy_mesh(&mut be)?;
    let pipe = be.create_pipeline(&PipelineDesc {
        label: "p",
        shader: ShaderId::UnlitVertexColor,
        vertex_layout: VertexLayout::PosColor,
    })?;
    let f = FrameRegistry::new();
    let cam = Camera::new(pos(&f, 0.0, R, 0.0)?);
    let item = RenderItem::new(mesh, pos(&f, 1.0, R + 0.5, -10.0)?);
    let mut set = PipelineSet::new();
    set.insert(VertexLayout::PosColor, pipe);
    let scene = extract(
        &f,
        &cam,
        ASPECT,
        &[item],
        1,
        &MaterialTable::new(),
        &DirectionalLight::default(),
    )?;
    let packet =
        RenderGraph::main_p0()
            .compile()?
            .build_frame_packet(&scene, &set, [0.0, 0.0, 0.0, 1.0])?;
    be.submit(&packet)?;
    let last = be.last_frame().ok_or("no frame recorded")?;
    assert!(last.passes[0].draws[0].mvp.is_finite());
    Ok(())
}
