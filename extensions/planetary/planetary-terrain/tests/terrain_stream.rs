//! Headless streaming integration through `Runtime` + `App::gpu_frame` on `NullBackend`.
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;

use engine_sdk::streaming::Uploader;
use engine_sdk::{
    App, AppControl, DVec3, Engine, EngineConfig, MAX_DRAWS_PER_FRAME, NullBackend, Resources,
    Runtime, SceneState, SdkError,
};
use planetary_core::lod::{LodParams, View, is_partition, select};
use planetary_core::{Face, PatchId};
use planetary_terrain::{
    CameraParams, DRAW_MARGIN, GpuUploader, PatchKey, PlanetaryCamera, TAN_HALF_FOV_Y,
    TerrainConfig, TerrainReport, TerrainSystem,
};

const RADIUS: f64 = 6_371_000.0;

struct Harness {
    sys: TerrainSystem,
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
        sys: TerrainSystem::new(cfg).expect("terrain system"),
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
            2 * app.sys.held_mesh_count(),
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
        let key = PatchKey::new(1, PatchId::root(Face::PosX))?;
        let mut up = GpuUploader {
            resources: r,
            held: &mut held,
            cells_per_edge: self.cells,
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
        assert!(TerrainSystem::new(cfg).is_err(), "max_leaves {max_leaves}");
    }
    for max_leaves in [6usize, MAX_DRAWS_PER_FRAME - DRAW_MARGIN] {
        let mut cfg = test_cfg();
        cfg.lod.max_leaves = max_leaves;
        TerrainSystem::new(cfg).expect("valid").shutdown();
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
    let none = select(
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
