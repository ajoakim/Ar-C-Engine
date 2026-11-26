//! Terrain streaming system: LOD selection, request planning, stream update, eviction
//! reconcile, effective cut and the camera-relative draw list.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use engine_sdk::streaming::{
    ClassBudget, FrameReport, ResidencyManager, ResidencyState, StreamConfig, StreamError,
    StreamManager, StreamRequest,
};
use engine_sdk::{
    CancelToken, DVec3, JobSystem, MAX_DRAWS_PER_FRAME, Material, MaterialId, Quat, RenderItem,
    Resources, SceneState, SdkError, Vec3, WorldPos,
};
use planetary_core::lod::{
    DEFAULT_KAPPA, LodModel, LodParams, SelectStats, View, max_level_jump, select,
};
use planetary_core::{MAX_LEVEL, PatchId, PlanetTerrain, TerrainParams, locate, payload_len};

use crate::effective::effective_cut;
use crate::key::PatchKey;
use crate::palette::{PALETTE_H, PALETTE_W, palette_rgba8};
use crate::requests::{plan_requests, priority_for_level, stale_requests, wanted_closure};
use crate::source::PatchSource;
use crate::uploader::{GpuUploader, HeldPatch};
use crate::{BASE_CLASS, DEMO_SEED, DETAIL_CLASS};

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
    pub draw_overflow: u32,
}

/// Streams, uploads and draws the planet terrain.
pub struct TerrainSystem {
    cfg: TerrainConfig,
    terrain: Arc<PlanetTerrain>,
    model: LodModel,
    manager: StreamManager<PatchKey, PatchSource>,
    jobs: JobSystem,
    held: BTreeMap<PatchId, HeldPatch>,
    requested: BTreeMap<PatchId, (PatchKey, CancelToken)>,
    prev_split: BTreeSet<PatchId>,
    wanted_split: BTreeSet<PatchId>,
    wanted: Vec<PatchId>,
    drawn: Vec<PatchId>,
    drawn_set: BTreeSet<PatchId>,
    material: MaterialId,
    frame: u64,
    report: TerrainReport,
}

fn app_err(what: &str, e: impl std::fmt::Display) -> SdkError {
    SdkError::App(format!("{what}: {e}"))
}

impl TerrainSystem {
    /// Validates the configuration and builds the terrain, stream manager and worker pool.
    ///
    /// # Errors
    /// `SdkError::App` for an invalid configuration, `SdkError::Job` if workers cannot start.
    pub fn new(cfg: TerrainConfig) -> Result<Self, SdkError> {
        if cfg.lod.max_level > MAX_LEVEL {
            return Err(SdkError::App("lod.max_level exceeds MAX_LEVEL".into()));
        }
        if cfg.workers < 1 {
            return Err(SdkError::App("workers must be >= 1".into()));
        }
        let terrain = Arc::new(
            PlanetTerrain::new(cfg.world_seed, cfg.terrain, cfg.cells_per_edge)
                .map_err(|e| app_err("terrain", e))?,
        );
        let model = LodModel::from_terrain(&terrain, cfg.kappa);
        let bytes = payload_len(cfg.cells_per_edge) as u64;
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
        let source = Arc::new(PatchSource::new(terrain.clone(), model));
        let mut manager = StreamManager::new(cfg.stream, residency, source)
            .map_err(|e| app_err("stream manager", e))?;
        manager.set_deterministic(cfg.settle_each_frame);
        let jobs = JobSystem::builder().workers(cfg.workers).build()?;
        Ok(Self {
            cfg,
            terrain,
            model,
            manager,
            jobs,
            held: BTreeMap::new(),
            requested: BTreeMap::new(),
            prev_split: BTreeSet::new(),
            wanted_split: BTreeSet::new(),
            wanted: Vec::new(),
            drawn: Vec::new(),
            drawn_set: BTreeSet::new(),
            material: MaterialId::DEFAULT,
            frame: 0,
            report: TerrainReport::default(),
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

    /// Creates the palette texture and terrain material.
    ///
    /// # Errors
    /// Propagates texture and material creation errors.
    pub fn setup(
        &mut self,
        resources: &mut Resources<'_>,
        scene: &mut SceneState,
    ) -> Result<(), SdkError> {
        let tex =
            resources.create_texture("planet-palette", PALETTE_W, PALETTE_H, &palette_rgba8())?;
        self.material = scene.add_material(Material {
            base_color: [1.0; 4],
            base_color_texture: Some(tex),
        })?;
        Ok(())
    }

    /// `App::update` step: select (with hysteresis), cancel stale requests, issue capped requests.
    pub fn prepare(&mut self, view: &View) {
        let mut rep = TerrainReport {
            frame: self.frame,
            ..TerrainReport::default()
        };
        let sel = select(&self.model, &self.cfg.lod, view, &self.prev_split, &|_| {
            None
        });
        self.prev_split.clone_from(&sel.split);
        self.wanted_split = sel.split;
        self.wanted = sel.leaves;
        rep.select = sel.stats;
        rep.wanted_leaves = self.wanted.len();

        // Requests that finished (resident) or failed/were evicted no longer count as pending.
        let manager = &self.manager;
        self.requested.retain(|_, (key, _)| {
            !matches!(manager.state(key), None | Some(ResidencyState::GpuResident))
        });

        let closure = wanted_closure(&self.wanted);
        let ids: BTreeSet<PatchId> = self.requested.keys().copied().collect();
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
                &self.wanted,
                &|id| held.contains_key(&id),
                &|id| requested.contains_key(&id),
                self.cfg.max_requests_per_frame as usize,
            )
        };
        for id in planned {
            let Ok(key) = PatchKey::new(self.cfg.planet, id) else {
                rep.request_errors += 1;
                continue;
            };
            let token = CancelToken::new();
            let class = if id.level() <= self.cfg.base_levels {
                BASE_CLASS
            } else {
                DETAIL_CLASS
            };
            let req = StreamRequest::new(key.clone(), priority_for_level(id.level()), class)
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
            cells_per_edge: self.cfg.cells_per_edge,
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
        let stale: Vec<PatchId> = self
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
        let eff = effective_cut(&self.wanted_split, &|id| held.contains_key(&id));
        scene.items.clear();
        self.drawn.clear();
        self.drawn_set.clear();
        let mut overflow = 0u32;
        for &leaf in &eff.leaves {
            let Some(h) = self.held.get(&leaf) else {
                continue;
            };
            if scene.items.len() >= MAX_DRAWS_PER_FRAME {
                overflow += 1;
                continue;
            }
            let _ = self.manager.resolve(&h.key);
            let position =
                WorldPos::from_xyz(scene.frames.root(), h.origin.x, h.origin.y, h.origin.z)?;
            scene.items.push(RenderItem {
                mesh: h.mesh,
                position,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                material: self.material,
            });
            self.drawn.push(leaf);
            self.drawn_set.insert(leaf);
        }

        self.report.frame = self.frame;
        self.report.stream = stream;
        self.report.drawn_leaves = self.drawn.len();
        self.report.drawn_max_jump = max_level_jump(&eff.leaves);
        self.report.held_meshes = self.held.len();
        self.report.upload_rejects = upload_rejects;
        self.report.destroyed = destroyed;
        self.report.draw_overflow = overflow;
        Ok(())
    }

    #[must_use]
    pub fn report(&self) -> &TerrainReport {
        &self.report
    }

    /// Leaves drawn in the last `gpu_frame`, ascending.
    #[must_use]
    pub fn drawn(&self) -> &[PatchId] {
        &self.drawn
    }

    /// Leaves wanted by the last `prepare`, ascending.
    #[must_use]
    pub fn wanted_leaves(&self) -> &[PatchId] {
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
            if self.drawn_set.contains(&locate(dir, level)) {
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
