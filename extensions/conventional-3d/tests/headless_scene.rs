//! Full headless frames of the conventional scene on NullBackend (SDK API only).
#![allow(clippy::result_large_err)]

use conventional_3d::procedural::{plane_mesh, procedural_cube_model};
use conventional_3d::{
    ImportLimits, ImportedModel, ImportedTexture, ModelAssets, ModelLoader, SceneBuilder,
    upload_model, upload_registered,
};
use engine_sdk::prelude::*;
use engine_sdk::{Engine, EngineConfig, EntityId, NullBackend, Runtime};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/textured_cube.glb");

struct TestScene {
    model: ImportedModel,
    via_registry: bool,
    builder: Option<SceneBuilder>,
}

impl TestScene {
    fn new(model: ImportedModel, via_registry: bool) -> Self {
        Self {
            model,
            via_registry,
            builder: None,
        }
    }
}

impl App for TestScene {
    fn setup(&mut self, res: &mut Resources<'_>, scene: &mut SceneState) -> Result<(), SdkError> {
        let model = self.model.clone();
        let (uploaded, nodes) = if self.via_registry {
            let mut assets = ModelAssets::new();
            let reg = assets
                .register("test/model", model)
                .map_err(|e| SdkError::App(e.to_string()))?;
            (upload_registered(res, scene, &assets, &reg)?, reg.nodes)
        } else {
            (upload_model(res, scene, &model)?, model.nodes.clone())
        };
        let ground_mesh = plane_mesh(20.0);
        let ground = res.create_lit_mesh("ground", &ground_mesh.vertices, &ground_mesh.indices)?;
        let gray = scene.add_material(Material {
            base_color: [0.5, 0.5, 0.5, 1.0],
            base_color_texture: None,
        })?;
        let mut b = SceneBuilder::new(scene.frames.root());
        b.spawn_mesh(ground, gray, Transform::IDENTITY)?;
        for x in -1..=1 {
            for z in -1..=1 {
                let t = Transform::from_translation(DVec3::new(
                    f64::from(x) * 3.0,
                    0.5,
                    f64::from(z) * 3.0,
                ));
                b.spawn_nodes(&nodes, &uploaded, t)?;
            }
        }
        self.builder = Some(b);
        Ok(())
    }

    fn update(&mut self, _dt: f64, scene: &mut SceneState) -> AppControl {
        if let Some(b) = self.builder.as_mut() {
            let _ = b.flush(&mut scene.items);
        }
        AppControl::Continue
    }
}

fn runtime() -> Runtime<NullBackend> {
    Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).unwrap()
}

fn run_one(app: &mut TestScene) -> Runtime<NullBackend> {
    let mut rt = runtime();
    rt.setup(app).unwrap();
    rt.frame(app, 1.0 / 60.0).unwrap();
    rt
}

#[test]
fn headless_procedural_scene_frame() {
    let mut app = TestScene::new(procedural_cube_model(), false);
    let rt = run_one(&mut app);
    let frame = rt.backend().last_frame().unwrap();
    let draws = &frame.passes[0].draws;
    assert_eq!(draws.len(), 10);
    assert!(draws.iter().all(|d| d.pipeline == draws[0].pipeline));
    let textured = draws.iter().filter(|d| d.texture.is_some()).count();
    let untextured = draws.iter().filter(|d| d.texture.is_none()).count();
    assert_eq!((textured, untextured), (9, 1));
    assert_eq!(frame.light, rt.scene().light);
}

#[test]
fn headless_registry_scene_frame() {
    let mut app = TestScene::new(procedural_cube_model(), true);
    let rt = run_one(&mut app);
    let draws = &rt.backend().last_frame().unwrap().passes[0].draws;
    assert_eq!(draws.len(), 10);
    assert_eq!(draws.iter().filter(|d| d.texture.is_some()).count(), 9);
}

#[test]
fn headless_glb_scene_frame() {
    let mut loader = ModelLoader::new(1).unwrap();
    let t = loader.load(FIXTURE, ImportLimits::default());
    let model = loader.wait(t).unwrap();
    let mut app = TestScene::new(model, true);
    let rt = run_one(&mut app);
    let draws = &rt.backend().last_frame().unwrap().passes[0].draws;
    assert!(!draws.is_empty());
    let cubes: Vec<_> = draws.iter().filter(|d| d.texture.is_some()).collect();
    assert_eq!(cubes.len(), 9);
    for d in cubes {
        assert_eq!(d.mesh.index.map(|i| i.count), Some(36));
    }
}

struct Probe(Option<(SceneBuilder, EntityIds)>);
type EntityIds = (EntityId, EntityId);

impl App for Probe {
    fn setup(&mut self, res: &mut Resources<'_>, scene: &mut SceneState) -> Result<(), SdkError> {
        let mut loader = ModelLoader::new(1).map_err(|e| SdkError::App(e.to_string()))?;
        let t = loader.load(FIXTURE, ImportLimits::default());
        let model = loader.wait(t).map_err(|e| SdkError::App(e.to_string()))?;
        let up = upload_model(res, scene, &model)?;
        let mut b = SceneBuilder::new(scene.frames.root());
        let root = b.spawn_model(
            &model,
            &up,
            Transform::from_translation(DVec3::new(1.0e6, 0.0, 0.0)),
        )?;
        let m = up.meshes[0];
        let solo = b.spawn_mesh(m.0, m.1, Transform::IDENTITY)?;
        self.0 = Some((b, (root, solo)));
        Ok(())
    }
    fn update(&mut self, _dt: f64, _scene: &mut SceneState) -> AppControl {
        AppControl::Continue
    }
}

#[test]
fn hierarchy_positions_f64() {
    let mut rt = runtime();
    let mut app = Probe(None);
    rt.setup(&mut app).unwrap();
    let (mut b, (_root, solo)) = app.0.take().unwrap();
    let mut items = Vec::new();
    b.flush(&mut items).unwrap();
    // Child node of the fixture sits at (0, 0.5, 0) under the root at 1.0e6.
    assert_eq!(items[0].position.local(), DVec3::new(1.0e6, 0.5, 0.0));

    // flush_reflects_set_local: drawable_count unchanged, position updated.
    let count = b.drawable_count();
    b.set_local(solo, Transform::from_translation(DVec3::new(7.0, 8.0, 9.0)))
        .unwrap();
    b.flush(&mut items).unwrap();
    assert_eq!(b.drawable_count(), count);
    assert_eq!(items.len(), count);
    assert_eq!(items[1].position.local(), DVec3::new(7.0, 8.0, 9.0));
}

#[test]
fn upload_model_rejects_bad_indices() {
    let mut model = procedural_cube_model();
    model.materials[0].texture = Some(99);
    let mut app = TestScene::new(model, false);
    let mut rt = runtime();
    assert!(matches!(rt.setup(&mut app), Err(SdkError::App(_))));

    let mut model = procedural_cube_model();
    model.meshes[0].material = Some(42);
    model.textures = Vec::<ImportedTexture>::new();
    model.materials.clear();
    let mut app = TestScene::new(model, false);
    let mut rt = runtime();
    assert!(matches!(rt.setup(&mut app), Err(SdkError::App(_))));
}
