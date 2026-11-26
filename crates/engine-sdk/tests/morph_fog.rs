//! Morph vertex meshes and distance fog through the SDK on NullBackend.
#![allow(clippy::result_large_err)]

use engine_sdk::prelude::*;
use engine_sdk::{
    DistanceFog, Engine, EngineConfig, MeshHandle, NullBackend, RenderError, RenderItem, Runtime,
    VertexPosNormalUvMorph, WorldPos,
};

fn vtx() -> VertexPosNormalUvMorph {
    VertexPosNormalUvMorph {
        position: [0.0; 3],
        normal: [0.0, 1.0, 0.0],
        uv: [0.0; 2],
        morph_position: [0.0, 1.0, 0.0],
        morph_uv: [1.0, 1.0],
    }
}

#[derive(Default)]
struct MorphApp {
    mesh: Option<MeshHandle>,
    empty_error: Option<bool>,
    morph: f32,
}

impl App for MorphApp {
    fn setup(&mut self, r: &mut Resources<'_>, s: &mut SceneState) -> Result<(), SdkError> {
        self.empty_error = Some(matches!(
            r.create_lit_morph_vertices("empty", &[]),
            Err(SdkError::Render(RenderError::EmptyMesh))
        ));
        let ib = r.create_index_buffer("ib", &[0, 1, 2, 2, 1, 3])?;
        let m = r
            .create_lit_morph_vertices("m", &[vtx(); 4])?
            .with_index_range(&ib, 3, 3)?;
        let pos = WorldPos::from_xyz(s.frames.root(), 0.0, 0.0, -5.0).unwrap();
        s.items.push(RenderItem::new(m, pos).with_morph(self.morph));
        s.fog = DistanceFog {
            color: [0.5, 0.6, 0.7],
            density: 0.01,
        };
        self.mesh = Some(m);
        Ok(())
    }

    fn update(&mut self, _dt: f64, _s: &mut SceneState) -> AppControl {
        AppControl::Continue
    }
}

fn runtime() -> Runtime<NullBackend> {
    Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).expect("runtime")
}

#[test]
fn morph_mesh_has_layout_count_and_draws_with_a_pipeline() {
    let mut rt = runtime();
    let mut app = MorphApp {
        morph: 0.5,
        ..MorphApp::default()
    };
    rt.setup(&mut app).unwrap();
    assert_eq!(app.empty_error, Some(true));
    let m = app.mesh.unwrap();
    assert_eq!(format!("{:?}", m.layout), "PosNormalUvMorph");
    assert_eq!(m.vertex_count, 4);
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    let packet = rt.backend().last_frame().unwrap();
    let draw = &packet.passes[0].draws[0];
    assert_eq!(draw.morph, 0.5);
    assert_eq!(format!("{:?}", draw.mesh.layout), "PosNormalUvMorph");
}

#[test]
fn scene_fog_defaults_off_and_reaches_the_packet() {
    assert_eq!(SceneState::new().fog, DistanceFog::default());
    assert_eq!(SceneState::new().fog.density, 0.0);
    let mut rt = runtime();
    let mut app = MorphApp::default();
    rt.setup(&mut app).unwrap();
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    let packet = rt.backend().last_frame().unwrap();
    assert_eq!(packet.fog, rt.scene().fog);
    assert_eq!(packet.fog.density, 0.01);
}
