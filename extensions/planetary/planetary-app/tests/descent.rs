//! PLAN-07: scripted orbit-to-surface descent on a simulated clock (no wall-clock time).
//!
//! Every frame is driven with a constant `dt = 1/60`; all assertions are counted work, never
//! durations.

use engine_sdk::streaming::StreamConfig;
use engine_sdk::{Engine, EngineConfig, MAX_DRAWS_PER_FRAME, NullBackend, Runtime};
use planetary_app::{AppSettings, DescentScript, PlanetaryApp};
use planetary_core::lod::{LodParams, is_partition, max_level_jump};
use planetary_core::payload_len;
use planetary_core::{Face, PatchId};
use planetary_terrain::{TerrainConfig, TerrainReport};

/// Frames of a standard run: 8 s descent + 2 s hover at 60 Hz.
const FRAMES: u32 = 600;
/// Frame at which the scripted descent reaches the surface (8 s).
const DESCENT_END_FRAME: u32 = 480;
/// Frames ignored before the first drawn set must exist.
const WARM_UP: usize = 10;
/// Cap on LOD selection node visits per frame.
const NODE_VISIT_CAP: u32 = 50_000;
/// Mesh uploads per frame (explicit, not inherited from the default config).
const UPLOADS_PER_FRAME: u32 = 4;
/// New patch requests per frame (explicit, not inherited from the default config).
const REQUESTS_PER_FRAME: u32 = 16;
/// Frames after the descent ends within which drawn must equal wanted (drain at 4 uploads/frame).
const CONVERGE_WITHIN: u32 = 240;
/// Deepest LOD level used by the test (see SUMMARY: relief-inflated bounds force every patch
/// within ~max_relief of the camera to max level, so level 16 yields >3000 leaves, above max_leaves).
const LOD_MAX_LEVEL: u8 = 13;
/// Cells per patch edge used by the test.
const CELLS: u32 = 16;

struct FrameRecord {
    report: TerrainReport,
    drawn: Vec<PatchId>,
    wanted: Vec<PatchId>,
    draws: usize,
    buffer_count: usize,
    held: usize,
    altitude: f64,
}

fn base_settings(radius_script: bool) -> AppSettings {
    let mut s = AppSettings {
        terrain: TerrainConfig {
            cells_per_edge: CELLS,
            lod: LodParams {
                max_level: LOD_MAX_LEVEL,
                ..Default::default()
            },
            settle_each_frame: true,
            workers: 2,
            uploads_per_frame: UPLOADS_PER_FRAME,
            max_requests_per_frame: REQUESTS_PER_FRAME,
            stream: StreamConfig {
                max_pending: 256,
                max_starts_per_frame: 8,
                max_completions_per_frame: 8,
            },
            ..Default::default()
        },
        ..Default::default()
    };
    if radius_script {
        s.script = Some(DescentScript::orbit_to_surface(s.terrain.terrain.radius));
    }
    s
}

fn run(cfg_mut: impl Fn(&mut AppSettings), frames: u32) -> Vec<FrameRecord> {
    let mut settings = base_settings(true);
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
        out.push(FrameRecord {
            report: app.report().clone(),
            drawn: app.terrain().drawn().to_vec(),
            wanted: app.terrain().wanted_leaves().to_vec(),
            draws,
            buffer_count: rt.backend().buffer_count(),
            held: app.terrain().held_mesh_count(),
            altitude,
        });
    }
    app.shutdown();
    out
}

fn converge_frames() -> u32 {
    FRAMES.max(DESCENT_END_FRAME + CONVERGE_WITHIN)
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
        rep.stream.uploaded_bytes <= u64::from(cfg.uploads_per_frame) * payload_len(CELLS) as u64,
        "f{i} uploaded_bytes"
    );
    assert!(
        rep.requests_issued <= cfg.max_requests_per_frame,
        "f{i} requests"
    );
    assert!(rep.stream.pending <= cfg.stream.max_pending, "f{i} pending");
    assert!(rep.select.nodes_visited <= NODE_VISIT_CAP, "f{i} visits");
    assert!(rep.select.leaves <= cfg.lod.max_leaves, "f{i} leaves");
    assert!(r.draws <= MAX_DRAWS_PER_FRAME, "f{i} draws");
    assert!(rep.draw_items <= MAX_DRAWS_PER_FRAME, "f{i} draw items");
    assert_eq!(rep.request_errors, 0, "f{i} request errors");
    assert_eq!(rep.upload_rejects, 0, "f{i} upload rejects");
}

fn std_cfg() -> TerrainConfig {
    base_settings(true).terrain
}

#[test]
fn descent_within_counted_caps() {
    let recs = run(|_| {}, FRAMES);
    let cfg = std_cfg();
    let mut m = (0u32, 0u32, 0u64, 0u32, 0usize, 0usize, 0.0f64);
    for (i, r) in recs.iter().enumerate() {
        assert_caps(r, &cfg, i);
        let s = &r.report;
        m.0 = m.0.max(s.stream.started);
        m.1 = m.1.max(s.stream.uploads);
        m.2 = m.2.max(s.stream.uploaded_bytes);
        m.3 = m.3.max(s.select.nodes_visited);
        m.4 = m.4.max(r.drawn.len());
        m.5 = m.5.max(r.held);
        m.6 = m.6.max(s.select.tau_eff);
    }
    eprintln!(
        "descent maxima over {} frames: started={} uploads={} uploaded_bytes={} nodes_visited={} drawn={} held={} tau_eff={:.3}",
        recs.len(),
        m.0,
        m.1,
        m.2,
        m.3,
        m.4,
        m.5,
        m.6
    );
}

#[test]
fn descent_crack_free_every_frame() {
    let recs = run(|_| {}, FRAMES);
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
fn descent_reaches_surface_and_converges() {
    let total = converge_frames();
    let recs = run(|_| {}, total);
    let at_end = &recs[DESCENT_END_FRAME as usize - 1];
    assert!(
        at_end.altitude <= 3.5,
        "altitude {} at descent end",
        at_end.altitude
    );
    let found = recs
        .iter()
        .enumerate()
        .skip(DESCENT_END_FRAME as usize)
        .find(|(_, r)| {
            let mut d: Vec<u64> = r.drawn.iter().map(|p| p.raw()).collect();
            let mut w: Vec<u64> = r.wanted.iter().map(|p| p.raw()).collect();
            d.sort_unstable();
            w.sort_unstable();
            d == w
        })
        .map(|(i, _)| i);
    let f = found.expect("drawn never equalled wanted after the descent");
    eprintln!(
        "converged at frame {f} ({} frames after descent end, bound {CONVERGE_WITHIN})",
        f - DESCENT_END_FRAME as usize
    );
    assert!(f < (DESCENT_END_FRAME + CONVERGE_WITHIN) as usize);
}

#[test]
fn descent_is_deterministic() {
    let key = |recs: Vec<FrameRecord>| -> Vec<(u32, u32, u32, Vec<u64>)> {
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
    let a = key(run(|_| {}, FRAMES));
    let b = key(run(|_| {}, FRAMES));
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
    let cfg = std_cfg();
    let recs = run(|_| {}, FRAMES);
    for (i, r) in recs.iter().enumerate() {
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
    eprintln!("small-budget run: total evicted = {evicted}");
    assert!(evicted > 0, "forced eviction never happened");
}

#[test]
fn production_path_never_blocks() {
    let recs = run(|s| s.terrain.settle_each_frame = false, FRAMES);
    assert_eq!(recs.len(), FRAMES as usize);
    let cfg = {
        let mut c = std_cfg();
        c.settle_each_frame = false;
        c
    };
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
