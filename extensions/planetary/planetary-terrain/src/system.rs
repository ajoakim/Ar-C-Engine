//! Terrain streaming system: LOD selection, request planning, stream update, eviction
//! reconcile, effective cut and the camera-relative draw list.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use engine_sdk::streaming::{
    ClassBudget, FrameReport, ResidencyManager, ResidencyState, StreamConfig, StreamError,
    StreamManager, StreamRequest, Uploader,
};
use engine_sdk::{
    CancelToken, DVec3, IndexBufferHandle, JobSystem, MAX_DRAWS_PER_FRAME, Material, MaterialId,
    RenderItem, Resources, SceneState, SdkError, WorldPos,
};
use planetary_core::lod::{
    DEFAULT_KAPPA, LodModel, LodParams, SelectStats, View, cell_bound, cell_distance, eye_in_cone,
    frustum_visible, horizon_visible, morph_factor, refine_distance,
};
use planetary_core::topology::select::{retain_culled, select_cells, unsplit_reasons};
use planetary_core::{MAX_LEVEL, PlanetTerrain, TerrainParams, Topology};

use crate::churn::{ChurnFlags, ChurnTracker};
use crate::effective::effective_cut;
use crate::key::CellKey;
use crate::palette::{
    DEBUG_PALETTE_H, PALETTE_H, PALETTE_H_V2, PALETTE_W, debug_palette_rgba8, palette_rgba8,
    palette_rgba8_v2,
};
use crate::requests::{
    plan_requests_scored, priority_for_level, request_score, stale_requests, wanted_closure,
};
use crate::source::{CellSource, cell_payload_estimate};
use crate::uploader::{GpuUploader, HeldCell};
use crate::{BASE_CLASS, DEMO_SEED, DETAIL_CLASS};

/// Draw slots kept free below `MAX_DRAWS_PER_FRAME` (other scene items).
pub const DRAW_MARGIN: usize = 24;

/// Terrain system configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainConfig {
    pub planet: u32,
    pub world_seed: u64,
    pub terrain: TerrainParams,
    pub cells_per_edge: u32,
    pub lod: LodParams,
    pub kappa: f64,
    pub stream: StreamConfig,
    /// Per-frame upload cap (patches) for the residency manager.
    pub uploads_per_frame: u32,
    /// Levels `0..=base_levels` use the base class budget.
    pub base_levels: u8,
    /// Detail class budget in patches.
    pub detail_budget_patches: u64,
    pub max_requests_per_frame: u32,
    /// Block on workers every frame. Tests only: production never settles.
    pub settle_each_frame: bool,
    pub workers: usize,
    /// Colour each cell by level/id through the debug palette (visible LOD transitions).
    pub debug_cells: bool,
    /// Adapt the per-frame streaming caps to the measured frame cost (see [`StreamBudget`]).
    /// Off by default: the caps above then apply exactly as configured.
    pub adaptive: bool,
    /// Hard maxima and the time target of the adaptive budget.
    pub budget: BudgetLimits,
    /// Compute the per-frame churn counters of [`TerrainReport`] (flicker diagnosis). Off by
    /// default: the counters then stay 0 and cost nothing.
    pub diagnostics: bool,
    /// Frames a split cell that was culled (outside the frustum or beyond the horizon) is kept
    /// split instead of dropping out of the selection (07.1-20); 0 disables the retention.
    pub retain_frames: u32,
    /// Drive the per-piece geomorph factor (ADR-0014). Off: every item is drawn at morph 0.
    pub morph_enabled: bool,
    /// The morph ends at `morph_end_scale * refine_distance(level)`; valid `1.0 <= s < 1.9`.
    pub morph_end_scale: f64,
    /// Slew limit: a shown factor falls by at most `1 / morph_frames` per frame (>= 1 when
    /// `morph_enabled`).
    pub morph_frames: u32,
}

/// Hard maxima and target of the adaptive per-frame streaming budget.
///
/// The caps of `TerrainConfig` (`uploads_per_frame`, `stream.max_starts_per_frame`,
/// `max_requests_per_frame`) are the base values; the adaptive budget scales them between
/// a quarter and these maxima. The terrain never reads a clock: the app feeds the measured cost
/// of the previous frame through [`TerrainSystem::set_frame_cost`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BudgetLimits {
    /// Per-frame time (ms) of terrain work (prepare plus gpu_frame) the budget aims for.
    pub target_ms: f64,
    /// Hard maximum uploads per frame.
    pub max_uploads: u32,
    /// Hard maximum request starts (and completions applied) per frame.
    pub max_starts: u32,
    /// Hard maximum requests issued per frame.
    pub max_requests: u32,
}

impl Default for BudgetLimits {
    fn default() -> Self {
        Self {
            target_ms: 8.0,
            max_uploads: 32,
            max_starts: 64,
            max_requests: 128,
        }
    }
}

/// Requests in flight (started, not yet uploaded) allowed per upload slot.
const IN_FLIGHT_PER_UPLOAD: usize = 3;
/// Frames of request starts the manager queue may hold.
const QUEUE_FRAMES: usize = 2;
/// Lowest budget scale (a slow frame shrinks the caps to a quarter of the base).
const MIN_SCALE: f64 = 0.25;

/// Effective per-frame streaming caps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreamBudget {
    pub uploads: u32,
    pub starts: u32,
    pub requests: u32,
    /// Smoothed scale applied to the base caps (1.0 = base).
    pub scale: f64,
}

impl StreamBudget {
    /// The configured base caps (scale 1).
    #[must_use]
    pub fn base(cfg: &TerrainConfig) -> Self {
        Self {
            uploads: cfg.uploads_per_frame,
            starts: cfg.stream.max_starts_per_frame,
            requests: cfg.max_requests_per_frame,
            scale: 1.0,
        }
    }

    /// Caps for the next frame from the previous scale and the measured cost (ms) of the last
    /// frame. An absent, non-finite or negative cost, or a non-adaptive configuration, gives the
    /// base caps. Otherwise `scale = (prev + target / max(cost, 0.05)) / 2` clamped to
    /// `[0.25, max / base]`, and each cap is `round(base * scale)` clamped to `[1, max(base, hard
    /// maximum)]`. Pure; fast frames grow the caps, slow frames shrink them.
    #[must_use]
    pub fn next(cfg: &TerrainConfig, prev_scale: f64, frame_ms: Option<f64>) -> Self {
        let base = Self::base(cfg);
        let Some(ms) = frame_ms.filter(|m| m.is_finite() && *m >= 0.0) else {
            return base;
        };
        if !cfg.adaptive {
            return base;
        }
        let b = &cfg.budget;
        let target = if b.target_ms.is_finite() && b.target_ms > 0.0 {
            b.target_ms
        } else {
            return base;
        };
        let prev = if prev_scale.is_finite() && prev_scale > 0.0 {
            prev_scale
        } else {
            1.0
        };
        let raw = target / ms.max(0.05);
        let cap = |base: u32, hard: u32| -> u32 { hard.max(base).max(1) };
        let hi_up = cap(base.uploads, b.max_uploads);
        let hi_st = cap(base.starts, b.max_starts);
        let hi_rq = cap(base.requests, b.max_requests);
        let max_scale = [
            f64::from(hi_up) / f64::from(base.uploads.max(1)),
            f64::from(hi_st) / f64::from(base.starts.max(1)),
            f64::from(hi_rq) / f64::from(base.requests.max(1)),
        ]
        .into_iter()
        .fold(1.0_f64, f64::max);
        let scale = (0.5 * (prev + raw)).clamp(MIN_SCALE, max_scale);
        let scaled = |base: u32, hi: u32| -> u32 {
            let v = (f64::from(base) * scale).round();
            let v = if v >= 1.0 { v } else { 1.0 };
            let v = if v <= f64::from(hi) { v } else { f64::from(hi) };
            v as u32
        };
        Self {
            uploads: scaled(base.uploads, hi_up),
            starts: scaled(base.starts, hi_st),
            requests: scaled(base.requests, hi_rq),
            scale,
        }
    }
}

/// Uploader wrapper that refuses uploads beyond a per-frame count. A refused upload stays
/// `CpuReady` and is retried next frame (it is not rejected or evicted).
struct Budgeted<U> {
    inner: U,
    left: u32,
}

impl<K, U: Uploader<K>> Uploader<K> for Budgeted<U> {
    fn upload(&mut self, key: &K, data: &[u8]) -> bool {
        if self.left == 0 {
            return false;
        }
        let ok = self.inner.upload(key, data);
        if ok {
            self.left -= 1;
        }
        ok
    }
}

/// Palette texture for a configuration: label, rows and RGBA8 bytes. The debug palette wins;
/// otherwise generator v2 uses the 8-row dry-to-rock ramp palette, v1 the 4-row mask palette.
pub(crate) fn palette_for(
    terrain: &TerrainParams,
    debug_cells: bool,
) -> (&'static str, u32, Vec<u8>) {
    if debug_cells {
        (
            "planet-debug-palette",
            DEBUG_PALETTE_H,
            debug_palette_rgba8(),
        )
    } else if terrain.generator_version >= 2 {
        ("planet-palette-v2", PALETTE_H_V2, palette_rgba8_v2())
    } else {
        ("planet-palette", PALETTE_H, palette_rgba8())
    }
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            planet: 1,
            world_seed: DEMO_SEED,
            terrain: TerrainParams::demo_v2(),
            cells_per_edge: 32,
            lod: LodParams::default(),
            kappa: DEFAULT_KAPPA,
            stream: StreamConfig::default(),
            uploads_per_frame: 4,
            base_levels: 3,
            detail_budget_patches: 1500,
            max_requests_per_frame: 16,
            settle_each_frame: false,
            workers: 2,
            debug_cells: false,
            adaptive: false,
            budget: BudgetLimits::default(),
            diagnostics: false,
            retain_frames: 8,
            morph_enabled: true,
            morph_end_scale: 1.25,
            morph_frames: 8,
        }
    }
}

/// Counted per-frame work.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerrainReport {
    pub frame: u64,
    pub stream: FrameReport,
    pub select: SelectStats,
    pub wanted_leaves: usize,
    pub drawn_leaves: usize,
    pub drawn_max_jump: u8,
    pub requests_issued: u32,
    pub requests_backpressure: u32,
    pub request_errors: u32,
    pub requests_cancelled: u32,
    pub upload_rejects: u32,
    pub destroyed: u32,
    pub held_meshes: usize,
    /// Patch requests issued and not yet resident.
    pub pending_requests: usize,
    /// Render items pushed this frame.
    pub draw_items: usize,
    /// Effective-cut leaves skipped by draw culling.
    pub culled_draws: u32,
    /// Wanted pieces plus ancestors, and how many of them are resident.
    pub closure_cells: usize,
    pub closure_resident: usize,
    /// Splits removed by the effective-cut coarsen stage this frame.
    pub effective_coarsened: u32,
    /// Churn counters (only with `TerrainConfig::diagnostics`, else 0); see [`crate::churn`].
    pub level_changes: u32,
    pub drawn_appeared: u32,
    pub drawn_disappeared: u32,
    pub cull_toggles: u32,
    pub overlap_pairs: u32,
    pub flipflops: u32,
    pub flipflops_unexplained: u32,
    /// Why cells split last selection are not split now (diagnostics): total, culled this frame,
    /// below the hysteresis band, within the band or above tau (budget or trim), parent unsplit.
    pub unsplit: [u32; 6],
    /// Previously split cells not retained this frame because they had been culled for
    /// `retain_frames` frames already (the retention age bound).
    pub retain_expired: u32,
    /// Cells the selection dropped as culled that the draw pass nevertheless drew visibly
    /// (selection and draw culling disagree; diagnostics).
    pub unsplit_culled_drawn_visible: u32,
    /// Geomorph factor over the drawn cells of this frame (0 when nothing is drawn).
    pub morph_min: f64,
    pub morph_mean: f64,
    /// Cells that appeared by refinement (a parent was drawn last frame) and were not shown at
    /// morph 1 on their first frame. Expected 0.
    pub morph_first_frame_defects: u32,
    /// Largest morph a drawn cell showed in the frame before it was replaced by descendants.
    pub handover_gap_max: f64,
    /// Drawn cells replaced by descendants more than one level deeper this frame.
    pub multi_level_handovers: u32,
}

/// Streams, uploads and draws the terrain of any [`Topology`].
///
/// Each held cell owns one vertex buffer; drawn pieces are index-range draws into one shared
/// static index buffer per vertex class (created in `setup`, ADR-0013).
pub struct TerrainSystem<T: Topology> {
    cfg: TerrainConfig,
    terrain: Arc<PlanetTerrain>,
    topo: Arc<T>,
    model: LodModel,
    manager: StreamManager<CellKey<T>, CellSource<T>>,
    jobs: JobSystem,
    held: BTreeMap<T::Cell, HeldCell<T>>,
    requested: BTreeMap<T::Cell, (CellKey<T>, CancelToken)>,
    prev_split: BTreeSet<T::Cell>,
    last_view: Option<View>,
    wanted_split: BTreeSet<T::Cell>,
    wanted: Vec<T::Cell>,
    /// Wanted pieces plus all their ancestors (the fallback chains); kept resident.
    closure: BTreeSet<T::Cell>,
    drawn: Vec<T::Cell>,
    drawn_set: BTreeSet<T::Cell>,
    /// Split cells of the last drawn cut (see [`TerrainSystem::drawn_split`]).
    drawn_split: BTreeSet<T::Cell>,
    material: MaterialId,
    frame: u64,
    report: TerrainReport,
    /// Exact vertex count per vertex class (`None` for classes the topology does not use).
    vertex_counts: Vec<Option<u32>>,
    /// Per `Piece::variant`: (vertex class, first index, index count) in the shared buffer.
    variant_ranges: Vec<(u8, u32, u32)>,
    /// Shared index buffer per vertex class, created by `setup`.
    shared: Vec<Option<IndexBufferHandle>>,
    /// Caps in force for the current frame (see [`StreamBudget`]).
    budget: StreamBudget,
    /// Measured cost (ms) of the previous frame as reported by the app.
    frame_cost_ms: Option<f64>,
    /// Churn counters state (diagnostics only).
    churn: ChurnTracker<T::Cell>,
    /// Cells dropped from the split set by this frame's selection because they were culled
    /// (diagnostics only).
    unsplit_culled: BTreeSet<T::Cell>,
    /// Consecutive frames each retained culled cell has been kept split.
    retained_age: BTreeMap<T::Cell, u32>,
    /// Cells drawn (or culled in the partition) in the previous frame.
    prev_drawn: BTreeSet<T::Cell>,
    /// Shown morph factor per cell drawn in the last frame.
    morph_shown: BTreeMap<T::Cell, f64>,
}

/// Maximum level difference across any shared edge of `cells` (cells may repeat).
fn max_level_jump<T: Topology>(topo: &T, cells: &[T::Cell]) -> u8 {
    let set: BTreeSet<T::Cell> = cells.iter().copied().collect();
    let mut jump = 0u8;
    let mut nb = Vec::new();
    for &p in &set {
        nb.clear();
        topo.neighbors(p, &mut nb);
        for &n in &nb {
            let mut cur = Some(n);
            while let Some(q) = cur {
                if set.contains(&q) {
                    jump = jump.max(topo.level(p).saturating_sub(topo.level(q)));
                    break;
                }
                cur = topo.primary_parent(q);
            }
        }
    }
    jump
}

fn app_err(what: &str, e: impl std::fmt::Display) -> SdkError {
    SdkError::App(format!("{what}: {e}"))
}

impl<T: Topology> TerrainSystem<T> {
    /// Validates the configuration and builds the terrain, stream manager and worker pool.
    /// `topo` must have been built for `cfg.cells_per_edge`-compatible sampling.
    ///
    /// # Errors
    /// `SdkError::App` for an invalid configuration, `SdkError::Job` if workers cannot start.
    pub fn new(cfg: TerrainConfig, topo: T) -> Result<Self, SdkError> {
        if cfg.lod.max_level > MAX_LEVEL {
            return Err(SdkError::App("lod.max_level exceeds MAX_LEVEL".into()));
        }
        if cfg.lod.max_leaves < 6 || cfg.lod.max_leaves + DRAW_MARGIN > MAX_DRAWS_PER_FRAME {
            return Err(SdkError::App(format!(
                "lod.max_leaves must be in 6..={}",
                MAX_DRAWS_PER_FRAME - DRAW_MARGIN
            )));
        }
        if cfg.workers < 1 {
            return Err(SdkError::App("workers must be >= 1".into()));
        }
        if cfg.morph_enabled {
            if !(cfg.morph_end_scale.is_finite()
                && cfg.morph_end_scale >= 1.0
                && cfg.morph_end_scale < 1.9)
            {
                return Err(SdkError::App("morph_end_scale must be in 1.0..1.9".into()));
            }
            if cfg.morph_frames == 0 {
                return Err(SdkError::App(
                    "morph_frames must be >= 1 when morph is enabled".into(),
                ));
            }
        }
        let terrain = Arc::new(
            PlanetTerrain::new(cfg.world_seed, cfg.terrain, cfg.cells_per_edge)
                .map_err(|e| app_err("terrain", e))?,
        );
        let model = LodModel::for_topology(&terrain, cfg.kappa, topo.sample_angle0());
        let topo = Arc::new(topo);
        let variants = topo.index_variants();
        if variants.is_empty() {
            return Err(SdkError::App("topology has no index variants".into()));
        }
        let class_count = variants
            .iter()
            .map(|v| usize::from(v.vertex_class) + 1)
            .max()
            .unwrap_or(0);
        let mut vertex_counts: Vec<Option<u32>> = vec![None; class_count];
        let mut variant_ranges = Vec::with_capacity(variants.len());
        let mut class_len = vec![0u32; class_count];
        for v in &variants {
            let max_index = v.indices.iter().copied().max().unwrap_or(0);
            let slot = &mut vertex_counts[usize::from(v.vertex_class)];
            *slot = Some(slot.map_or(max_index + 1, |c| c.max(max_index + 1)));
            let count = u32::try_from(v.indices.len())
                .map_err(|_| SdkError::App("index variant too large".into()))?;
            let len = &mut class_len[usize::from(v.vertex_class)];
            variant_ranges.push((v.vertex_class, *len, count));
            *len = len
                .checked_add(count)
                .ok_or_else(|| SdkError::App("shared index buffer too large".into()))?;
        }
        let max_vertex_count = vertex_counts.iter().flatten().copied().max().unwrap_or(0);
        let bytes = cell_payload_estimate(max_vertex_count);
        if cfg.adaptive
            && !(cfg.budget.target_ms.is_finite()
                && cfg.budget.target_ms > 0.0
                && cfg.budget.max_uploads >= 1
                && cfg.budget.max_starts >= 1
                && cfg.budget.max_requests >= 1)
        {
            return Err(SdkError::App("invalid adaptive budget limits".into()));
        }
        // Adaptive mode builds the stream manager and the upload byte cap at the hard maxima
        // (neither can change after construction) and throttles per frame below them.
        let mut stream_cfg = cfg.stream;
        let mut upload_slots = cfg.uploads_per_frame;
        if cfg.adaptive {
            stream_cfg.max_starts_per_frame =
                stream_cfg.max_starts_per_frame.max(cfg.budget.max_starts);
            stream_cfg.max_completions_per_frame = stream_cfg
                .max_completions_per_frame
                .max(cfg.budget.max_starts);
            upload_slots = upload_slots.max(cfg.budget.max_uploads);
        }
        let mut residency = ResidencyManager::new(u64::from(upload_slots) * bytes);
        let base_patches = 6 * ((1u64 << (2 * (u32::from(cfg.base_levels) + 1))) - 1) / 3 + 16;
        residency.set_class_budget(
            BASE_CLASS,
            ClassBudget {
                cpu_bytes: base_patches * bytes,
                gpu_bytes: base_patches * bytes,
            },
        );
        residency.set_class_budget(
            DETAIL_CLASS,
            ClassBudget {
                cpu_bytes: cfg.detail_budget_patches * bytes,
                gpu_bytes: cfg.detail_budget_patches * bytes,
            },
        );
        let source = Arc::new(CellSource::new(
            topo.clone(),
            terrain.clone(),
            model,
            cfg.debug_cells,
            max_vertex_count,
        ));
        let mut manager = StreamManager::new(stream_cfg, residency, source)
            .map_err(|e| app_err("stream manager", e))?;
        manager.set_deterministic(cfg.settle_each_frame);
        let jobs = JobSystem::builder().workers(cfg.workers).build()?;
        let budget = StreamBudget::base(&cfg);
        Ok(Self {
            budget,
            frame_cost_ms: None,
            churn: ChurnTracker::default(),
            unsplit_culled: BTreeSet::new(),
            retained_age: BTreeMap::new(),
            prev_drawn: BTreeSet::new(),
            morph_shown: BTreeMap::new(),
            cfg,
            terrain,
            topo,
            model,
            manager,
            jobs,
            held: BTreeMap::new(),
            requested: BTreeMap::new(),
            prev_split: BTreeSet::new(),
            last_view: None,
            wanted_split: BTreeSet::new(),
            wanted: Vec::new(),
            closure: BTreeSet::new(),
            drawn: Vec::new(),
            drawn_set: BTreeSet::new(),
            drawn_split: BTreeSet::new(),
            material: MaterialId::DEFAULT,
            frame: 0,
            report: TerrainReport::default(),
            shared: vec![None; vertex_counts.len()],
            vertex_counts,
            variant_ranges,
        })
    }

    #[must_use]
    pub fn config(&self) -> &TerrainConfig {
        &self.cfg
    }

    #[must_use]
    pub fn terrain(&self) -> &Arc<PlanetTerrain> {
        &self.terrain
    }

    #[must_use]
    pub fn model(&self) -> &LodModel {
        &self.model
    }

    /// Feeds the measured cost (ms of prepare plus gpu_frame) of the previous frame; it sets the
    /// caps of the next `prepare` and `gpu_frame` when `TerrainConfig::adaptive` is on. The
    /// terrain never reads a clock. Non-finite and negative values count as absent.
    pub fn set_frame_cost(&mut self, ms: f64) {
        self.frame_cost_ms = (ms.is_finite() && ms >= 0.0).then_some(ms);
    }

    /// BLOCKING: waits until every job started so far has finished (its completion is applied by
    /// the next `gpu_frame`). For benches and tests that model workers finishing within a frame
    /// without sleeping; never call it on a production frame path.
    pub fn wait_for_workers(&mut self) {
        self.manager.settle();
    }

    /// The per-frame caps currently in force.
    #[must_use]
    pub fn stream_budget(&self) -> StreamBudget {
        self.budget
    }

    /// The topology this system streams.
    #[must_use]
    pub fn topology(&self) -> &Arc<T> {
        &self.topo
    }

    /// The terrain material (the debug-palette material when `debug_cells` is set).
    #[must_use]
    pub fn material(&self) -> MaterialId {
        self.material
    }

    /// Creates the palette texture, the terrain material and one shared static index buffer per
    /// vertex class (all index variants of a class concatenated).
    ///
    /// # Errors
    /// Propagates texture, material and buffer creation errors.
    pub fn setup(
        &mut self,
        resources: &mut Resources<'_>,
        scene: &mut SceneState,
    ) -> Result<(), SdkError> {
        let (label, rows, data) = palette_for(&self.cfg.terrain, self.cfg.debug_cells);
        let tex = resources.create_texture(label, PALETTE_W, rows, &data)?;
        let mut lists: Vec<Vec<u32>> = vec![Vec::new(); self.shared.len()];
        for v in self.topo.index_variants() {
            lists[usize::from(v.vertex_class)].extend_from_slice(&v.indices);
        }
        for (class, list) in lists.iter().enumerate() {
            if !list.is_empty() {
                self.shared[class] = Some(resources.create_index_buffer("terrain-indices", list)?);
            }
        }
        self.material = scene.add_material(Material {
            base_color: [1.0; 4],
            base_color_texture: Some(tex),
        })?;
        Ok(())
    }

    /// Destroys the held cell meshes and the shared index buffers. Call once before the backend
    /// goes away if buffers must be released explicitly (otherwise they die with the backend).
    ///
    /// # Errors
    /// Propagates backend destroy errors (the first one; all destroys are attempted).
    pub fn teardown(&mut self, resources: &mut Resources<'_>) -> Result<(), SdkError> {
        let mut first: Option<SdkError> = None;
        for (_, h) in std::mem::take(&mut self.held) {
            if let Err(e) = resources.destroy_mesh(h.mesh) {
                first.get_or_insert(e);
            }
        }
        for slot in &mut self.shared {
            if let Some(ib) = slot.take()
                && let Err(e) = resources.destroy_index_buffer(ib)
            {
                first.get_or_insert(e);
            }
        }
        first.map_or(Ok(()), Err)
    }

    /// `App::update` step: select (with hysteresis), cancel stale requests, issue capped requests.
    pub fn prepare(&mut self, view: &View) {
        let mut rep = TerrainReport {
            frame: self.frame,
            ..TerrainReport::default()
        };
        self.last_view = Some(*view);
        self.budget = StreamBudget::next(&self.cfg, self.budget.scale, self.frame_cost_ms);
        let mut sel = {
            let held = &self.held;
            select_cells(
                &*self.topo,
                &self.model,
                &self.cfg.lod,
                view,
                &self.prev_split,
                &|id| held.get(&id).map(|h| h.range),
            )
        };
        // Pieces added by the retention: drawn if resident, never requested (they are not visible,
        // and requests for them would compete with the detail the camera is waiting for).
        let mut before: BTreeSet<T::Cell> = BTreeSet::new();
        if self.cfg.retain_frames > 0 {
            // Cells culled this frame keep their split for at most `retain_frames` frames, so a
            // cell that leaves the view and returns is not split again (flip-flop).
            let limit = self.cfg.retain_frames;
            let ages = &self.retained_age;
            let candidates: BTreeSet<T::Cell> = self
                .prev_split
                .iter()
                .copied()
                .filter(|c| ages.get(c).is_none_or(|a| *a < limit))
                .collect();
            rep.retain_expired = u32::try_from(
                self.prev_split
                    .iter()
                    .filter(|c| ages.get(c).is_some_and(|a| *a >= limit) && !sel.split.contains(c))
                    .count(),
            )
            .unwrap_or(u32::MAX);
            let held = &self.held;
            let r = retain_culled(
                &*self.topo,
                &self.model,
                &self.cfg.lod,
                view,
                &candidates,
                &|id| held.get(&id).map(|h| h.range),
                &mut sel.split,
            );
            sel.stats.retained_culled = u32::try_from(r.cells.len()).unwrap_or(u32::MAX);
            sel.stats.retain_rejected = r.rejected;
            if !r.cells.is_empty() {
                before = sel.pieces.iter().map(|p| p.cell).collect();
                sel.pieces.clear();
                self.topo.pieces(&sel.split, &mut sel.pieces);
                sel.stats.leaves = sel.pieces.len();
            }
            self.retained_age = r
                .cells
                .iter()
                .map(|c| (*c, self.retained_age.get(c).copied().unwrap_or(0) + 1))
                .collect();
        }
        // Urgency of each piece that is not resident yet: nearest and largest screen-space error
        // first. Resident pieces need no request, so they are not scored.
        let scored: Vec<(T::Cell, f64)> = sel
            .pieces
            .iter()
            .filter(|p| {
                before.is_empty() || before.contains(&p.cell) || self.held.contains_key(&p.cell)
            })
            .map(|p| {
                let score = if self.held.contains_key(&p.cell) {
                    0.0
                } else {
                    request_score(
                        &self.model,
                        view,
                        &self.topo.footprint(p.cell),
                        self.model.height,
                        self.topo.level(p.cell),
                    )
                };
                (p.cell, score)
            })
            .collect();
        if self.cfg.diagnostics {
            let held = &self.held;
            let (u, culled_cells) = unsplit_reasons(
                &*self.topo,
                &self.model,
                &self.cfg.lod,
                view,
                &self.prev_split,
                &sel.split,
                &|id| held.get(&id).map(|h| h.range),
            );
            self.unsplit_culled = culled_cells.into_iter().collect();
            rep.unsplit = [
                u.total,
                u.culled,
                u.culled_horizon,
                u.below_band,
                u.within_band_or_above,
                u.cascade,
            ];
        }
        self.prev_split.clone_from(&sel.split);
        self.wanted_split = sel.split;
        self.wanted = sel.pieces.iter().map(|p| p.cell).collect();
        rep.select = sel.stats;
        rep.wanted_leaves = self.wanted.len();

        // Requests that finished (resident) or failed/were evicted no longer count as pending.
        let manager = &self.manager;
        self.requested.retain(|_, (key, _)| {
            !matches!(manager.state(key), None | Some(ResidencyState::GpuResident))
        });

        let closure = wanted_closure(&*self.topo, &self.wanted);
        let ids: BTreeSet<T::Cell> = self.requested.keys().copied().collect();
        for id in stale_requests(&ids, &closure) {
            if let Some((key, token)) = self.requested.remove(&id) {
                token.cancel();
                let _ = self.manager.cancel(&key);
                rep.requests_cancelled += 1;
            }
        }

        // Keep the manager queue short so the next frame's nearest-first order is not stuck
        // behind a stale backlog: the queue holds at most two frames of starts, and the
        // in-flight window bounds started-but-not-uploaded work.
        let queued = self.manager.pending_len();
        let in_flight = self.manager.in_flight_len();
        let window = self.budget.uploads as usize * IN_FLIGHT_PER_UPLOAD;
        let issue = (self.budget.requests as usize)
            .min((self.budget.starts as usize * QUEUE_FRAMES).saturating_sub(queued))
            .min(window.saturating_sub(in_flight));
        self.closure = closure;
        let planned = {
            let held = &self.held;
            let requested = &self.requested;
            plan_requests_scored(
                &*self.topo,
                &scored,
                &|id| held.contains_key(&id),
                &|id| requested.contains_key(&id),
                issue,
            )
        };
        for id in planned {
            let Ok(key) = CellKey::new(&self.topo, self.cfg.planet, id) else {
                rep.request_errors += 1;
                continue;
            };
            let token = CancelToken::new();
            let class = if self.topo.level(id) <= self.cfg.base_levels {
                BASE_CLASS
            } else {
                DETAIL_CLASS
            };
            let req =
                StreamRequest::new(key.clone(), priority_for_level(self.topo.level(id)), class)
                    .with_cancel(token.clone());
            match self.manager.request(req) {
                Ok(()) => {
                    self.requested.insert(id, (key, token));
                    rep.requests_issued += 1;
                }
                Err(StreamError::Duplicate(_) | StreamError::QueueFull { .. }) => {
                    rep.requests_backpressure += 1;
                }
                Err(_) => rep.request_errors += 1,
            }
        }
        rep.pending_requests = self.requested.len();
        self.report = rep;
    }

    /// `App::gpu_frame` step: advance streaming and upload on the frame thread, destroy meshes
    /// of silently evicted patches, compute the effective cut and rewrite `scene.items`.
    ///
    /// The planetary app owns the whole item list, so `scene.items` is cleared every frame.
    ///
    /// # Errors
    /// Propagates stream, GPU and position errors.
    pub fn gpu_frame(
        &mut self,
        resources: &mut Resources<'_>,
        scene: &mut SceneState,
    ) -> Result<(), SdkError> {
        self.frame += 1;
        let mut uploader = Budgeted {
            inner: GpuUploader {
                resources: &mut *resources,
                held: &mut self.held,
                vertex_counts: &self.vertex_counts,
                rejected: Vec::new(),
            },
            // The byte cap alone admits more small cells (hex pentagons and half cells are
            // smaller than the estimate), so the count cap is always enforced here.
            left: if self.cfg.adaptive {
                self.budget.uploads
            } else {
                self.cfg.uploads_per_frame
            },
        };
        let stream = self
            .manager
            .update(&self.jobs, self.frame, &mut uploader)
            .map_err(|e| app_err("stream update", e))?;
        let rejected = std::mem::take(&mut uploader.inner.rejected);
        if self.cfg.settle_each_frame {
            // Test-only determinism: production never blocks on workers.
            self.manager.settle();
        }
        for key in &rejected {
            let _ = self.manager.evict(key);
        }
        let upload_rejects = u32::try_from(rejected.len()).unwrap_or(u32::MAX);

        // Destroy meshes of patches that are no longer GPU resident before building draws.
        let stale: Vec<T::Cell> = self
            .held
            .iter()
            .filter(|(_, h)| self.manager.state(&h.key) != Some(ResidencyState::GpuResident))
            .map(|(id, _)| *id)
            .collect();
        let mut destroyed = 0u32;
        let mut destroyed_ids: Vec<T::Cell> = Vec::new();
        for id in stale {
            if let Some(h) = self.held.remove(&id) {
                resources.destroy_mesh(h.mesh)?;
                destroyed += 1;
                if self.cfg.diagnostics {
                    destroyed_ids.push(id);
                }
            }
        }

        let held = &self.held;
        let mut eff = effective_cut(&*self.topo, &self.wanted_split, &|id| {
            held.contains_key(&id)
        });
        self.drawn_split = std::mem::take(&mut eff.split);
        // Keep every wanted piece and its ancestors fresh in the residency LRU: an ancestor of a
        // drawn leaf is not drawn itself, so without this it would be the first eviction victim
        // and the fallback chain under the camera would be lost (holes back to the base level).
        let mut closure_resident = 0usize;
        for id in &self.closure {
            if let Some(h) = self.held.get(id) {
                let _ = self.manager.resolve(&h.key);
                closure_resident += 1;
            }
        }
        scene.items.clear();
        self.drawn.clear();
        self.drawn_set.clear();
        let mut culled = 0u32;
        let mut culled_cells: BTreeSet<T::Cell> = BTreeSet::new();
        let occluder = self.model.radius + self.model.height.min;
        let mut new_shown: BTreeMap<T::Cell, f64> = BTreeMap::new();
        let mut refine_cells: Vec<T::Cell> = Vec::new();
        let mut parent_buf: Vec<T::Cell> = Vec::new();
        for piece in &eff.pieces {
            let leaf = piece.cell;
            let Some(h) = self.held.get(&leaf) else {
                continue;
            };
            let morph = if let Some(&m) = new_shown.get(&leaf) {
                m
            } else {
                let level = self.topo.level(leaf);
                let m_dist = match &self.last_view {
                    Some(view) if self.cfg.morph_enabled && level >= 1 => {
                        let fp = self.topo.footprint(leaf);
                        let d = cell_distance(self.model.radius, view.eye, &fp, h.range);
                        let d = if d > self.cfg.lod.min_distance {
                            d
                        } else {
                            self.cfg.lod.min_distance
                        };
                        let tau = self.cfg.lod.tau_px;
                        let d_start = refine_distance(&self.model, view.proj_scale, tau, level - 1);
                        let d_end = self.cfg.morph_end_scale
                            * refine_distance(&self.model, view.proj_scale, tau, level);
                        morph_factor(d, d_start, d_end)
                    }
                    _ => 0.0,
                };
                let m = if !self.cfg.morph_enabled || level == 0 {
                    0.0
                } else if let Some(&prev) = self.morph_shown.get(&leaf) {
                    m_dist.max(prev - 1.0 / f64::from(self.cfg.morph_frames))
                } else {
                    parent_buf.clear();
                    self.topo.parents(leaf, &mut parent_buf);
                    if parent_buf.iter().any(|q| self.prev_drawn.contains(q)) {
                        refine_cells.push(leaf);
                        1.0
                    } else {
                        m_dist
                    }
                };
                new_shown.insert(leaf, m);
                m
            };
            let record = |drawn: &mut Vec<T::Cell>, set: &mut BTreeSet<T::Cell>| {
                if drawn.last() != Some(&leaf) {
                    drawn.push(leaf);
                }
                set.insert(leaf);
            };
            if let Some(view) = &self.last_view {
                let fp = self.topo.footprint(leaf);
                let visible = horizon_visible(self.model.radius, view.eye, &fp, h.range, occluder)
                    && (eye_in_cone(view.eye, &fp)
                        || frustum_visible(view, &cell_bound(self.model.radius, &fp, h.range)));
                if !visible {
                    culled += 1;
                    if self.cfg.diagnostics {
                        culled_cells.insert(leaf);
                    }
                    // Culled leaves still belong to the drawn partition.
                    record(&mut self.drawn, &mut self.drawn_set);
                    continue;
                }
            }
            if scene.items.len() >= MAX_DRAWS_PER_FRAME {
                return Err(SdkError::App(format!(
                    "draw budget exceeded: {} items",
                    scene.items.len() + 1
                )));
            }
            let Some(&(class, first, count)) = self.variant_ranges.get(usize::from(piece.variant))
            else {
                return Err(SdkError::App(format!(
                    "piece variant {} out of range",
                    piece.variant
                )));
            };
            let Some(Some(shared)) = self.shared.get(usize::from(class)) else {
                return Err(SdkError::App(
                    "shared index buffer missing (setup not run)".into(),
                ));
            };
            let mesh = h.mesh.with_index_range(shared, first, count)?;
            let _ = self.manager.resolve(&h.key);
            let position =
                WorldPos::from_xyz(scene.frames.root(), h.origin.x, h.origin.y, h.origin.z)?;
            scene.items.push(
                RenderItem::new(mesh, position)
                    .with_material(self.material)
                    .with_morph(morph as f32),
            );
            record(&mut self.drawn, &mut self.drawn_set);
        }

        // Morph bookkeeping: counters, then roll the per-frame state.
        let defects = refine_cells
            .iter()
            .filter(|c| new_shown.get(c).is_none_or(|&m| m != 1.0))
            .count();
        self.report.morph_first_frame_defects = u32::try_from(defects).unwrap_or(u32::MAX);
        if new_shown.is_empty() {
            self.report.morph_min = 0.0;
            self.report.morph_mean = 0.0;
        } else {
            let mut lo = 1.0f64;
            let mut sum = 0.0f64;
            for &m in new_shown.values() {
                lo = lo.min(m);
                sum += m;
            }
            self.report.morph_min = lo;
            self.report.morph_mean = sum / new_shown.len() as f64;
        }
        let mut gap_max = 0.0f64;
        let mut replaced_deep: BTreeSet<T::Cell> = BTreeSet::new();
        let mut children: Vec<T::Cell> = Vec::new();
        for c in self.prev_drawn.difference(&self.drawn_set) {
            children.clear();
            self.topo.children(*c, &mut children);
            if children.iter().any(|k| self.drawn_set.contains(k)) {
                gap_max = gap_max.max(self.morph_shown.get(c).copied().unwrap_or(0.0));
            }
        }
        for d in self.drawn_set.difference(&self.prev_drawn) {
            parent_buf.clear();
            self.topo.parents(*d, &mut parent_buf);
            if parent_buf
                .iter()
                .any(|q| self.prev_drawn.contains(q) && !self.drawn_set.contains(q))
            {
                // A direct parent was replaced: an ordinary one-level handover.
                continue;
            }
            // Walk up to the first previously drawn ancestor that is gone now; two or more
            // levels up is a multi-level handover (the single-parent target cannot be exact).
            let mut cur = self.topo.primary_parent(*d);
            let mut depth = 1u32;
            while let Some(a) = cur {
                if self.prev_drawn.contains(&a) && !self.drawn_set.contains(&a) {
                    if depth >= 2 {
                        replaced_deep.insert(a);
                        gap_max = gap_max.max(self.morph_shown.get(&a).copied().unwrap_or(0.0));
                    }
                    break;
                }
                cur = self.topo.primary_parent(a);
                depth += 1;
            }
        }
        self.report.handover_gap_max = gap_max;
        self.report.multi_level_handovers = u32::try_from(replaced_deep.len()).unwrap_or(u32::MAX);
        self.prev_drawn.clone_from(&self.drawn_set);
        self.morph_shown = new_shown;

        if self.cfg.diagnostics {
            let sel = &self.report.select;
            let flags = ChurnFlags {
                budget_or_trim: sel.trimmed_splits > 0
                    || sel.node_budget_hit
                    || sel.leaf_budget_hit
                    || sel.retain_rejected > 0
                    || self.report.retain_expired > 0
                    || eff.coarsened > 0,
            };
            let culled_for_diag = culled_cells.clone();
            let c = self.churn.observe(
                &*self.topo,
                self.frame,
                &eff.pieces,
                &self.drawn_set,
                culled_cells,
                &self.drawn_split,
                flags,
                &destroyed_ids,
            );
            self.report.unsplit_culled_drawn_visible = u32::try_from(
                self.unsplit_culled
                    .iter()
                    .filter(|x| self.drawn_set.contains(x) && !culled_for_diag.contains(x))
                    .count(),
            )
            .unwrap_or(u32::MAX);
            self.report.level_changes = c.level_changes;
            self.report.drawn_appeared = c.drawn_appeared;
            self.report.drawn_disappeared = c.drawn_disappeared;
            self.report.cull_toggles = c.cull_toggles;
            self.report.overlap_pairs = c.overlap_pairs;
            self.report.flipflops = c.flipflops;
            self.report.flipflops_unexplained = c.flipflops_unexplained;
        }
        self.report.effective_coarsened = eff.coarsened;
        self.report.frame = self.frame;
        self.report.stream = stream;
        self.report.drawn_leaves = self.drawn.len();
        self.report.drawn_max_jump = max_level_jump(
            &*self.topo,
            &eff.pieces.iter().map(|p| p.cell).collect::<Vec<_>>(),
        );
        self.report.held_meshes = self.held.len();
        self.report.upload_rejects = upload_rejects;
        self.report.destroyed = destroyed;
        self.report.draw_items = scene.items.len();
        self.report.culled_draws = culled;
        self.report.closure_cells = self.closure.len();
        self.report.closure_resident = closure_resident;
        Ok(())
    }

    #[must_use]
    pub fn report(&self) -> &TerrainReport {
        &self.report
    }

    /// Leaves drawn in the last `gpu_frame`, ascending.
    #[must_use]
    pub fn drawn(&self) -> &[T::Cell] {
        &self.drawn
    }

    /// Split cells of the drawn cut of the last `gpu_frame` (the pieces are
    /// `Topology::pieces` of this set, restricted to resident cells).
    #[must_use]
    pub fn drawn_split(&self) -> &BTreeSet<T::Cell> {
        &self.drawn_split
    }

    /// Morph factor shown for `cell` in the last `gpu_frame`, `None` when it was not drawn.
    #[must_use]
    pub fn drawn_morph(&self, cell: T::Cell) -> Option<f64> {
        self.morph_shown.get(&cell).copied()
    }

    /// Leaves wanted by the last `prepare`, ascending.
    #[must_use]
    pub fn wanted_leaves(&self) -> &[T::Cell] {
        &self.wanted
    }

    #[must_use]
    pub fn held_mesh_count(&self) -> usize {
        self.held.len()
    }

    /// Planet radius plus terrain height in direction `dir`.
    #[must_use]
    pub fn surface_radius_at(&self, dir: DVec3) -> f64 {
        self.cfg.terrain.radius + self.terrain.height_at_dir(dir)
    }

    /// Geometric error of the drawn leaf containing `dir`, 0.0 if none is drawn there.
    #[must_use]
    pub fn drawn_error_at(&self, dir: DVec3) -> f64 {
        for level in 0..=self.cfg.lod.max_level {
            if self.drawn_set.contains(&self.topo.locate(dir, level)) {
                return self.model.geometric_error(level);
            }
        }
        0.0
    }

    /// Stops the worker pool.
    pub fn shutdown(self) {
        self.jobs.shutdown();
    }
}

#[cfg(test)]
mod palette_tests {
    use super::*;

    #[test]
    fn default_config_is_generator_v2() {
        assert_eq!(TerrainConfig::default().terrain.generator_version, 2);
    }

    #[test]
    fn palette_follows_the_generator_version() {
        let v2 = TerrainParams::demo_v2();
        let v1 = TerrainParams::default();
        let (_, rows, data) = palette_for(&v2, false);
        assert_eq!((rows, data.len()), (8, (PALETTE_W * 8 * 4) as usize));
        let (_, rows, data) = palette_for(&v1, false);
        assert_eq!((rows, data.len()), (4, (PALETTE_W * 4 * 4) as usize));
        // The debug palette wins for both versions.
        for p in [&v1, &v2] {
            let (_, rows, data) = palette_for(p, true);
            assert_eq!((rows, data.len()), (16, (PALETTE_W * 16 * 4) as usize));
        }
    }
}
