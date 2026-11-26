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
    Quat, RenderItem, Resources, SceneState, SdkError, Vec3, WorldPos,
};
use planetary_core::lod::{
    DEFAULT_KAPPA, LodModel, LodParams, SelectStats, View, cell_bound, eye_in_cone,
    frustum_visible, horizon_visible,
};
use planetary_core::topology::select::select_cells;
use planetary_core::{MAX_LEVEL, PlanetTerrain, TerrainParams, Topology};

use crate::effective::effective_cut;
use crate::key::CellKey;
use crate::palette::{DEBUG_PALETTE_H, PALETTE_H, PALETTE_W, debug_palette_rgba8, palette_rgba8};
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
            max_uploads: 16,
            max_starts: 32,
            max_requests: 64,
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

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            planet: 1,
            world_seed: DEMO_SEED,
            terrain: TerrainParams::default(),
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
        let tex = if self.cfg.debug_cells {
            resources.create_texture(
                "planet-debug-palette",
                PALETTE_W,
                DEBUG_PALETTE_H,
                &debug_palette_rgba8(),
            )?
        } else {
            resources.create_texture("planet-palette", PALETTE_W, PALETTE_H, &palette_rgba8())?
        };
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
        let sel = {
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
        // Urgency of each piece that is not resident yet: nearest and largest screen-space error
        // first. Resident pieces need no request, so they are not scored.
        let scored: Vec<(T::Cell, f64)> = sel
            .pieces
            .iter()
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
            left: if self.cfg.adaptive {
                self.budget.uploads
            } else {
                u32::MAX
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
        for id in stale {
            if let Some(h) = self.held.remove(&id) {
                resources.destroy_mesh(h.mesh)?;
                destroyed += 1;
            }
        }

        let held = &self.held;
        let eff = effective_cut(&*self.topo, &self.wanted_split, &|id| {
            held.contains_key(&id)
        });
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
        let occluder = self.model.radius + self.model.height.min;
        for piece in &eff.pieces {
            let leaf = piece.cell;
            let Some(h) = self.held.get(&leaf) else {
                continue;
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
            scene.items.push(RenderItem {
                mesh,
                position,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                material: self.material,
            });
            record(&mut self.drawn, &mut self.drawn_set);
        }

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
