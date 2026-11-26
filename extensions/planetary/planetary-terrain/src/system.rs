//! Terrain streaming system: LOD selection, request planning, stream update, eviction
//! reconcile, effective cut and the camera-relative draw list.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use engine_sdk::streaming::{
    ClassBudget, FrameReport, ResidencyManager, ResidencyState, StreamConfig, StreamError,
    StreamManager, StreamRequest,
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
use crate::requests::{plan_requests, priority_for_level, stale_requests, wanted_closure};
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
        let mut residency = ResidencyManager::new(u64::from(cfg.uploads_per_frame) * bytes);
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
        let mut manager = StreamManager::new(cfg.stream, residency, source)
            .map_err(|e| app_err("stream manager", e))?;
        manager.set_deterministic(cfg.settle_each_frame);
        let jobs = JobSystem::builder().workers(cfg.workers).build()?;
        Ok(Self {
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

        let planned = {
            let held = &self.held;
            let requested = &self.requested;
            plan_requests(
                &*self.topo,
                &self.wanted,
                &|id| held.contains_key(&id),
                &|id| requested.contains_key(&id),
                self.cfg.max_requests_per_frame as usize,
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
        let mut uploader = GpuUploader {
            resources: &mut *resources,
            held: &mut self.held,
            vertex_counts: &self.vertex_counts,
            rejected: Vec::new(),
        };
        let stream = self
            .manager
            .update(&self.jobs, self.frame, &mut uploader)
            .map_err(|e| app_err("stream update", e))?;
        let rejected = std::mem::take(&mut uploader.rejected);
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
