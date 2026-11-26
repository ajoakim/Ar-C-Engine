//! Planetary reference app (orbit-to-surface flight) on the public SDK.
//!
//! Extension crate (tier 8). Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! Engine crates under crates/ must never depend on this crate.

pub mod descent;
pub mod stream_bench;
pub mod telemetry;

use std::collections::BTreeSet;

use engine_sdk::tracing;
use engine_sdk::{App, AppControl, DVec3, InputEvent, Resources, SceneState, SdkError};
use planetary_core::lod::View;
use planetary_core::{CubeSphere, HexSphere, TerrainParams, Topology};
use planetary_terrain::{
    CameraParams, PlanetaryCamera, StreamBudget, TAN_HALF_FOV_Y, TerrainConfig, TerrainReport,
    TerrainSystem,
};

pub use descent::DescentScript;
pub use telemetry::{
    ALTITUDE_BANDS, Clock, FrameSample, PerfSettings, PerfSummary, SimClock, Telemetry, WallClock,
    band_of, spin_for_ms, summarize,
};

/// Planet topology flown by the app.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TopologyKind {
    /// Icosahedral hexagonal cell hierarchy (default, ADR-0011).
    #[default]
    Hex,
    /// Cube-sphere quadtree (reference / fallback).
    Cube,
}

impl TopologyKind {
    /// Flag value (`hex` or `cube`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Hex => "hex",
            Self::Cube => "cube",
        }
    }

    /// Parses a flag value; anything but `hex` or `cube` is `None`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "hex" => Some(Self::Hex),
            "cube" => Some(Self::Cube),
            _ => None,
        }
    }
}

/// Planetary app settings.
#[derive(Clone, Debug, PartialEq)]
pub struct AppSettings {
    /// Topology of the planet (default hex).
    pub topology: TopologyKind,
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
    /// Adapt the per-frame streaming caps to the measured prepare plus gpu_frame cost
    /// (default off; the flight example turns it on). Needs a real [`Clock`].
    pub adaptive_streaming: bool,
    /// Compute the terrain churn counters (flicker diagnosis; default off, see
    /// `TerrainConfig::diagnostics`).
    pub diagnostics: bool,
}

impl AppSettings {
    /// Cube-sphere settings (32 cells per edge), the reference topology.
    #[must_use]
    pub fn cube_default() -> Self {
        Self {
            topology: TopologyKind::Cube,
            terrain: TerrainConfig {
                cells_per_edge: 32,
                ..TerrainConfig::default()
            },
            ..Self::default()
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            topology: TopologyKind::Hex,
            terrain: TerrainConfig {
                cells_per_edge: 16,
                ..TerrainConfig::default()
            },
            camera: CameraParams::default(),
            start_altitude: TerrainParams::default().radius,
            viewport_height: 720,
            viewport_width: 1280,
            frustum_culling: true,
            script: None,
            perf: PerfSettings::default(),
            adaptive_streaming: false,
            diagnostics: false,
        }
    }
}

/// Level of the cell in `set` (ascending) containing `dir`, searching all levels.
fn level_in<T: Topology>(topo: &T, set: &[T::Cell], dir: DVec3, max_level: u8) -> Option<u8> {
    (0..=max_level).find(|&l| set.binary_search(&topo.locate(dir, l)).is_ok())
}

/// Cut validity: every split cell has its required splits split, every root is split or
/// drawn, and every drawn cell is a piece cell of the split set.
fn cut_closed<T: Topology>(topo: &T, split: &BTreeSet<T::Cell>, drawn: &[T::Cell]) -> bool {
    let mut buf = Vec::new();
    for &c in split {
        buf.clear();
        topo.required_splits(c, &mut buf);
        if buf.iter().any(|r| !split.contains(r)) {
            return false;
        }
    }
    let drawn_set: BTreeSet<T::Cell> = drawn.iter().copied().collect();
    if !topo
        .roots()
        .iter()
        .all(|r| split.contains(r) || drawn_set.contains(r))
    {
        return false;
    }
    let mut pieces = Vec::new();
    topo.pieces(split, &mut pieces);
    // Pieces of cells that are not resident are skipped by the effective cut, so the drawn
    // cells must be a subset of the pieces' cells and every drawn cell must belong to a piece.
    let piece_cells: BTreeSet<T::Cell> = pieces.iter().map(|p| p.cell).collect();
    drawn_set.is_subset(&piece_cells)
}

/// Topology dispatch over the two terrain systems.
enum AnyTerrain {
    Hex(TerrainSystem<HexSphere>),
    Cube(TerrainSystem<CubeSphere>),
}

macro_rules! each {
    ($self:expr, $t:ident => $e:expr) => {
        match $self {
            AnyTerrain::Hex($t) => $e,
            AnyTerrain::Cube($t) => $e,
        }
    };
}

#[allow(clippy::result_large_err)] // SdkError is the SDK-wide error type.
impl AnyTerrain {
    fn report(&self) -> &TerrainReport {
        each!(self, t => t.report())
    }
    fn surface_radius_at(&self, d: DVec3) -> f64 {
        each!(self, t => t.surface_radius_at(d))
    }
    fn drawn_error_at(&self, d: DVec3) -> f64 {
        each!(self, t => t.drawn_error_at(d))
    }
    fn setup(&mut self, r: &mut Resources<'_>, s: &mut SceneState) -> Result<(), SdkError> {
        each!(self, t => t.setup(r, s))
    }
    fn prepare(&mut self, v: &View) {
        each!(self, t => t.prepare(v));
    }
    fn gpu_frame(&mut self, r: &mut Resources<'_>, s: &mut SceneState) -> Result<(), SdkError> {
        each!(self, t => t.gpu_frame(r, s))
    }
    fn held(&self) -> usize {
        each!(self, t => t.held_mesh_count())
    }
    fn set_frame_cost(&mut self, ms: f64) {
        each!(self, t => t.set_frame_cost(ms));
    }
    fn wait_for_workers(&mut self) {
        each!(self, t => t.wait_for_workers());
    }
    fn stream_budget(&self) -> StreamBudget {
        each!(self, t => t.stream_budget())
    }
    fn shutdown(self) {
        each!(self, t => t.shutdown());
    }
    fn drawn_raw(&self) -> Vec<u64> {
        each!(self, t => t.drawn().iter().map(|&c| t.topology().raw(c)).collect())
    }
    fn wanted_raw(&self) -> Vec<u64> {
        each!(self, t => t.wanted_leaves().iter().map(|&c| t.topology().raw(c)).collect())
    }
    fn drawn_levels(&self) -> Vec<u8> {
        each!(self, t => t.drawn().iter().map(|&c| t.topology().level(c)).collect())
    }
    fn wanted_levels(&self) -> Vec<u8> {
        each!(self, t => t.wanted_leaves().iter().map(|&c| t.topology().level(c)).collect())
    }
    fn level_at(&self, d: DVec3, wanted: bool) -> Option<u8> {
        each!(self, t => {
            let max = t.config().lod.max_level;
            let set = if wanted { t.wanted_leaves() } else { t.drawn() };
            level_in(&**t.topology(), set, d, max)
        })
    }
    fn cut_closed(&self) -> bool {
        each!(self, t => cut_closed(&**t.topology(), t.drawn_split(), t.drawn()))
    }
}

/// Flies the planetary camera over streamed terrain.
pub struct PlanetaryApp {
    kind: TopologyKind,
    script: Option<DescentScript>,
    terrain: AnyTerrain,
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
    adaptive_streaming: bool,
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
        let mut terrain_cfg = settings.terrain.clone();
        terrain_cfg.adaptive |= settings.adaptive_streaming;
        terrain_cfg.diagnostics |= settings.diagnostics;
        let n = settings.terrain.cells_per_edge;
        let terrain = match settings.topology {
            TopologyKind::Hex => {
                let topo = HexSphere::new(n).ok_or_else(|| {
                    SdkError::App(format!("invalid hex cells_per_edge {n} (even, 2..=64)"))
                })?;
                AnyTerrain::Hex(TerrainSystem::new(terrain_cfg, topo)?)
            }
            TopologyKind::Cube => {
                AnyTerrain::Cube(TerrainSystem::new(terrain_cfg, CubeSphere::new(n))?)
            }
        };
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
            kind: settings.topology,
            nominal_radius: radius,
            telemetry,
            frames: 0,
            exit_after_frames: settings.perf.exit_after_frames,
            adaptive_streaming: settings.adaptive_streaming,
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

    /// Topology in use.
    #[must_use]
    pub fn topology_kind(&self) -> TopologyKind {
        self.kind
    }

    /// Planet radius plus terrain height in direction `dir`.
    #[must_use]
    pub fn surface_radius_at(&self, dir: DVec3) -> f64 {
        self.terrain.surface_radius_at(dir)
    }

    /// Raw ids of the drawn cells, ascending.
    #[must_use]
    pub fn drawn_raw(&self) -> Vec<u64> {
        self.terrain.drawn_raw()
    }

    /// Levels of the drawn cells (same order as [`PlanetaryApp::drawn_raw`]).
    #[must_use]
    pub fn drawn_levels(&self) -> Vec<u8> {
        self.terrain.drawn_levels()
    }

    /// Raw ids of the wanted leaves, ascending.
    #[must_use]
    pub fn wanted_raw(&self) -> Vec<u64> {
        self.terrain.wanted_raw()
    }

    /// Levels of the wanted leaves (same order as [`PlanetaryApp::wanted_raw`]).
    #[must_use]
    pub fn wanted_levels(&self) -> Vec<u8> {
        self.terrain.wanted_levels()
    }

    /// Level of the drawn cell containing `dir`.
    #[must_use]
    pub fn piece_level_at(&self, dir: DVec3) -> Option<u8> {
        self.terrain.level_at(dir, false)
    }

    /// Level of the wanted leaf containing `dir`.
    #[must_use]
    pub fn wanted_level_at(&self, dir: DVec3) -> Option<u8> {
        self.terrain.level_at(dir, true)
    }

    /// True when the drawn cut is closed: no drawn cell under another, every split cell has its
    /// required splits split, every root covered.
    #[must_use]
    pub fn cut_closed(&self) -> bool {
        self.terrain.cut_closed()
    }

    /// Drawn cells form an exact partition (cube: leaf partition; hex: closed cut, see
    /// [`PlanetaryApp::cut_closed`]; hex exact coverage is proven in planetary-core).
    #[must_use]
    pub fn drawn_is_partition(&self) -> bool {
        match &self.terrain {
            AnyTerrain::Cube(t) => planetary_core::lod::is_partition(t.drawn()),
            AnyTerrain::Hex(_) => self.terrain.cut_closed(),
        }
    }

    /// Vertex buffers held by the terrain.
    #[must_use]
    pub fn held_mesh_count(&self) -> usize {
        self.terrain.held()
    }

    /// Current adaptive streaming caps.
    #[must_use]
    pub fn stream_budget(&self) -> StreamBudget {
        self.terrain.stream_budget()
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

    /// BLOCKING: waits for the terrain jobs started so far (bench and test pacing only).
    pub fn wait_for_workers(&mut self) {
        self.terrain.wait_for_workers();
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
            level_changes: r.level_changes,
            drawn_appeared: r.drawn_appeared,
            drawn_disappeared: r.drawn_disappeared,
            cull_toggles: r.cull_toggles,
            overlap_pairs: r.overlap_pairs,
            flipflops: r.flipflops,
            flipflops_unexplained: r.flipflops_unexplained,
            ..FrameSample::default()
        };
        self.telemetry.record(sample);
        if self.adaptive_streaming
            && let Some(last) = self.telemetry.last()
        {
            // The measured cost of this frame sets the caps of the next one.
            self.terrain
                .set_frame_cost(last.prepare_ms + last.gpu_frame_ms);
        }
        self.frames += 1;
        result
    }
}
