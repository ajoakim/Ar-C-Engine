//! Conventional 3D reference app: a lit, textured glTF scene with a directional light
//! and a fly camera, built on the SDK only.
//!
//! Controls: WASD/arrows move, E/Space up, Q/Ctrl down, Shift boost,
//! hold right mouse button to look, Esc quits.
//!
//! Usage: `basic_scene [--model <path.glb>] [--headless <frames>]`
#![allow(clippy::result_large_err)]

use std::path::PathBuf;

use conventional_3d::procedural::{plane_mesh, procedural_cube_model};
use conventional_3d::{
    FlyCamera, ImportLimits, ModelAssets, ModelLoader, SceneBuilder, upload_registered,
};
use engine_sdk::prelude::*;
use engine_sdk::tracing;
use engine_sdk::{Engine, EngineConfig, EntityId, NullBackend, Runtime, WindowConfig};

const USAGE: &str = "usage: basic_scene [--model <path>] [--headless <frames>]";

struct BasicScene {
    loader: Option<ModelLoader>,
    model_path: PathBuf,
    builder: Option<SceneBuilder>,
    fly: FlyCamera,
    spinner: Option<EntityId>,
    angle: f32,
    base: DVec3,
    quit: bool,
}

impl BasicScene {
    fn new(model_path: PathBuf) -> Self {
        Self {
            loader: None,
            model_path,
            builder: None,
            fly: FlyCamera::new(),
            spinner: None,
            angle: 0.0,
            base: DVec3::new(1.0e6, 0.0, 0.0),
            quit: false,
        }
    }
}

impl App for BasicScene {
    fn setup(&mut self, res: &mut Resources<'_>, scene: &mut SceneState) -> Result<(), SdkError> {
        let mut loader = ModelLoader::new(1).map_err(|e| SdkError::App(e.to_string()))?;
        let ticket = loader.load(&self.model_path, ImportLimits::default());
        // Resources exist only during setup (ADR-0007), so block on the import job here.
        let model = match loader.wait(ticket) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(error = %e, "model import failed, using procedural cube");
                procedural_cube_model()
            }
        };
        self.loader = Some(loader);

        // Route through the asset registry, then upload from the registered handles.
        let mut assets = ModelAssets::new();
        let registered = assets
            .register("basic_scene/model", model)
            .map_err(|e| SdkError::App(e.to_string()))?;
        let uploaded = upload_registered(res, scene, &assets, &registered)?;

        let plane = plane_mesh(40.0);
        let ground_mesh = res.create_lit_mesh("ground", &plane.vertices, &plane.indices)?;
        let ground_mat = scene.add_material(Material {
            base_color: [0.35, 0.37, 0.4, 1.0],
            base_color_texture: None,
        })?;

        let base = self.base;
        let mut builder = SceneBuilder::new(scene.frames.root());
        builder.spawn_mesh(ground_mesh, ground_mat, Transform::from_translation(base))?;
        for x in -1i32..=1 {
            for z in -1i32..=1 {
                let pos = base + DVec3::new(f64::from(x) * 3.0, 0.5, f64::from(z) * 3.0);
                let yaw = (x * 3 + z + 4) as f32 * 0.35;
                let t = Transform::new(pos, Quat::from_rotation_y(yaw), Vec3::ONE);
                let root = builder.spawn_nodes(&registered.nodes, &uploaded, t)?;
                if x == 0 && z == 0 {
                    self.spinner = Some(root);
                }
            }
        }
        self.builder = Some(builder);

        scene.light = DirectionalLight {
            direction: Vec3::new(-0.5, -1.0, -0.3).normalize(),
            color: [1.0, 0.97, 0.9],
            intensity: 1.0,
            ambient: [0.15, 0.15, 0.18],
        };
        scene.clear_color = [0.45, 0.6, 0.8, 1.0];
        let cam_pos = WorldPos::new(scene.frames.root(), base + DVec3::new(0.0, 2.5, 9.0))?;
        scene.camera = Camera::new(cam_pos);
        self.fly.pitch = -0.2;
        self.fly.update(0.0, &mut scene.camera)?;
        Ok(())
    }

    fn input(&mut self, event: &InputEvent, _scene: &mut SceneState) {
        self.fly.handle_input(event);
        match event {
            InputEvent::KeyPressed {
                key: KeyCode::Escape,
                ..
            }
            | InputEvent::CloseRequested => self.quit = true,
            _ => {}
        }
    }

    fn update(&mut self, dt: f64, scene: &mut SceneState) -> AppControl {
        if self.quit {
            return AppControl::Exit;
        }
        if let Err(e) = self.fly.update(dt, &mut scene.camera) {
            tracing::warn!(error = %e, "fly camera update failed");
        }
        self.angle += dt as f32;
        if let (Some(builder), Some(spinner)) = (self.builder.as_mut(), self.spinner) {
            let t = Transform::new(
                self.base + DVec3::new(0.0, 0.5, 0.0),
                Quat::from_rotation_y(self.angle),
                Vec3::ONE,
            );
            if let Err(e) = builder.set_local(spinner, t) {
                tracing::warn!(error = %e, "spinner update failed");
            }
        }
        if let Some(builder) = self.builder.as_mut()
            && let Err(e) = builder.flush(&mut scene.items)
        {
            tracing::warn!(error = %e, "scene flush failed");
        }
        AppControl::Continue
    }
}

fn main() -> Result<(), SdkError> {
    let mut model_path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/textured_cube.glb"
    ));
    let mut headless: Option<u32> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--headless" => {
                let n = args
                    .next()
                    .and_then(|v| v.parse::<u32>().ok())
                    .ok_or_else(|| SdkError::App(USAGE.into()))?;
                headless = Some(n);
            }
            "--model" => {
                let p = args.next().ok_or_else(|| SdkError::App(USAGE.into()))?;
                model_path = PathBuf::from(p);
            }
            _ => return Err(SdkError::App(USAGE.into())),
        }
    }

    let mut app = BasicScene::new(model_path);
    if let Some(n) = headless {
        let mut rt = Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new())?;
        rt.setup(&mut app)?;
        for _ in 0..n {
            rt.frame(&mut app, 1.0 / 60.0)?;
        }
        let draws = rt
            .backend()
            .last_frame()
            .map_or(0, |f| f.passes.iter().map(|p| p.draws.len()).sum::<usize>());
        let submitted = rt.backend().submitted_count();
        println!(
            "basic_scene headless: {n} frames, {draws} draws in last frame, {submitted} submits"
        );
        rt.shutdown()?;
        if draws == 0 {
            return Err(SdkError::App("headless run produced zero draws".into()));
        }
        return Ok(());
    }
    run_app(
        app,
        AppConfig {
            window: WindowConfig {
                title: "conventional-3d basic scene".into(),
                ..Default::default()
            },
            ..Default::default()
        },
    )
}
