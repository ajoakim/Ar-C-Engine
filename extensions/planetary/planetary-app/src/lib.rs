//! Planetary reference app (orbit-to-surface flight) on the public SDK.
//!
//! Extension crate (tier 8). Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! Engine crates under crates/ must never depend on this crate.

pub mod atmosphere;
pub mod descent;
pub mod free_flight;
pub mod overlay_snapshot;
pub mod stream_bench;
pub mod telemetry;
pub mod title;

use std::collections::BTreeSet;

use engine_sdk::tracing;
use engine_sdk::{
    App, AppControl, DVec3, DirectionalLight, DistanceFog, InputEvent, MAX_DRAWS_PER_FRAME,
    RenderItem, Resources, SceneState, SdkError, Vec3, WorldPos,
};
use planetary_core::lod::View;
use planetary_core::terrain::{RELIEF_SCALE_MAX, RELIEF_SCALE_MIN};
use planetary_core::{CubeSphere, HexSphere, TerrainParams, Topology};
use planetary_terrain::{
    CameraParams, PlanetaryCamera, StreamBudget, TAN_HALF_FOV_Y, TerrainConfig, TerrainReport,
    TerrainSystem,
};

pub use descent::DescentScript;
pub use free_flight::{DEFAULT_ROLL_RATE, FlightMode, FreeFlight};
pub use telemetry::{
    ALTITUDE_BANDS, Clock, FrameSample, PerfSettings, PerfSummary, SimClock, Telemetry, WallClock,
    band_of, spin_for_ms, summarize,
};

/// Altitude (m) below which the haze is at full density.
pub const HAZE_FULL_ALTITUDE: f64 = 20_000.0;
/// Altitude (m) at and above which the haze is gone (space).
pub const HAZE_NONE_ALTITUDE: f64 = 200_000.0;
/// Full haze density per metre (half haze at 100 km, see `engine_render::fog_factor`).
pub const HAZE_DENSITY: f32 = 1.0e-5;
/// Sky colour (linear RGB); the fog colour and the clear colour at full haze.
pub const SKY_COLOR: [f32; 3] = [0.30, 0.45, 0.70];
/// Clear colour of space (the scene default).
pub const SPACE_COLOR: [f32; 4] = [0.02, 0.02, 0.05, 1.0];
/// Sun elevation above the local horizon at the descent target (degrees).
pub const SUN_ELEVATION_DEG: f64 = 35.0;
/// Light intensity multiplier of the relief light.
pub const LIGHT_INTENSITY: f32 = 1.8;
/// Ambient light level (all channels). Raised from 0.025 (11-06D): slopes facing away from the sun
/// were pitch black; 0.10 keeps them readable. No golden renders with this light (the
/// atmosphere_ring golden uses `DirectionalLight::default()`).
pub const LIGHT_AMBIENT: f32 = 0.10;

/// Distance haze and sky colour for one altitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Haze {
    pub fog: DistanceFog,
    pub clear_color: [f32; 4],
}

/// Haze for the camera altitude (metres above the surface): full density up to
/// [`HAZE_FULL_ALTITUDE`], fading linearly to zero at [`HAZE_NONE_ALTITUDE`]. The clear colour
/// blends the sky colour (full haze) with space (no haze) by the same factor, so the horizon
/// fog colour equals the sky behind it. A non-finite altitude gives no haze.
#[must_use]
pub fn haze_for_altitude(altitude: f64) -> Haze {
    haze_with_sky(altitude, SKY_COLOR)
}

/// [`haze_for_altitude`] with an explicit sky colour (the atmosphere module's sun-tinted sky).
/// Density and fade are independent of the colour.
#[must_use]
pub(crate) fn haze_with_sky(altitude: f64, sky_color: [f32; 3]) -> Haze {
    let fade = if altitude.is_finite() {
        let t = (HAZE_NONE_ALTITUDE - altitude) / (HAZE_NONE_ALTITUDE - HAZE_FULL_ALTITUDE);
        t.clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let mix = |sky: f32, space: f32| sky * fade + space * (1.0 - fade);
    Haze {
        fog: DistanceFog {
            color: sky_color,
            density: HAZE_DENSITY * fade,
        },
        clear_color: [
            mix(sky_color[0], SPACE_COLOR[0]),
            mix(sky_color[1], SPACE_COLOR[1]),
            mix(sky_color[2], SPACE_COLOR[2]),
            1.0,
        ],
    }
}

/// Light travel direction (unit, from the sun towards the planet) such that the sun stands
/// [`SUN_ELEVATION_DEG`] above the local horizon at `target_dir`. The sun's azimuth is fixed by
/// the world `Y` axis (`Z` when the target is near it). Deterministic; a degenerate target gives
/// the default light direction.
#[must_use]
pub fn sun_direction_for_target(target_dir: DVec3) -> Vec3 {
    let len = target_dir.length();
    if !(len.is_finite() && len > 0.0) {
        return DirectionalLight::default().direction;
    }
    let up = target_dir / len;
    let reference = if up.y.abs() > 0.99 {
        DVec3::Z
    } else {
        DVec3::Y
    };
    let east = reference.cross(up).normalize();
    let north = up.cross(east);
    let e = SUN_ELEVATION_DEG.to_radians();
    let to_sun = up * e.sin() + north * e.cos();
    let travel = -to_sun;
    Vec3::new(travel.x as f32, travel.y as f32, travel.z as f32)
}

/// Elevation in degrees of the sun above the local horizon at `pos` for light travel `dir`.
fn elevation_deg(pos: DVec3, dir: Vec3) -> f64 {
    let len = pos.length();
    if !(len.is_finite() && len > 0.0) {
        return 0.0;
    }
    let to_sun = -DVec3::new(f64::from(dir.x), f64::from(dir.y), f64::from(dir.z));
    (pos / len).dot(to_sun).clamp(-1.0, 1.0).asin().to_degrees()
}

/// The relief light for the scripted descent target direction.
#[must_use]
pub fn relief_light(target_dir: DVec3) -> DirectionalLight {
    DirectionalLight {
        direction: sun_direction_for_target(target_dir),
        color: [1.0, 0.97, 0.9],
        intensity: LIGHT_INTENSITY,
        ambient: [LIGHT_AMBIENT; 3],
    }
}

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
    /// Altitude-faded distance haze and sky colour (default on). Off keeps the scene fog and
    /// clear colour at their defaults; the relief light is always set.
    pub haze: bool,
    /// Sun-tinted sky colour and the opaque limb ring (default on, ADR-0019). Needs `haze`; off
    /// keeps exactly the plain haze behaviour.
    pub atmosphere: bool,
}

impl AppSettings {
    /// Selects the terrain generator version (1, 2 or 3 as the example's `--generator`).
    /// Returns false (settings unchanged) for any other value.
    /// An earlier [`AppSettings::set_relief_scale`] is kept, so the two setters commute.
    pub fn set_generator(&mut self, version: u32) -> bool {
        let relief_scale = self.terrain.terrain.relief_scale;
        self.terrain.terrain = match version {
            1 => TerrainParams::default(),
            2 => TerrainParams::demo_v2(),
            3 => TerrainParams::demo_v3(),
            _ => return false,
        };
        self.terrain.terrain.relief_scale = relief_scale;
        true
    }

    /// Sets the relief exaggeration (generator v3 only; the example's `--relief`). Returns false
    /// (settings unchanged) when `scale` is not finite or outside
    /// `RELIEF_SCALE_MIN..=RELIEF_SCALE_MAX` (0.1..=8.0).
    pub fn set_relief_scale(&mut self, scale: f64) -> bool {
        if !(scale.is_finite() && (RELIEF_SCALE_MIN..=RELIEF_SCALE_MAX).contains(&scale)) {
            return false;
        }
        self.terrain.terrain.relief_scale = scale;
        true
    }

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
            haze: true,
            atmosphere: true,
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
    fn prepare_with(&mut self, v: &View, probe: &mut dyn planetary_terrain::ScopeProbe) {
        each!(self, t => t.prepare_with(v, probe));
    }
    fn gpu_frame(&mut self, r: &mut Resources<'_>, s: &mut SceneState) -> Result<(), SdkError> {
        each!(self, t => t.gpu_frame(r, s))
    }
    fn held(&self) -> usize {
        each!(self, t => t.held_mesh_count())
    }
    fn drawn_morphs(&self) -> Vec<f64> {
        each!(self, t => t.drawn().iter().map(|&c| t.drawn_morph(c).unwrap_or(0.0)).collect())
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
    #[cfg(feature = "debug-overlay")]
    fn held_mesh_cap(&self) -> u64 {
        each!(self, t => t.held_mesh_cap())
    }
    fn shutdown(self) {
        each!(self, t => t.shutdown());
    }
    fn wanted_split_raw(&self) -> Vec<u64> {
        each!(self, t => t.wanted_split().iter().map(|&c| t.topology().raw(c)).collect())
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

/// The startup readout: adapter, requested and active present mode, supported modes, frame
/// latency and the monitor refresh rate.
#[must_use]
pub fn startup_line(info: &engine_sdk::StartupInfo) -> String {
    let refresh = info.monitor_refresh_mhz.map_or_else(
        || "unknown".to_string(),
        |mhz| format!("{:.3} Hz", f64::from(mhz) / 1000.0),
    );
    let present = info.present.as_ref().map_or_else(
        || "present: n/a".to_string(),
        |p| {
            format!(
                "present: requested {}, active {}{}, supported [{}], frame latency {}",
                p.requested.name(),
                p.active,
                if p.fell_back { " (fell back)" } else { "" },
                p.supported.join(", "),
                p.frame_latency
            )
        },
    );
    format!("gpu: {} | {present} | refresh: {refresh}", info.adapter)
}

/// Flies the planetary camera over streamed terrain.
pub struct PlanetaryApp {
    kind: TopologyKind,
    script: Option<DescentScript>,
    terrain: AnyTerrain,
    camera: PlanetaryCamera,
    free: FreeFlight,
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
    haze: bool,
    atmosphere: bool,
    /// Outer radius of the planet for the ring: nominal radius plus maximum relief.
    ring_radius: f64,
    /// Ring mesh uploaded last frame, destroyed before the next one is built.
    ring_mesh: Option<engine_sdk::MeshHandle>,
    light: DirectionalLight,
    generator_version: u32,
    relief_scale: f64,
    prev_pos: Option<DVec3>,
    pose_override: Option<(DVec3, engine_sdk::DQuat)>,
    /// F3 toggles the read-only debug overlay (hidden at start).
    #[cfg(feature = "debug-overlay")]
    overlay_visible: bool,
}

/// Direction of the point the sun is anchored to: the end of the descent script, or the end of
/// the default descent when flying interactively.
fn sun_target(script: Option<&DescentScript>, radius: f64) -> DVec3 {
    let s = script
        .copied()
        .unwrap_or_else(|| DescentScript::orbit_to_surface(radius));
    let (pos, _) = s.pose_at(s.descent_seconds, &|_| radius);
    pos
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
        let free = FreeFlight::new(camera.position(), camera.orientation(), settings.camera)
            .ok_or_else(|| SdkError::App("invalid camera start pose".into()))?;
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
            haze: settings.haze,
            atmosphere: settings.atmosphere,
            ring_radius: radius + settings.terrain.terrain.max_relief(),
            ring_mesh: None,
            light: relief_light(sun_target(settings.script.as_ref(), radius)),
            generator_version: settings.terrain.terrain.generator_version,
            relief_scale: settings.terrain.terrain.relief_scale,
            script: settings.script,
            terrain,
            camera,
            free,
            viewport_h,
            viewport_w: settings.viewport_width.max(1),
            frustum_culling: settings.frustum_culling,
            time: 0.0,
            last_view: View::new(pos, viewport_h, TAN_HALF_FOV_Y),
            quit: false,
            prev_pos: None,
            pose_override: None,
            #[cfg(feature = "debug-overlay")]
            overlay_visible: false,
        })
    }

    /// Light travel direction currently sent to the scene.
    #[must_use]
    pub fn light_direction(&self) -> Vec3 {
        self.light.direction
    }

    /// Sun elevation above the local horizon at the camera for the light direction the scene
    /// currently uses, in degrees.
    #[must_use]
    pub fn sun_elevation_at_camera_deg(&self) -> f64 {
        elevation_deg(self.camera.position(), self.light.direction)
    }

    /// Look readout for the startup line: generator version, relief scale, topology and the sun
    /// elevation at the camera (the sun follows the camera from the first update of a free flight).
    #[must_use]
    pub fn look_line(&self) -> String {
        let dir = if self.script.is_none() && self.pose_override.is_none() {
            sun_direction_for_target(self.camera.position())
        } else {
            self.light.direction
        };
        format!(
            "generator: v{} | relief scale: {}{} | topology: {} | sun elevation at camera: {:.1} deg",
            self.generator_version,
            self.relief_scale,
            if self.generator_version >= 3 {
                ""
            } else {
                " (ignored below v3)"
            },
            self.kind.name(),
            elevation_deg(self.camera.position(), dir)
        )
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

    /// Raw ids of the split set the last frame selected, ascending.
    #[must_use]
    pub fn wanted_split_raw(&self) -> Vec<u64> {
        self.terrain.wanted_split_raw()
    }

    /// Forces the camera pose every update (before the script or the interactive camera), for
    /// deterministic test paths such as seeded random flights. `None` restores normal control.
    pub fn set_pose_override(&mut self, pose: Option<(DVec3, engine_sdk::DQuat)>) {
        self.pose_override = pose;
    }

    /// Raw ids of the drawn cells, ascending.
    #[must_use]
    pub fn drawn_raw(&self) -> Vec<u64> {
        self.terrain.drawn_raw()
    }

    /// Shown geomorph factor of the drawn cells (same order as [`PlanetaryApp::drawn_raw`]).
    #[must_use]
    pub fn drawn_morphs(&self) -> Vec<f64> {
        self.terrain.drawn_morphs()
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

    /// GPU buffers held by the atmosphere ring (0 or 1): the vertex buffer uploaded last frame.
    #[must_use]
    pub fn ring_buffer_count(&self) -> usize {
        usize::from(self.ring_mesh.is_some())
    }

    /// Current adaptive streaming caps.
    #[must_use]
    pub fn stream_budget(&self) -> StreamBudget {
        self.terrain.stream_budget()
    }

    /// Overlay data for this frame (LOD histograms, queue, budgets, timings).
    #[cfg(feature = "debug-overlay")]
    #[must_use]
    pub fn diagnostics_snapshot(&self) -> engine_sdk::DiagnosticsSnapshot {
        let sample = self.telemetry.last().cloned().unwrap_or_default();
        overlay_snapshot::build(
            &sample,
            &self.terrain.drawn_levels(),
            &self.terrain.wanted_levels(),
            self.terrain.stream_budget(),
            self.terrain.held_mesh_cap(),
        )
    }

    /// Whether the F3 overlay is currently shown.
    #[cfg(feature = "debug-overlay")]
    #[must_use]
    pub fn overlay_visible(&self) -> bool {
        self.overlay_visible
    }

    /// Current interactive flight mode. Defaults to [`FlightMode::Free`]; the F key toggles it.
    #[must_use]
    pub fn flight_mode(&self) -> FlightMode {
        self.free.mode()
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

    /// Window-title readout (altitude, speed, level) on throttle frames. Counts frames only,
    /// never reads a clock; the host ignores it for headless runs.
    fn update_title(&mut self, dt: f64, scene: &mut SceneState) {
        let pos = self.camera.position();
        let moved = self.prev_pos.map_or(0.0, |p| (pos - p).length());
        self.prev_pos = Some(pos);
        if !title::should_update_title(self.frames, title::TITLE_EVERY_FRAMES) {
            return;
        }
        let len = pos.length();
        let (alt, level) = if len > 0.0 {
            let dir = pos / len;
            (
                len - self.terrain.surface_radius_at(dir),
                self.terrain.level_at(dir, false),
            )
        } else {
            (0.0, None)
        };
        scene.window_title = Some(title::format_title(
            alt,
            title::speed_from_displacement(moved, dt),
            self.kind.name(),
            level,
        ));
    }

    /// Replaces last frame's limb-ring mesh with this frame's ring item (after the terrain has
    /// rewritten `scene.items`). Failures are logged and skip the ring; the flight goes on.
    fn update_ring(
        &mut self,
        resources: &mut Resources<'_>,
        scene: &mut SceneState,
        altitude: f64,
    ) {
        if let Some(old) = self.ring_mesh.take()
            && let Err(e) = resources.destroy_mesh(old)
        {
            tracing::warn!(error = %e, "ring mesh destroy failed");
        }
        if !(self.haze && self.atmosphere) || scene.items.len() >= MAX_DRAWS_PER_FRAME {
            return;
        }
        let Some(ring) = atmosphere::ring_frame(
            self.camera.position(),
            self.ring_radius,
            altitude,
            self.light.direction,
            scene.clear_color,
        ) else {
            return;
        };
        let Ok(position) = WorldPos::from_xyz(scene.frames.root(), 0.0, 0.0, 0.0) else {
            return;
        };
        match resources.create_mesh("atmosphere-ring", &ring.vertices) {
            Ok(mesh) => {
                self.ring_mesh = Some(mesh);
                let mut item = RenderItem::new(mesh, position);
                item.rotation = ring.rotation;
                item.scale = Vec3::splat(ring.scale);
                scene.items.push(item);
            }
            Err(e) => tracing::warn!(error = %e, "ring mesh upload failed"),
        }
    }

    fn apply_camera(&self, scene: &mut SceneState) {
        let alt = self.camera.altitude(&|d| self.terrain.surface_radius_at(d));
        if let Err(e) = self
            .camera
            .apply_to(&mut scene.camera, scene.frames.root(), alt)
        {
            tracing::warn!(error = %e, "camera pose rejected, keeping previous camera");
        }
        scene.light = self.light;
        if self.haze {
            let haze = if self.atmosphere {
                atmosphere::atmosphere_for_view(alt, self.light.direction, self.camera.position())
            } else {
                haze_for_altitude(alt)
            };
            scene.fog = haze.fog;
            scene.clear_color = haze.clear_color;
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
        if !scene.startup.adapter.is_empty() {
            println!("{} | {}", startup_line(&scene.startup), self.look_line());
        }
        Ok(())
    }

    fn input(&mut self, event: &InputEvent, _scene: &mut SceneState) {
        #[cfg(feature = "debug-overlay")]
        if matches!(
            event,
            InputEvent::KeyPressed {
                key: engine_sdk::KeyCode::F3,
                repeat: false
            }
        ) {
            self.overlay_visible = !self.overlay_visible;
        }
        match *event {
            InputEvent::Resized { width, height } if height > 0 => {
                self.viewport_h = height;
                self.viewport_w = width.max(1);
            }
            InputEvent::CloseRequested => self.quit = true,
            _ => {}
        }
        if self.script.is_none() {
            let before = self.free.mode();
            self.free.handle_input(event);
            let after = self.free.mode();
            if after != before {
                println!("flight mode: {}", after.name());
                tracing::info!(mode = after.name(), "flight mode changed");
            }
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

    #[cfg(feature = "debug-overlay")]
    fn overlay_frame(&mut self) -> Option<engine_sdk::DiagnosticsSnapshot> {
        self.overlay_visible.then(|| self.diagnostics_snapshot())
    }

    fn update(&mut self, dt: f64, scene: &mut SceneState) -> AppControl {
        if dt.is_finite() && dt > 0.0 {
            self.time += dt;
        }
        if let Some((pos, rot)) = self.pose_override {
            self.camera.set_pose(pos, rot);
            self.free.set_pose(pos, rot);
        } else if let Some(script) = self.script {
            let (pos, rot) = script.pose_at(self.time, &|d| self.terrain.surface_radius_at(d));
            self.camera.set_pose(pos, rot);
            self.free.set_pose(pos, rot);
        } else {
            let pos = self.free.position();
            let len = pos.length();
            let extra = if len > 0.0 {
                self.terrain.drawn_error_at(pos / len)
            } else {
                0.0
            };
            let terrain = &self.terrain;
            self.free
                .update(dt, &|d| terrain.surface_radius_at(d), extra);
            self.camera
                .set_pose(self.free.position(), self.free.orientation());
            // Headlight-style sun: fixed elevation over the camera (no day/night cycle).
            self.light.direction = sun_direction_for_target(self.camera.position());
        }
        self.apply_camera(scene);
        self.update_title(dt, scene);
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
            self.terrain.prepare_with(&view, &mut self.telemetry);
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
        self.update_ring(resources, scene, altitude_m);
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
            morph_min: r.morph_min,
            morph_mean: r.morph_mean,
            morph_first_frame_defects: r.morph_first_frame_defects,
            handover_gap_max: r.handover_gap_max,
            multi_level_handovers: r.multi_level_handovers,
            reselect_skipped: r.reselect_skipped,
            prep_lookups: r.range_lookups,
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

#[cfg(test)]
mod look_tests {
    use super::*;

    #[test]
    fn haze_density_endpoints_and_monotone() {
        assert_eq!(haze_for_altitude(HAZE_NONE_ALTITUDE).fog.density, 0.0);
        assert_eq!(haze_for_altitude(1.0e9).fog.density, 0.0);
        assert_eq!(
            haze_for_altitude(HAZE_FULL_ALTITUDE).fog.density,
            HAZE_DENSITY
        );
        assert_eq!(haze_for_altitude(0.0).fog.density, HAZE_DENSITY);
        assert_eq!(haze_for_altitude(-50.0).fog.density, HAZE_DENSITY);
        let mut last = f32::INFINITY;
        let mut alt = -100.0;
        while alt < 400_000.0 {
            let d = haze_for_altitude(alt).fog.density;
            assert!(
                d.is_finite() && d >= 0.0 && d <= last,
                "alt {alt}: {d} after {last}"
            );
            assert!(haze_for_altitude(alt).fog.validate().is_ok());
            last = d;
            alt += 997.0;
        }
        // Linear in between: halfway density is half.
        let mid = 0.5 * (HAZE_FULL_ALTITUDE + HAZE_NONE_ALTITUDE);
        let d = haze_for_altitude(mid).fog.density;
        assert!((d - 0.5 * HAZE_DENSITY).abs() < 1e-9, "{d}");
    }

    #[test]
    fn haze_is_nan_safe() {
        for a in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let h = haze_for_altitude(a);
            assert_eq!(h.fog.density, 0.0, "{a}");
            assert_eq!(h.clear_color, SPACE_COLOR);
            assert!(h.fog.validate().is_ok());
        }
    }

    #[test]
    fn clear_colour_is_sky_at_full_haze_and_space_at_none() {
        let full = haze_for_altitude(0.0);
        assert_eq!(full.clear_color[..3], SKY_COLOR);
        assert_eq!(full.fog.color, SKY_COLOR);
        let none = haze_for_altitude(HAZE_NONE_ALTITUDE);
        assert_eq!(none.clear_color, SPACE_COLOR);
        // The horizon fog colour equals the sky behind it at every altitude with haze.
        for alt in [0.0, 30_000.0, 100_000.0, 190_000.0] {
            let h = haze_for_altitude(alt);
            let fade = h.fog.density / HAZE_DENSITY;
            for k in 0..3 {
                let want = SKY_COLOR[k] * fade + SPACE_COLOR[k] * (1.0 - fade);
                assert!((h.clear_color[k] - want).abs() < 1e-6);
            }
        }
    }

    fn elevation_deg(target: DVec3) -> f64 {
        let travel = sun_direction_for_target(target);
        let to_sun = DVec3::new(
            -f64::from(travel.x),
            -f64::from(travel.y),
            -f64::from(travel.z),
        );
        assert!((to_sun.length() - 1.0).abs() < 1e-6);
        to_sun.dot(target.normalize()).asin().to_degrees()
    }

    #[test]
    fn sun_stands_35_degrees_above_the_target_horizon() {
        let script = DescentScript::orbit_to_surface(6_371_000.0);
        let (end, _) = script.pose_at(script.descent_seconds, &|_| 6_371_000.0);
        for t in [
            end,
            DVec3::new(1.0, 0.8, 0.6),
            DVec3::new(-0.3, 0.9, 0.1),
            DVec3::Y,
            DVec3::NEG_Y,
            DVec3::new(0.0, 0.0, -2.0),
        ] {
            let e = elevation_deg(t);
            assert!((e - SUN_ELEVATION_DEG).abs() < 0.5, "{t:?}: {e}");
            assert_eq!(sun_direction_for_target(t), sun_direction_for_target(t));
        }
    }

    #[test]
    fn degenerate_target_gives_the_default_direction() {
        for t in [DVec3::ZERO, DVec3::new(f64::NAN, 0.0, 0.0)] {
            assert_eq!(
                sun_direction_for_target(t),
                DirectionalLight::default().direction
            );
        }
    }

    #[test]
    fn relief_light_is_valid() {
        let l = relief_light(DVec3::new(1.0, 0.8, 0.6));
        assert!(l.validate().is_ok());
        assert_eq!(l.intensity, LIGHT_INTENSITY);
        assert_eq!(l.ambient, [LIGHT_AMBIENT; 3]);
    }
}
