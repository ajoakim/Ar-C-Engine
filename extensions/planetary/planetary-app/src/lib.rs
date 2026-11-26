//! Planetary reference app (orbit-to-surface flight) on the public SDK.
//!
//! Extension crate (tier 8). Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! Engine crates under crates/ must never depend on this crate.

pub mod descent;
pub mod stream_bench;
pub mod telemetry;

use engine_sdk::tracing;
use engine_sdk::{App, AppControl, DVec3, InputEvent, Resources, SceneState, SdkError};
use planetary_core::lod::View;
use planetary_core::{CubeSphere, TerrainParams};
use planetary_terrain::{
    CameraParams, PlanetaryCamera, TAN_HALF_FOV_Y, TerrainConfig, TerrainReport, TerrainSystem,
};

pub use descent::DescentScript;
pub use telemetry::{
    ALTITUDE_BANDS, Clock, FrameSample, PerfSettings, PerfSummary, SimClock, Telemetry, WallClock,
    band_of, spin_for_ms, summarize,
};

/// Planetary app settings.
#[derive(Clone, Debug, PartialEq)]
pub struct AppSettings {
    pub terrain: TerrainConfig,
    pub camera: CameraParams,
    /// Initial altitude above the nominal radius for the interactive camera.
    pub start_altitude: f64,
    pub viewport_height: u32,
    /// Initial viewport width (updated from resize events).
    pub viewport_width: u32,
    /// Attach the camera frustum to the selection view (default true).
    pub frustum_culling: bool,
    /// Scripted descent; `None` means the interactive camera.
    pub script: Option<DescentScript>,
    /// Frame telemetry options (all off by default).
    pub perf: PerfSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            terrain: TerrainConfig::default(),
            camera: CameraParams::default(),
            start_altitude: TerrainParams::default().radius,
            viewport_height: 720,
            viewport_width: 1280,
            frustum_culling: true,
            script: None,
            perf: PerfSettings::default(),
        }
    }
}

/// Flies the planetary camera over streamed terrain.
pub struct PlanetaryApp {
    script: Option<DescentScript>,
    terrain: TerrainSystem<CubeSphere>,
    camera: PlanetaryCamera,
    viewport_h: u32,
    viewport_w: u32,
    frustum_culling: bool,
    time: f64,
    last_view: Option<View>,
    quit: bool,
    nominal_radius: f64,
    telemetry: Telemetry,
    frames: u64,
    exit_after_frames: Option<u64>,
}

impl PlanetaryApp {
    /// Builds the terrain system and the initial camera.
    ///
    /// # Errors
    /// `SdkError::App` for an invalid configuration or camera start.
    #[allow(clippy::result_large_err)] // SdkError is the SDK-wide error type.
    pub fn new(settings: AppSettings) -> Result<Self, SdkError> {
        Self::with_clock(settings, Box::new(SimClock::new(16_666_667)))
    }

    /// Like [`PlanetaryApp::new`] with an explicit telemetry clock.
    ///
    /// # Errors
    /// `SdkError::App` for an invalid configuration, camera start or perf log.
    #[allow(clippy::result_large_err)]
    pub fn with_clock(settings: AppSettings, clock: Box<dyn Clock>) -> Result<Self, SdkError> {
        let terrain = TerrainSystem::new(
            settings.terrain.clone(),
            CubeSphere::new(settings.terrain.cells_per_edge),
        )?;
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
        let telemetry = Telemetry::new(clock, &settings.perf)
            .map_err(|e| SdkError::App(format!("perf log: {e}")))?;
        Ok(Self {
            nominal_radius: radius,
            telemetry,
            frames: 0,
            exit_after_frames: settings.perf.exit_after_frames,
            script: settings.script,
            terrain,
            camera,
            viewport_h,
            viewport_w: settings.viewport_width.max(1),
            frustum_culling: settings.frustum_culling,
            time: 0.0,
            last_view: View::new(pos, viewport_h, TAN_HALF_FOV_Y),
            quit: false,
        })
    }

    #[must_use]
    pub fn terrain(&self) -> &TerrainSystem<CubeSphere> {
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

    #[must_use]
    pub fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }

    /// Frames recorded by telemetry.
    #[must_use]
    pub fn frames_recorded(&self) -> u64 {
        self.frames
    }

    /// Stops the terrain worker pool.
    pub fn shutdown(mut self) {
        self.telemetry.flush();
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
            InputEvent::Resized { width, height } if height > 0 => {
                self.viewport_h = height;
                self.viewport_w = width.max(1);
            }
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
            // Conservative aspect: never narrower than 16:9 so culling cannot clip visible edges.
            let aspect = (f64::from(self.viewport_w) / f64::from(self.viewport_h)).max(16.0 / 9.0);
            let alt = self.camera.altitude(&|d| self.terrain.surface_radius_at(d));
            let near = f64::from(PlanetaryCamera::near_plane(alt));
            self.last_view = Some(if self.frustum_culling {
                view.with_frustum(self.camera.frustum_planes(aspect, near))
            } else {
                view
            });
        }
        if let Some(view) = self.last_view {
            self.telemetry.begin_phase();
            self.terrain.prepare(&view);
            self.telemetry.end_prepare();
        }
        let frames_done = self.exit_after_frames.is_some_and(|n| self.frames >= n);
        if self.quit || self.camera.exit_requested() || frames_done {
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
        self.telemetry.measure_gpu_frame_start();
        let result = self.terrain.gpu_frame(resources, scene);
        let pos = self.camera.position();
        let len = pos.length();
        let altitude_m = if len > 0.0 {
            len - self.terrain.surface_radius_at(pos / len)
        } else {
            0.0
        };
        let r = self.terrain.report();
        let sample = FrameSample {
            frame: self.frames + 1,
            sim_time_s: self.time,
            altitude_m,
            nominal_altitude_m: len - self.nominal_radius,
            center_distance_m: len,
            nodes_visited: r.select.nodes_visited,
            leaves: r.select.leaves,
            wanted_leaves: r.wanted_leaves,
            drawn_leaves: r.drawn_leaves,
            held_meshes: r.held_meshes,
            pending: r.stream.pending,
            in_flight: r.stream.in_flight,
            awaiting_upload: r.stream.awaiting_upload,
            started: r.stream.started,
            completions: r.stream.completions_applied,
            uploads: r.stream.uploads,
            uploaded_bytes: r.stream.uploaded_bytes,
            evicted: r.stream.evicted,
            cancelled: r.stream.cancelled,
            destroyed: r.destroyed,
            upload_rejects: r.upload_rejects,
            pending_requests: r.pending_requests,
            draw_items: r.draw_items,
            culled_draws: r.culled_draws,
            tau_eff: r.select.tau_eff,
            forced_splits: r.select.forced_splits,
            trimmed_splits: r.select.trimmed_splits,
            culled_horizon: r.select.culled_horizon,
            culled_frustum: r.select.culled_frustum,
            node_budget_hit: r.select.node_budget_hit,
            leaf_budget_hit: r.select.leaf_budget_hit,
            ..FrameSample::default()
        };
        self.telemetry.record(sample);
        self.frames += 1;
        result
    }
}
