//! Headless streaming integration through `Runtime` + `App::gpu_frame` on `NullBackend`.
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;

use engine_sdk::streaming::Uploader;
use engine_sdk::{
    App, AppControl, DVec3, Engine, EngineConfig, MAX_DRAWS_PER_FRAME, NullBackend, Resources,
    Runtime, SceneState, SdkError,
};
use planetary_core::lod::{LodParams, View, is_partition};
use planetary_core::topology::select::select_cells;
use planetary_core::{CubeSphere, Face, PatchId};
use planetary_terrain::{
    BudgetLimits, CameraParams, DRAW_MARGIN, GpuUploader, PatchKey, PlanetaryCamera, StreamBudget,
    TAN_HALF_FOV_Y, TerrainConfig, TerrainReport, TerrainSystem,
};

const RADIUS: f64 = 6_371_000.0;

struct Harness {
    sys: TerrainSystem<CubeSphere>,
    eyes: Vec<DVec3>,
    frame: usize,
    frustum: bool,
}

impl App for Harness {
    fn setup(&mut self, r: &mut Resources<'_>, s: &mut SceneState) -> Result<(), SdkError> {
        self.sys.setup(r, s)
    }

    fn update(&mut self, _dt: f64, _s: &mut SceneState) -> AppControl {
        let eye = self.eyes[self.frame.min(self.eyes.len() - 1)];
        let mut view = View {
            eye,
            proj_scale: 720.0 / (2.0 * TAN_HALF_FOV_Y),
            frustum: None,
        };
        if self.frustum {
            let cam = PlanetaryCamera::new(eye, CameraParams::default()).expect("camera");
            view = view.with_frustum(cam.frustum_planes(16.0 / 9.0, 10.0));
        }
        self.sys.prepare(&view);
        self.frame += 1;
        AppControl::Continue
    }

    fn gpu_frame(&mut self, r: &mut Resources<'_>, s: &mut SceneState) -> Result<(), SdkError> {
        self.sys.gpu_frame(r, s)
    }
}

fn test_cfg() -> TerrainConfig {
    TerrainConfig {
        cells_per_edge: 8,
        lod: LodParams {
            max_level: 10,
            ..Default::default()
        },
        settle_each_frame: true,
        workers: 2,
        ..Default::default()
    }
}

fn start(cfg: TerrainConfig, eyes: Vec<DVec3>) -> (Runtime<NullBackend>, Harness) {
    let mut rt =
        Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).expect("runtime");
    let mut app = Harness {
        sys: TerrainSystem::new(cfg.clone(), CubeSphere::new(cfg.cells_per_edge))
            .expect("terrain system"),
        eyes,
        frame: 0,
        frustum: false,
    };
    rt.setup(&mut app).expect("setup");
    (rt, app)
}

fn step(rt: &mut Runtime<NullBackend>, app: &mut Harness) {
    rt.frame(app, 1.0 / 60.0).expect("frame");
}

fn above_pos_x() -> Vec<DVec3> {
    vec![DVec3::new(RADIUS + 1000.0, 0.0, 0.0)]
}

fn orbit(n: usize, altitude: f64) -> Vec<DVec3> {
    (0..n)
        .map(|i| {
            let a = (i as f64) * core::f64::consts::TAU / (n as f64);
            DVec3::new(a.cos(), a.sin() * 0.6, a.sin() * 0.8) * (RADIUS + altitude)
        })
        .collect()
}

#[test]
fn warm_up_draws_roots_then_partition() {
    let (mut rt, mut app) = start(test_cfg(), above_pos_x());
    for f in 0..60 {
        step(&mut rt, &mut app);
        if f >= 10 {
            let r = app.sys.report();
            assert!(r.drawn_leaves >= 6, "frame {f}: drawn {}", r.drawn_leaves);
            assert!(is_partition(app.sys.drawn()), "frame {f}");
            assert!(r.drawn_max_jump <= 1, "frame {f}");
        }
    }
    app.sys.shutdown();
}

#[test]
fn converges_to_wanted() {
    let (mut rt, mut app) = start(test_cfg(), above_pos_x());
    let mut done = false;
    for _ in 0..400 {
        step(&mut rt, &mut app);
        if app.sys.drawn() == app.sys.wanted_leaves() && app.sys.report().requests_issued == 0 {
            done = true;
            break;
        }
    }
    assert!(done, "did not converge");
    app.sys.shutdown();
}

#[test]
fn per_frame_caps_hold() {
    let cfg = test_cfg();
    let (mut rt, mut app) = start(cfg.clone(), orbit(200, 2000.0));
    for _ in 0..200 {
        step(&mut rt, &mut app);
        let r: &TerrainReport = app.sys.report();
        assert!(r.stream.started <= cfg.stream.max_starts_per_frame);
        assert!(r.stream.completions_applied <= cfg.stream.max_completions_per_frame);
        assert!(r.stream.uploads <= cfg.uploads_per_frame);
        assert!(r.requests_issued <= cfg.max_requests_per_frame);
        assert!(r.drawn_leaves <= MAX_DRAWS_PER_FRAME);
        assert!(r.draw_items <= MAX_DRAWS_PER_FRAME);
        assert_eq!(r.request_errors, 0);
    }
    app.sys.shutdown();
}

#[test]
fn eviction_reconciles_without_leak() {
    let cfg = TerrainConfig {
        detail_budget_patches: 40,
        ..test_cfg()
    };
    let (mut rt, mut app) = start(cfg, orbit(600, 2000.0));
    let mut evicted = 0u64;
    for _ in 0..600 {
        step(&mut rt, &mut app);
        evicted += u64::from(app.sys.report().stream.evicted);
        assert_eq!(
            rt.backend().buffer_count(),
            app.sys.held_mesh_count() + 1,
            "buffer leak or dangling mesh"
        );
    }
    assert!(evicted > 0, "no eviction happened");
    app.sys.shutdown();
}

#[test]
fn stale_requests_are_cancelled() {
    let mut cfg = test_cfg();
    cfg.stream.max_starts_per_frame = 1;
    let a = DVec3::new(RADIUS + 1000.0, 0.0, 0.0);
    let b = -a;
    let mut eyes: Vec<DVec3> = Vec::new();
    for i in 0..12 {
        let e = if i % 2 == 0 { a } else { b };
        eyes.extend(std::iter::repeat_n(e, 5));
    }
    let jump_frames = eyes.len();
    eyes.push(a);
    let (mut rt, mut app) = start(cfg, eyes);
    let mut cancelled = 0u64;
    for _ in 0..jump_frames {
        step(&mut rt, &mut app);
        cancelled += u64::from(app.sys.report().requests_cancelled);
    }
    assert!(cancelled > 0, "no request was cancelled");
    let mut done = false;
    for _ in 0..1500 {
        step(&mut rt, &mut app);
        if app.sys.drawn() == app.sys.wanted_leaves() && app.sys.report().requests_issued == 0 {
            done = true;
            break;
        }
    }
    assert!(
        done,
        "did not converge after the eye stopped: wanted {} drawn {} held {} rep {:?}",
        app.sys.wanted_leaves().len(),
        app.sys.drawn().len(),
        app.sys.held_mesh_count(),
        app.sys.report()
    );
    app.sys.shutdown();
}

type Trace = Vec<(u32, u32, Vec<PatchId>)>;

fn run_trace() -> Trace {
    let (mut rt, mut app) = start(test_cfg(), orbit(200, 2000.0));
    let mut out = Vec::new();
    for _ in 0..200 {
        step(&mut rt, &mut app);
        let r = app.sys.report();
        out.push((r.stream.started, r.stream.uploads, app.sys.drawn().to_vec()));
    }
    app.sys.shutdown();
    out
}

#[test]
fn deterministic_twice() {
    assert_eq!(run_trace(), run_trace());
}

struct Corrupt {
    cells: u32,
    checked: bool,
    before: usize,
    after: usize,
    rejected: usize,
    accepted: bool,
}

impl App for Corrupt {
    fn setup(&mut self, _r: &mut Resources<'_>, _s: &mut SceneState) -> Result<(), SdkError> {
        Ok(())
    }

    fn update(&mut self, _dt: f64, _s: &mut SceneState) -> AppControl {
        AppControl::Continue
    }

    fn gpu_frame(&mut self, r: &mut Resources<'_>, _s: &mut SceneState) -> Result<(), SdkError> {
        let mut held = BTreeMap::new();
        let key = PatchKey::new(
            &std::sync::Arc::new(planetary_core::CubeSphere::new(32)),
            1,
            PatchId::root(Face::PosX),
        )?;
        let counts = [Some(self.cells)];
        let mut up = GpuUploader {
            resources: r,
            held: &mut held,
            vertex_counts: &counts,
            rejected: Vec::new(),
        };
        self.accepted = up.upload(&key, &[1, 2, 3]);
        self.rejected = up.rejected.len();
        self.after = held.len();
        self.before = 0;
        self.checked = true;
        Ok(())
    }
}

#[test]
fn corrupt_payload_rejected() {
    let mut rt =
        Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).expect("runtime");
    let mut app = Corrupt {
        cells: 8,
        checked: false,
        before: 1,
        after: 1,
        rejected: 0,
        accepted: false,
    };
    rt.setup(&mut app).expect("setup");
    rt.frame(&mut app, 1.0 / 60.0).expect("frame");
    assert!(app.checked);
    assert!(app.accepted, "corrupt payload must not be retried");
    assert_eq!(app.rejected, 1);
    assert_eq!(app.after, app.before);
    assert_eq!(rt.backend().buffer_count(), 0);
}

#[test]
fn config_rejects_draw_budget_overflow() {
    for max_leaves in [
        5usize,
        MAX_DRAWS_PER_FRAME - DRAW_MARGIN + 1,
        MAX_DRAWS_PER_FRAME,
    ] {
        let mut cfg = test_cfg();
        cfg.lod.max_leaves = max_leaves;
        assert!(
            TerrainSystem::new(cfg.clone(), CubeSphere::new(cfg.cells_per_edge)).is_err(),
            "max_leaves {max_leaves}"
        );
    }
    for max_leaves in [6usize, MAX_DRAWS_PER_FRAME - DRAW_MARGIN] {
        let mut cfg = test_cfg();
        cfg.lod.max_leaves = max_leaves;
        TerrainSystem::new(cfg.clone(), CubeSphere::new(cfg.cells_per_edge))
            .expect("valid")
            .shutdown();
    }
}

#[test]
fn resident_ranges_reduce_node_visits() {
    let (mut rt, mut app) = start(test_cfg(), above_pos_x());
    let mut done = false;
    for _ in 0..400 {
        step(&mut rt, &mut app);
        if app.sys.drawn() == app.sys.wanted_leaves() && app.sys.report().requests_issued == 0 {
            done = true;
            break;
        }
    }
    assert!(done, "did not converge");
    assert_eq!(app.sys.drawn(), app.sys.wanted_leaves());
    let view = View {
        eye: above_pos_x()[0],
        proj_scale: 720.0 / (2.0 * TAN_HALF_FOV_Y),
        frustum: None,
    };
    let none = select_cells(
        &CubeSphere::new(app.sys.config().cells_per_edge),
        app.sys.model(),
        &app.sys.config().lod,
        &view,
        &Default::default(),
        &|_| None,
    );
    let with = app.sys.report().select.nodes_visited;
    assert!(
        with < none.stats.nodes_visited,
        "resident ranges {with} vs global {}",
        none.stats.nodes_visited
    );
    app.sys.shutdown();
}

#[test]
fn draw_items_plus_culled_equals_drawn() {
    let (mut rt, mut app) = start(test_cfg(), orbit(200, 2000.0));
    app.frustum = true;
    let mut culled_any = false;
    for _ in 0..200 {
        step(&mut rt, &mut app);
        let r = app.sys.report();
        assert_eq!(r.draw_items + r.culled_draws as usize, r.drawn_leaves);
        assert!(r.draw_items <= r.drawn_leaves);
        culled_any |= r.culled_draws > 0;
    }
    assert!(culled_any, "no draw was ever culled");
    app.sys.shutdown();
}

#[test]
fn held_meshes_bounded_while_hovering() {
    let cfg = TerrainConfig {
        detail_budget_patches: 60,
        ..test_cfg()
    };
    let base_levels = u32::from(cfg.base_levels);
    let base = 6 * ((1u64 << (2 * (base_levels + 1))) - 1) / 3 + 16;
    let bound = usize::try_from(base + cfg.detail_budget_patches).expect("bound");
    // Slow drift 40 m above the surface: new cells keep entering the wanted set.
    let eyes: Vec<DVec3> = (0..600)
        .map(|i| {
            let a = f64::from(i) * 2.0e-6;
            DVec3::new(a.cos(), a.sin(), 0.0) * (RADIUS + 40.0)
        })
        .collect();
    let (mut rt, mut app) = start(cfg.clone(), eyes);
    let mut max_held = 0usize;
    for _ in 0..600 {
        step(&mut rt, &mut app);
        let r = app.sys.report();
        assert!(r.stream.pending <= cfg.stream.max_pending);
        assert!(app.sys.held_mesh_count() <= bound, "held {}", r.held_meshes);
        assert!(
            r.pending_requests <= bound,
            "requests {}",
            r.pending_requests
        );
        max_held = max_held.max(app.sys.held_mesh_count());
    }
    assert!(max_held > 0);
    app.sys.shutdown();
}

#[test]
fn shared_index_buffers_created_once() {
    let (mut rt, mut app) = start(test_cfg(), above_pos_x());
    // setup created exactly one shared index buffer (one cube vertex class), no meshes yet.
    assert_eq!(rt.backend().buffer_count(), 1, "after setup");
    for f in 0..2 {
        step(&mut rt, &mut app);
        assert_eq!(
            rt.backend().buffer_count(),
            app.sys.held_mesh_count() + 1,
            "frame {f}: held + classes"
        );
    }
    assert!(
        app.sys.held_mesh_count() > 0,
        "nothing uploaded in 2 frames"
    );
    app.sys.shutdown();
}

#[test]
fn debug_cells_uses_debug_material() {
    use engine_sdk::MaterialId;
    use engine_sdk::streaming::StreamSource;
    use planetary_core::lod::LodModel;
    use planetary_core::{PlanetTerrain, Topology, decode_cell_payload};
    use planetary_terrain::CellSource;
    use std::sync::Arc;

    let cfg = TerrainConfig {
        debug_cells: true,
        ..test_cfg()
    };
    let (mut rt, mut app) = start(cfg.clone(), above_pos_x());
    for _ in 0..30 {
        step(&mut rt, &mut app);
    }
    assert_ne!(app.sys.material(), MaterialId::DEFAULT);
    assert!(app.sys.report().draw_items > 0);
    app.sys.shutdown();

    // Per-cell row selector in uv[1] for debug cells, the terrain attribute otherwise.
    let topo = Arc::new(CubeSphere::new(cfg.cells_per_edge));
    let terrain = Arc::new(
        PlanetTerrain::new(cfg.world_seed, cfg.terrain, cfg.cells_per_edge).expect("terrain"),
    );
    let model = LodModel::for_topology(&terrain, cfg.kappa, topo.sample_angle0());
    let vc = topo.index_variants()[0]
        .indices
        .iter()
        .max()
        .copied()
        .expect("indices")
        + 1;
    let cell = PatchId::root(Face::PosX);
    let key = PatchKey::new(&topo, 1, cell).expect("key");
    let token = engine_sdk::CancelToken::new();
    let uv1 = |debug: bool| {
        let src = CellSource::new(topo.clone(), terrain.clone(), model, debug, vc);
        let bytes = src.produce(&key, &token).expect("payload");
        let m = decode_cell_payload(&bytes, key.raw(), &|c| (c == 0).then_some(vc)).expect("v3");
        m.vertices.iter().map(|v| v.uv[1]).collect::<Vec<f32>>()
    };
    let dbg = uv1(true);
    let row = ((u32::from(cell.level()) * 7).wrapping_add((cell.raw() >> 7) as u32) % 16) as f32;
    let want = (row + 0.5) / 16.0;
    assert!(
        dbg.iter().all(|v| (*v - want).abs() < 1e-6),
        "row selector {want}"
    );
    assert_ne!(dbg, uv1(false), "debug uv must differ from terrain uv");
}

fn adaptive_cfg() -> TerrainConfig {
    TerrainConfig {
        adaptive: true,
        ..test_cfg()
    }
}

#[test]
fn budget_grows_with_fast_frames_shrinks_with_slow_and_respects_hard_max() {
    let cfg = adaptive_cfg();
    let b = cfg.budget;
    let base = StreamBudget::base(&cfg);
    assert_eq!((base.uploads, base.starts, base.requests), (4, 8, 16));
    // Iterate the controller at a constant cost until it settles.
    let settle = |ms: f64| {
        let mut cur = base;
        for _ in 0..40 {
            cur = StreamBudget::next(&cfg, cur.scale, Some(ms));
        }
        cur
    };
    let fast = settle(0.01);
    assert_eq!(
        (fast.uploads, fast.starts, fast.requests),
        (b.max_uploads, b.max_starts, b.max_requests),
        "a very fast frame must reach the hard maxima and never exceed them"
    );
    let mid = settle(b.target_ms);
    assert_eq!(
        (mid.uploads, mid.starts, mid.requests),
        (base.uploads, base.starts, base.requests),
        "cost at target keeps the base caps"
    );
    let slow = settle(10_000.0);
    assert_eq!((slow.uploads, slow.starts, slow.requests), (1, 2, 4));
    assert!(slow.uploads < base.uploads && base.uploads < fast.uploads);
    // One step moves only part of the way (damped).
    let one = StreamBudget::next(&cfg, 1.0, Some(4.0));
    assert!(one.uploads > base.uploads && one.uploads < fast.uploads);
}

#[test]
fn budget_absent_invalid_or_disabled_gives_the_base_caps() {
    let cfg = adaptive_cfg();
    let base = StreamBudget::base(&cfg);
    for ms in [None, Some(f64::NAN), Some(-1.0), Some(f64::INFINITY)] {
        assert_eq!(StreamBudget::next(&cfg, 3.0, ms), base, "{ms:?}");
    }
    let off = TerrainConfig {
        adaptive: false,
        ..cfg.clone()
    };
    assert_eq!(StreamBudget::next(&off, 1.0, Some(0.01)), base);
    // Invalid limits fall back to the base caps instead of dividing by zero.
    let bad = TerrainConfig {
        budget: BudgetLimits {
            target_ms: 0.0,
            ..cfg.budget
        },
        ..cfg
    };
    assert_eq!(StreamBudget::next(&bad, 1.0, Some(1.0)), base);
    assert!(
        TerrainSystem::new(
            TerrainConfig {
                budget: BudgetLimits {
                    max_uploads: 0,
                    ..BudgetLimits::default()
                },
                ..adaptive_cfg()
            },
            CubeSphere::new(8)
        )
        .is_err()
    );
}

#[test]
fn absent_measurement_keeps_the_default_caps_in_the_system() {
    let cfg = adaptive_cfg();
    let (mut rt, mut app) = start(cfg.clone(), orbit(120, 2000.0));
    for _ in 0..120 {
        step(&mut rt, &mut app);
        let r = app.sys.report();
        assert_eq!(app.sys.stream_budget(), StreamBudget::base(&cfg));
        assert!(r.stream.uploads <= cfg.uploads_per_frame);
        assert!(r.requests_issued <= cfg.max_requests_per_frame);
    }
    app.sys.shutdown();
}

#[test]
fn adaptive_budget_respects_hard_max_and_throttles_when_slow() {
    let cfg = adaptive_cfg();
    let hard = cfg.budget;
    let (mut rt, mut app) = start(cfg.clone(), orbit(300, 2000.0));
    let mut max_up = 0u32;
    for _ in 0..100 {
        app.sys.set_frame_cost(0.01);
        step(&mut rt, &mut app);
        let r = app.sys.report();
        let b = app.sys.stream_budget();
        assert!(b.uploads <= hard.max_uploads && b.starts <= hard.max_starts);
        assert!(b.requests <= hard.max_requests);
        assert!(r.stream.uploads <= b.uploads, "uploads over the cap");
        assert!(r.stream.started <= hard.max_starts, "started over hard max");
        assert!(r.stream.completions_applied <= hard.max_starts);
        assert!(r.requests_issued <= b.requests);
        assert!(is_partition(app.sys.drawn()) || r.drawn_leaves == 0);
        max_up = max_up.max(r.stream.uploads);
    }
    assert!(
        max_up > cfg.uploads_per_frame,
        "fast frames never raised the upload rate above the base ({max_up})"
    );
    for f in 0..100 {
        app.sys.set_frame_cost(10_000.0);
        step(&mut rt, &mut app);
        let r = app.sys.report();
        let b = app.sys.stream_budget();
        assert!(r.stream.uploads <= b.uploads, "uploads over the cap");
        assert!(r.requests_issued <= b.requests);
        if f >= 20 {
            assert_eq!((b.uploads, b.starts, b.requests), (1, 2, 4));
            assert!(
                r.stream.uploads <= 1,
                "slow frame uploaded {}",
                r.stream.uploads
            );
            assert!(r.stream.started <= 2 * b.starts, "queue bound");
        }
        assert!(is_partition(app.sys.drawn()) || r.drawn_leaves == 0);
    }
    app.sys.shutdown();
}

#[test]
fn partition_holds_under_throttling_and_eviction() {
    let cfg = TerrainConfig {
        detail_budget_patches: 60,
        ..adaptive_cfg()
    };
    let (mut rt, mut app) = start(cfg, orbit(400, 800.0));
    let mut evicted = 0u64;
    for f in 0..400 {
        // Alternate fast and slow measurements so the caps keep changing.
        app.sys
            .set_frame_cost(if (f / 25) % 2 == 0 { 0.01 } else { 1.0e4 });
        step(&mut rt, &mut app);
        let r = app.sys.report();
        evicted += u64::from(r.stream.evicted);
        if f >= 10 {
            assert!(!app.sys.drawn().is_empty(), "f{f} empty drawn set");
            assert!(is_partition(app.sys.drawn()), "f{f} not a partition");
            assert!(r.drawn_max_jump <= 1, "f{f} jump");
        }
    }
    assert!(evicted > 0, "eviction never happened");
    app.sys.shutdown();
}
