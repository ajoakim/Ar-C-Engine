//! Planetary reference app (orbit-to-surface flight) on the public SDK.
//!
//! Extension crate (tier 8). Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! Engine crates under crates/ must never depend on this crate.

pub mod descent;

use engine_sdk::tracing;
use engine_sdk::{App, AppControl, DVec3, InputEvent, Resources, SceneState, SdkError};
use planetary_core::TerrainParams;
use planetary_core::lod::View;
use planetary_terrain::{
    CameraParams, PlanetaryCamera, TAN_HALF_FOV_Y, TerrainConfig, TerrainReport, TerrainSystem,
};

pub use descent::DescentScript;

/// Planetary app settings.
#[derive(Clone, Debug, PartialEq)]
pub struct AppSettings {
    pub terrain: TerrainConfig,
    pub camera: CameraParams,
    /// Initial altitude above the nominal radius for the interactive camera.
    pub start_altitude: f64,
    pub viewport_height: u32,
    /// Scripted descent; `None` means the interactive camera.
    pub script: Option<DescentScript>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            terrain: TerrainConfig::default(),
            camera: CameraParams::default(),
            start_altitude: TerrainParams::default().radius,
            viewport_height: 720,
            script: None,
        }
    }
}

/// Flies the planetary camera over streamed terrain.
pub struct PlanetaryApp {
    script: Option<DescentScript>,
    terrain: TerrainSystem,
    camera: PlanetaryCamera,
    viewport_h: u32,
    time: f64,
    last_view: Option<View>,
    quit: bool,
}

impl PlanetaryApp {
    /// Builds the terrain system and the initial camera.
    ///
    /// # Errors
    /// `SdkError::App` for an invalid configuration or camera start.
    #[allow(clippy::result_large_err)] // SdkError is the SDK-wide error type.
    pub fn new(settings: AppSettings) -> Result<Self, SdkError> {
        let terrain = TerrainSystem::new(settings.terrain.clone())?;
        let radius = settings.terrain.terrain.radius;
        let surface = |d: DVec3| terrain.surface_radius_at(d);
        let (pos, orient) = match settings.script {
            Some(s) => {
                let (p, q) = s.pose_at(0.0, &surface);
                (p, Some(q))
            }
            None => (DVec3::new(radius + settings.start_altitude, 0.0, 0.0), None),
        };
        let mut camera = PlanetaryCamera::new(pos, settings.camera)
            .ok_or_else(|| SdkError::App("invalid camera start pose".into()))?;
        if let Some(q) = orient {
            camera.set_pose(pos, q);
        }
        let viewport_h = settings.viewport_height.max(1);
        Ok(Self {
            script: settings.script,
            terrain,
            camera,
            viewport_h,
            time: 0.0,
            last_view: View::new(pos, viewport_h, TAN_HALF_FOV_Y),
            quit: false,
        })
    }

    #[must_use]
    pub fn terrain(&self) -> &TerrainSystem {
        &self.terrain
    }

    #[must_use]
    pub fn camera(&self) -> &PlanetaryCamera {
        &self.camera
    }

    #[must_use]
    pub fn report(&self) -> &TerrainReport {
        self.terrain.report()
    }

    /// Simulated seconds accumulated by `update`.
    #[must_use]
    pub fn simulated_time(&self) -> f64 {
        self.time
    }

    /// Stops the terrain worker pool.
    pub fn shutdown(self) {
        self.terrain.shutdown();
    }

    fn apply_camera(&self, scene: &mut SceneState) {
        let alt = self.camera.altitude(&|d| self.terrain.surface_radius_at(d));
        if let Err(e) = self
            .camera
            .apply_to(&mut scene.camera, scene.frames.root(), alt)
        {
            tracing::warn!(error = %e, "camera pose rejected, keeping previous camera");
        }
    }
}

impl App for PlanetaryApp {
    fn setup(
        &mut self,
        resources: &mut Resources<'_>,
        scene: &mut SceneState,
    ) -> Result<(), SdkError> {
        self.terrain.setup(resources, scene)?;
        self.apply_camera(scene);
        Ok(())
    }

    fn input(&mut self, event: &InputEvent, _scene: &mut SceneState) {
        match *event {
            InputEvent::Resized { height, .. } if height > 0 => self.viewport_h = height,
            InputEvent::CloseRequested => self.quit = true,
            _ => {}
        }
        if self.script.is_none() {
            self.camera.handle_input(event);
        } else if matches!(
            event,
            InputEvent::KeyPressed {
                key: engine_sdk::KeyCode::Escape,
                ..
            }
        ) {
            self.quit = true;
        }
    }

    fn update(&mut self, dt: f64, scene: &mut SceneState) -> AppControl {
        if dt.is_finite() && dt > 0.0 {
            self.time += dt;
        }
        if let Some(script) = self.script {
            let (pos, rot) = script.pose_at(self.time, &|d| self.terrain.surface_radius_at(d));
            self.camera.set_pose(pos, rot);
        } else {
            let pos = self.camera.position();
            let len = pos.length();
            let extra = if len > 0.0 {
                self.terrain.drawn_error_at(pos / len)
            } else {
                0.0
            };
            let terrain = &self.terrain;
            self.camera
                .update(dt, &|d| terrain.surface_radius_at(d), extra);
        }
        self.apply_camera(scene);
        if let Some(view) = View::new(self.camera.position(), self.viewport_h, TAN_HALF_FOV_Y) {
            self.last_view = Some(view);
        }
        if let Some(view) = self.last_view {
            self.terrain.prepare(&view);
        }
        if self.quit || self.camera.exit_requested() {
            AppControl::Exit
        } else {
            AppControl::Continue
        }
    }

    fn gpu_frame(
        &mut self,
        resources: &mut Resources<'_>,
        scene: &mut SceneState,
    ) -> Result<(), SdkError> {
        self.terrain.gpu_frame(resources, scene)
    }
}
