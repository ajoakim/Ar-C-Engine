//! Headless lit-frame tests on NullBackend (SDK API only).
#![allow(clippy::result_large_err)]

use engine_sdk::prelude::*;
use engine_sdk::{Engine, EngineConfig, MeshHandle, NullBackend, Runtime};

fn quad(res: &mut Resources<'_>) -> Result<MeshHandle, SdkError> {
    let v = |p: [f32; 3], uv: [f32; 2]| VertexPosNormalUv {
        position: p,
        normal: [0.0, 0.0, 1.0],
        uv,
    };
    res.create_lit_mesh(
        "quad",
        &[
            v([-0.5, -0.5, 0.0], [0.0, 1.0]),
            v([0.5, -0.5, 0.0], [1.0, 1.0]),
            v([0.5, 0.5, 0.0], [1.0, 0.0]),
            v([-0.5, 0.5, 0.0], [0.0, 0.0]),
        ],
        &[0, 1, 2, 0, 2, 3],
    )
}

#[derive(Default)]
struct LitApp {
    mixed: bool,
    texture: Option<engine_sdk::TextureId>,
}

impl App for LitApp {
    fn setup(&mut self, res: &mut Resources<'_>, scene: &mut SceneState) -> Result<(), SdkError> {
        let root = scene.frames.root();
        scene.camera = Camera::new(WorldPos::from_xyz(root, 0.0, 6.4e6, 2.0)?);
        let mesh = quad(res)?;
        let tex = res.create_texture("checker", 2, 2, &[200; 16])?;
        self.texture = Some(tex);
        let mat = scene.add_material(Material {
            base_color: [0.9, 0.5, 0.2, 1.0],
            base_color_texture: Some(tex),
        })?;
        scene.light = DirectionalLight {
            direction: Vec3::new(0.0, -1.0, 0.0),
            color: [1.0; 3],
            intensity: 1.5,
            ambient: [0.1; 3],
        };
        scene.items.push(
            RenderItem::new(mesh, WorldPos::from_xyz(root, 0.0, 6.4e6, 0.0)?).with_material(mat),
        );
        if self.mixed {
            let v = |p: [f32; 3]| VertexPosColor {
                position: p,
                color: [1.0; 3],
            };
            let tri = res.create_mesh(
                "tri",
                &[v([-0.5, 0.0, 0.0]), v([0.5, 0.0, 0.0]), v([0.0, 0.5, 0.0])],
            )?;
            scene.items.push(RenderItem::new(
                tri,
                WorldPos::from_xyz(root, 1.0, 6.4e6, 0.0)?,
            ));
        }
        Ok(())
    }

    fn update(&mut self, _dt: f64, _scene: &mut SceneState) -> AppControl {
        AppControl::Continue
    }
}

fn runtime() -> Runtime<NullBackend> {
    Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).unwrap()
}

#[test]
fn headless_lit_frame_carries_material_texture_light() {
    let mut rt = runtime();
    let mut app = LitApp::default();
    rt.setup(&mut app).unwrap();
    rt.frame(&mut app, 0.016).unwrap();
    let frame = rt.backend().last_frame().unwrap();
    let draws = &frame.passes[0].draws;
    assert_eq!(draws.len(), 1);
    assert_eq!(draws[0].texture, app.texture);
    assert_eq!(draws[0].base_color, [0.9, 0.5, 0.2, 1.0]);
    assert_eq!(frame.light, rt.scene().light);
    assert!(draws[0].mvp.is_finite());
}

#[test]
fn headless_mixed_scene_uses_two_pipelines() {
    let mut rt = runtime();
    let mut app = LitApp {
        mixed: true,
        ..Default::default()
    };
    rt.setup(&mut app).unwrap();
    rt.frame(&mut app, 0.016).unwrap();
    let draws = &rt.backend().last_frame().unwrap().passes[0].draws;
    assert_eq!(draws.len(), 2);
    assert_ne!(draws[0].pipeline, draws[1].pipeline);
    assert_eq!(rt.backend().pipeline_count(), 2);
}
