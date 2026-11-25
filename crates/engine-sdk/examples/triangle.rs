//! Manual smoke test: `cargo run -p engine-sdk --example triangle`.
//! Expect a window with an RGB triangle rotating; WASD moves the camera; key/mouse events
//! are logged; Escape or closing the window exits.
#![allow(clippy::result_large_err)]

use engine_sdk::prelude::*;
use engine_sdk::{AppConfig, MeshHandle, WindowConfig, run_app, tracing};

#[derive(Default)]
struct TriangleApp {
    mesh: Option<MeshHandle>,
    angle: f32,
    velocity: DVec3,
    quit: bool,
}

fn vertex(position: [f32; 3], color: [f32; 3]) -> VertexPosColor {
    VertexPosColor { position, color }
}

impl App for TriangleApp {
    fn setup(&mut self, res: &mut Resources<'_>, scene: &mut SceneState) -> Result<(), SdkError> {
        let mesh = res.create_mesh(
            "triangle",
            &[
                vertex([-0.5, -0.4, 0.0], [1.0, 0.0, 0.0]),
                vertex([0.5, -0.4, 0.0], [0.0, 1.0, 0.0]),
                vertex([0.0, 0.5, 0.0], [0.0, 0.0, 1.0]),
            ],
        )?;
        self.mesh = Some(mesh);
        let root = scene.frames.root();
        // -Z is forward (ADR-0001), so the triangle at z = 0 is in front of the camera at z = 2.
        scene.camera = Camera::new(WorldPos::from_xyz(root, 0.0, 6.4e6, 2.0)?);
        scene.items.push(RenderItem::new(
            mesh,
            WorldPos::from_xyz(root, 0.0, 6.4e6, 0.0)?,
        ));
        Ok(())
    }

    fn input(&mut self, event: &InputEvent, _scene: &mut SceneState) {
        tracing::info!(?event, "input");
        match *event {
            InputEvent::KeyPressed { key, .. } => match key {
                KeyCode::W => self.velocity.z = -1.0,
                KeyCode::S => self.velocity.z = 1.0,
                KeyCode::A => self.velocity.x = -1.0,
                KeyCode::D => self.velocity.x = 1.0,
                KeyCode::Escape => self.quit = true,
                _ => {}
            },
            InputEvent::KeyReleased { key } => match key {
                KeyCode::W | KeyCode::S => self.velocity.z = 0.0,
                KeyCode::A | KeyCode::D => self.velocity.x = 0.0,
                _ => {}
            },
            _ => {}
        }
    }

    fn update(&mut self, dt: f64, scene: &mut SceneState) -> AppControl {
        if self.quit {
            return AppControl::Exit;
        }
        match scene.camera.position.translated(self.velocity * dt) {
            Ok(p) => scene.camera.position = p,
            Err(e) => tracing::warn!(error = %e, "camera move rejected"),
        }
        self.angle += dt as f32;
        if let Some(item) = scene.items.first_mut() {
            item.rotation = Quat::from_rotation_y(self.angle);
        }
        AppControl::Continue
    }
}

fn main() -> Result<(), SdkError> {
    run_app(
        TriangleApp::default(),
        AppConfig {
            window: WindowConfig {
                title: "engine triangle".into(),
                ..Default::default()
            },
            ..Default::default()
        },
    )
}
