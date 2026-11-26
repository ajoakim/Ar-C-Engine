//! PLAN-07: scripted orbit-to-surface descent and surface-to-orbit ascent at the REAL default
//! terrain parameters (`TerrainConfig::default()`, max_level 16, 32 cells per edge), on a
//! simulated clock (no wall-clock time).
//!
//! Every frame is driven with a constant `dt = 1/60`; all assertions are counted work, never
//! durations.

use std::sync::OnceLock;

use engine_sdk::{DVec3, Engine, EngineConfig, MAX_DRAWS_PER_FRAME, NullBackend, Runtime};
use planetary_app::{AppSettings, DescentScript, PlanetaryApp};
use planetary_core::lod::{is_partition, max_level_jump};
use planetary_core::{Face, PatchId, locate, payload_len};
use planetary_terrain::{DRAW_MARGIN, TerrainConfig, TerrainReport};

/// Frames of a standard run: 8 s scripted path + 4 s hover at 60 Hz.
const FRAMES: u32 = 720;
/// Frame at which the scripted path ends (8 s).
const DESCENT_END_FRAME: u32 = 480;
/// Frames ignored before the first drawn set must exist.
const WARM_UP: usize = 10;
/// Frames after the descent ends within which drawn must equal wanted.
const CONVERGE_WITHIN: u32 = 240;

struct FrameRecord {
    report: TerrainReport,
    drawn: Vec<PatchId>,
    wanted: Vec<PatchId>,
    draws: usize,
    buffer_count: usize,
    held: usize,
    altitude: f64,
    /// Direction of the point 10 m ahead of the eye.
    look_dir: DVec3,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Path {
    Descent,
    Ascent,
}

fn real_terrain() -> TerrainConfig {
    TerrainConfig {
        settle_each_frame: true,
        ..TerrainConfig::default()
    }
}

fn base_settings(path: Path) -> AppSettings {
    let mut s = AppSettings {
        terrain: real_terrain(),
        ..Default::default()
    };
    let r = s.terrain.terrain.radius;
    s.script = Some(match path {
        Path::Descent => DescentScript::orbit_to_surface(r),
        Path::Ascent => DescentScript::surface_to_orbit(r),
    });
    s
}

fn run_path(path: Path, cfg_mut: impl Fn(&mut AppSettings), frames: u32) -> Vec<FrameRecord> {
    let mut settings = base_settings(path);
    cfg_mut(&mut settings);
    let mut app = PlanetaryApp::new(settings).expect("app");
    let mut rt =
        Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).expect("runtime");
    rt.setup(&mut app).expect("setup");
    let mut out = Vec::with_capacity(frames as usize);
    for _ in 0..frames {
        rt.frame(&mut app, 1.0 / 60.0).expect("frame");
        let draws = rt
            .backend()
            .last_frame()
            .map_or(0, |f| f.passes.iter().map(|p| p.draws.len()).sum());
        let altitude = app
            .camera()
            .altitude(&|d| app.terrain().surface_radius_at(d));
        let ahead = app.camera().position() + app.camera().forward() * 10.0;
        out.push(FrameRecord {
            report: app.report().clone(),
            drawn: app.terrain().drawn().to_vec(),
            wanted: app.terrain().wanted_leaves().to_vec(),
            draws,
            buffer_count: rt.backend().buffer_count(),
            held: app.terrain().held_mesh_count(),
            altitude,
            look_dir: ahead.normalize(),
        });
    }
    app.shutdown();
    out
}

fn run(cfg_mut: impl Fn(&mut AppSettings), frames: u32) -> Vec<FrameRecord> {
    run_path(Path::Descent, cfg_mut, frames)
}

/// The default descent run is deterministic in settle mode, so tests share one run.
fn shared_descent() -> &'static [FrameRecord] {
    static RUN: OnceLock<Vec<FrameRecord>> = OnceLock::new();
    RUN.get_or_init(|| run(|_| {}, FRAMES))
}

fn shared_ascent() -> &'static [FrameRecord] {
    static RUN: OnceLock<Vec<FrameRecord>> = OnceLock::new();
    RUN.get_or_init(|| run_path(Path::Ascent, |_| {}, FRAMES))
}

/// Level of the leaf in `set` that contains direction `d`, if any.
fn leaf_level_at(set: &[PatchId], d: DVec3) -> Option<u8> {
    set.iter()
        .find(|p| locate(d, p.level()) == **p)
        .map(|p| p.level())
}

fn sorted_raw(v: &[PatchId]) -> Vec<u64> {
    let mut r: Vec<u64> = v.iter().map(|p| p.raw()).collect();
    r.sort_unstable();
    r
}

fn assert_caps(r: &FrameRecord, cfg: &TerrainConfig, i: usize) {
    let rep = &r.report;
    assert!(
        rep.stream.started <= cfg.stream.max_starts_per_frame,
        "f{i} started"
    );
    assert!(
        rep.stream.completions_applied <= cfg.stream.max_completions_per_frame,
        "f{i} completions"
    );
    assert!(rep.stream.uploads <= cfg.uploads_per_frame, "f{i} uploads");
    assert!(
        rep.stream.uploaded_bytes
            <= u64::from(cfg.uploads_per_frame) * payload_len(cfg.cells_per_edge) as u64,
        "f{i} uploaded_bytes"
    );
    assert!(
        rep.requests_issued <= cfg.max_requests_per_frame,
        "f{i} requests"
    );
    assert!(rep.stream.pending <= cfg.stream.max_pending, "f{i} pending");
    assert!(
        rep.select.nodes_visited <= cfg.lod.max_nodes_visited,
        "f{i} visits {}",
        rep.select.nodes_visited
    );
    assert!(rep.select.leaves <= cfg.lod.max_leaves, "f{i} leaves");
    assert!(r.draws <= MAX_DRAWS_PER_FRAME, "f{i} draws");
    assert!(
        rep.draw_items <= MAX_DRAWS_PER_FRAME - DRAW_MARGIN,
        "f{i} draw items {}",
        rep.draw_items
    );
    assert_eq!(rep.request_errors, 0, "f{i} request errors");
    assert_eq!(rep.upload_rejects, 0, "f{i} upload rejects");
}

#[test]
fn real_defaults_are_used() {
    let cfg = base_settings(Path::Descent).terrain;
    assert_eq!(
        cfg,
        TerrainConfig {
            settle_each_frame: true,
            ..TerrainConfig::default()
        }
    );
    assert_eq!(cfg.lod.max_level, 16);
    assert_eq!(cfg.cells_per_edge, 32);
}

#[test]
fn descent_streams_detail_at_real_defaults() {
    let recs = shared_descent();
    let found = recs
        .iter()
        .enumerate()
        .skip(DESCENT_END_FRAME as usize)
        .find(|(_, r)| sorted_raw(&r.drawn) == sorted_raw(&r.wanted))
        .map(|(i, _)| i);
    let f = found.expect("drawn never equalled wanted after the descent");
    println!(
        "converged at frame {f} ({} frames after descent end, bound {CONVERGE_WITHIN})",
        f - DESCENT_END_FRAME as usize
    );
    assert!(f < (DESCENT_END_FRAME + CONVERGE_WITHIN) as usize);

    let last = &recs[f];
    let lvl = leaf_level_at(&last.drawn, last.look_dir).expect("drawn leaf at look-at point");
    println!("drawn leaf level at look-at point: {lvl}");
    assert_eq!(lvl, 16, "look-at leaf did not reach max_level");

    let at_end = &recs[DESCENT_END_FRAME as usize - 1];
    assert!(
        at_end.altitude <= 3.5,
        "altitude {} at descent end",
        at_end.altitude
    );

    // Wanted level at the look-at point never decreases while descending.
    let mut prev = 0u8;
    for i in (0..DESCENT_END_FRAME as usize).step_by(60) {
        let r = &recs[i];
        let l = leaf_level_at(&r.wanted, r.look_dir).expect("wanted leaf at look-at point");
        assert!(
            l >= prev,
            "wanted level at look-at decreased {prev} -> {l} at f{i}"
        );
        prev = l;
    }
}

#[test]
fn descent_within_counted_caps() {
    let recs = shared_descent();
    let cfg = real_terrain();
    let (mut nodes, mut leaves, mut drawn, mut held, mut pending, mut culled) =
        (0u32, 0usize, 0usize, 0usize, 0usize, 0u32);
    let (mut started, mut uploads, mut items) = (0u32, 0u32, 0usize);
    for (i, r) in recs.iter().enumerate() {
        assert_caps(r, &cfg, i);
        let s = &r.report;
        nodes = nodes.max(s.select.nodes_visited);
        leaves = leaves.max(s.select.leaves);
        drawn = drawn.max(r.drawn.len());
        held = held.max(r.held);
        pending = pending.max(s.stream.pending);
        culled = culled.max(s.culled_draws);
        started = started.max(s.stream.started);
        uploads = uploads.max(s.stream.uploads);
        items = items.max(s.draw_items);
    }
    println!(
        "descent maxima over {} frames: nodes_visited={nodes} leaves={leaves} drawn={drawn} held={held} pending={pending} culled_draws={culled} draw_items={items} started={started} uploads={uploads}",
        recs.len()
    );
}

#[test]
fn descent_crack_free_every_frame() {
    let recs = shared_descent();
    for (i, r) in recs.iter().enumerate().skip(WARM_UP) {
        assert!(!r.drawn.is_empty(), "f{i} empty drawn set");
        assert!(is_partition(&r.drawn), "f{i} not a partition");
        assert!(
            r.report.drawn_max_jump <= 1,
            "f{i} jump {}",
            r.report.drawn_max_jump
        );
        assert!(max_level_jump(&r.drawn) <= 1, "f{i} balance");
    }
}

#[test]
fn ascent_aggregates() {
    let recs = shared_ascent();
    let cfg = real_terrain();
    for (i, r) in recs.iter().enumerate() {
        assert_caps(r, &cfg, i);
        if i >= WARM_UP {
            assert!(is_partition(&r.drawn), "f{i} not a partition");
            assert!(max_level_jump(&r.drawn) <= 1, "f{i} balance");
        }
    }
    let last = recs.last().expect("frames");
    let max_wanted = last.wanted.iter().map(|p| p.level()).max().unwrap_or(0);
    let max_drawn = last.drawn.iter().map(|p| p.level()).max().unwrap_or(0);
    println!(
        "ascent end: altitude={:.0} max wanted level={max_wanted} max drawn level={max_drawn} wanted={} drawn={}",
        last.altitude,
        last.wanted.len(),
        last.drawn.len()
    );
    assert!(max_wanted <= 6, "max wanted level {max_wanted} at orbit");
    assert_eq!(sorted_raw(&last.drawn), sorted_raw(&last.wanted));
}

#[test]
fn descent_is_deterministic() {
    let key = |recs: &[FrameRecord]| -> Vec<(u32, u32, u32, Vec<u64>)> {
        recs.iter()
            .map(|r| {
                (
                    r.report.stream.started,
                    r.report.stream.completions_applied,
                    r.report.stream.uploads,
                    r.drawn.iter().map(|p| p.raw()).collect(),
                )
            })
            .collect()
    };
    let a = key(shared_descent());
    let b = key(&run(|_| {}, FRAMES));
    assert_eq!(a, b);
}

#[test]
fn descent_degrades_with_one_start_per_frame() {
    let recs = run(
        |s| {
            s.terrain.stream.max_starts_per_frame = 1;
            s.terrain.stream.max_completions_per_frame = 1;
        },
        FRAMES,
    );
    for (i, r) in recs.iter().enumerate() {
        assert!(r.report.stream.started <= 1, "f{i} started");
        if i >= WARM_UP {
            assert!(!r.drawn.is_empty(), "f{i} empty drawn set");
            assert!(is_partition(&r.drawn), "f{i} not a partition");
            assert!(max_level_jump(&r.drawn) <= 1, "f{i} balance");
        }
    }
}

#[test]
fn descent_no_gpu_leak() {
    let base_patches = 6 * ((1usize << (2 * (3 + 1))) - 1) / 3 + 16;
    let cfg = real_terrain();
    for (i, r) in shared_descent().iter().enumerate() {
        assert_eq!(r.buffer_count, 2 * r.held, "f{i} buffers");
        assert!(
            r.held <= base_patches + cfg.detail_budget_patches as usize,
            "f{i} held {}",
            r.held
        );
    }
    let recs = run(|s| s.terrain.detail_budget_patches = 60, FRAMES);
    let mut evicted = 0u64;
    for (i, r) in recs.iter().enumerate() {
        assert_eq!(r.buffer_count, 2 * r.held, "f{i} buffers (small budget)");
        assert!(r.held <= base_patches + 60, "f{i} held {}", r.held);
        evicted += r.report.stream.evicted as u64;
    }
    println!("small-budget run: total evicted = {evicted}");
    assert!(evicted > 0, "forced eviction never happened");
}

#[test]
fn production_path_never_blocks() {
    let recs = run(|s| s.terrain.settle_each_frame = false, FRAMES);
    assert_eq!(recs.len(), FRAMES as usize);
    let cfg = TerrainConfig::default();
    for (i, r) in recs.iter().enumerate() {
        assert_caps(r, &cfg, i);
        assert!(r.report.drawn_max_jump <= 1, "f{i} jump");
        let mut faces: Vec<u8> = r.drawn.iter().map(|p| p.face().index()).collect();
        faces.sort_unstable();
        faces.dedup();
        if faces.len() == Face::ALL.len() {
            assert!(is_partition(&r.drawn), "f{i} not a partition");
            assert!(max_level_jump(&r.drawn) <= 1, "f{i} balance");
        }
    }
    let last = recs.last().expect("frames");
    assert!(last.report.drawn_leaves > 0 || last.report.stream.pending > 0);
}
