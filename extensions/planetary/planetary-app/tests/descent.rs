//! PLAN-07 / HEX-03: scripted orbit-to-surface descent and surface-to-orbit ascent at the REAL
//! default parameters, for BOTH topologies, on a simulated clock (no wall-clock time).
//!
//! Hex (`AppSettings::default()`: N = 16, max_level 16, 8192 node visits, 900 pieces) is the
//! subject; the cube-sphere (`AppSettings::cube_default()`, N = 32) is the oracle and keeps the
//! original test names. Every frame is driven with a constant `dt = 1/60`; all assertions are
//! counted work, never durations.

use std::sync::OnceLock;

use engine_sdk::{DVec3, Engine, EngineConfig, MAX_DRAWS_PER_FRAME, NullBackend, Runtime};
use planetary_app::{AppSettings, DescentScript, PlanetaryApp, TopologyKind};
use planetary_core::hex::mesh::{vertex_count_hex, vertex_count_pentagon};
use planetary_terrain::{DRAW_MARGIN, TerrainConfig, TerrainReport, cell_payload_estimate};

/// Frames of a standard run: 8 s scripted path + 4 s hover at 60 Hz.
const FRAMES: u32 = 720;
/// Frame at which the scripted path ends (8 s).
const DESCENT_END_FRAME: u32 = 480;
/// Frames ignored before the first drawn set must exist.
const WARM_UP: usize = 10;
/// Frames after the descent ends within which drawn must equal wanted.
const CONVERGE_WITHIN: u32 = 240;

use TopologyKind::{Cube, Hex};

/// Vertices of a cube patch: the (n+1)^2 grid plus four skirt rows of n+1.
fn cube_vertex_count(n: u32) -> u32 {
    (n + 1) * (n + 1) + 4 * (n + 1)
}

/// Largest vertex count of one cell of the topology.
fn max_vertex_count(kind: TopologyKind, n: u32) -> u32 {
    match kind {
        Cube => cube_vertex_count(n),
        Hex => vertex_count_hex(n).max(vertex_count_pentagon(n)),
    }
}

/// Vertex classes (shared index buffers) of the topology.
fn vertex_classes(kind: TopologyKind) -> usize {
    match kind {
        Cube => 1,
        Hex => 2,
    }
}

struct FrameRecord {
    report: TerrainReport,
    drawn: Vec<u64>,
    wanted: Vec<u64>,
    drawn_levels: Vec<u8>,
    wanted_levels: Vec<u8>,
    partition: bool,
    cut_closed: bool,
    draws: usize,
    buffer_count: usize,
    held: usize,
    altitude: f64,
    /// Level of the wanted / drawn cell at the point 10 m ahead of the eye.
    look_wanted: Option<u8>,
    look_drawn: Option<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Path {
    Descent,
    Ascent,
}

fn settings_for(kind: TopologyKind) -> AppSettings {
    match kind {
        Hex => AppSettings::default(),
        Cube => AppSettings::cube_default(),
    }
}

fn real_terrain(kind: TopologyKind) -> TerrainConfig {
    TerrainConfig {
        settle_each_frame: true,
        ..settings_for(kind).terrain
    }
}

fn base_settings(kind: TopologyKind, path: Path) -> AppSettings {
    let mut s = AppSettings {
        terrain: real_terrain(kind),
        ..settings_for(kind)
    };
    let r = s.terrain.terrain.radius;
    s.script = Some(match path {
        Path::Descent => DescentScript::orbit_to_surface(r),
        Path::Ascent => DescentScript::surface_to_orbit(r),
    });
    s
}

fn sorted(mut v: Vec<u64>) -> Vec<u64> {
    v.sort_unstable();
    v
}

fn run_path(
    kind: TopologyKind,
    path: Path,
    cfg_mut: impl Fn(&mut AppSettings),
    frames: u32,
) -> Vec<FrameRecord> {
    let mut settings = base_settings(kind, path);
    cfg_mut(&mut settings);
    let mut app = PlanetaryApp::new(settings).expect("app");
    assert_eq!(app.topology_kind(), kind);
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
        let altitude = app.camera().altitude(&|d| app.surface_radius_at(d));
        let ahead: DVec3 = (app.camera().position() + app.camera().forward() * 10.0).normalize();
        out.push(FrameRecord {
            report: app.report().clone(),
            drawn: sorted(app.drawn_raw()),
            wanted: sorted(app.wanted_raw()),
            drawn_levels: app.drawn_levels(),
            wanted_levels: app.wanted_levels(),
            partition: app.drawn_is_partition(),
            cut_closed: app.cut_closed(),
            draws,
            buffer_count: rt.backend().buffer_count(),
            held: app.held_mesh_count(),
            altitude,
            look_wanted: app.wanted_level_at(ahead),
            look_drawn: app.piece_level_at(ahead),
        });
    }
    app.shutdown();
    out
}

fn run(kind: TopologyKind, cfg_mut: impl Fn(&mut AppSettings), frames: u32) -> Vec<FrameRecord> {
    run_path(kind, Path::Descent, cfg_mut, frames)
}

/// The default descent / ascent runs are deterministic in settle mode, so tests share one run
/// per topology and path.
fn shared(kind: TopologyKind, path: Path) -> &'static [FrameRecord] {
    static HEX_DESCENT: OnceLock<Vec<FrameRecord>> = OnceLock::new();
    static HEX_ASCENT: OnceLock<Vec<FrameRecord>> = OnceLock::new();
    static CUBE_DESCENT: OnceLock<Vec<FrameRecord>> = OnceLock::new();
    static CUBE_ASCENT: OnceLock<Vec<FrameRecord>> = OnceLock::new();
    let cell = match (kind, path) {
        (Hex, Path::Descent) => &HEX_DESCENT,
        (Hex, Path::Ascent) => &HEX_ASCENT,
        (Cube, Path::Descent) => &CUBE_DESCENT,
        (Cube, Path::Ascent) => &CUBE_ASCENT,
    };
    cell.get_or_init(|| run_path(kind, path, |_| {}, FRAMES))
}

fn assert_caps(kind: TopologyKind, r: &FrameRecord, cfg: &TerrainConfig, i: usize) {
    let rep = &r.report;
    assert!(
        rep.stream.started <= cfg.stream.max_starts_per_frame,
        "f{i} started"
    );
    assert!(
        rep.stream.completions_applied <= cfg.stream.max_completions_per_frame,
        "f{i} completions"
    );
    assert!(
        rep.stream.uploads <= cfg.uploads_per_frame,
        "f{i} uploads {} bytes {}",
        rep.stream.uploads,
        rep.stream.uploaded_bytes
    );
    assert!(
        rep.stream.uploaded_bytes
            <= u64::from(cfg.uploads_per_frame)
                * cell_payload_estimate(max_vertex_count(kind, cfg.cells_per_edge)),
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

fn real_defaults_are_used_for(kind: TopologyKind) {
    let s = base_settings(kind, Path::Descent);
    assert_eq!(s.topology, kind);
    let cfg = s.terrain;
    let defaults = TerrainConfig::default();
    let expected_n = match kind {
        Hex => 16,
        Cube => 32,
    };
    assert_eq!(
        cfg,
        TerrainConfig {
            settle_each_frame: true,
            cells_per_edge: expected_n,
            ..defaults.clone()
        }
    );
    assert_eq!(cfg.lod.max_level, 16);
    assert_eq!(cfg.lod.max_nodes_visited, 8192);
    assert_eq!(cfg.lod.max_leaves, 900);
    assert_eq!(cfg.cells_per_edge, expected_n);
    if kind == Hex {
        assert_eq!(AppSettings::default().topology, Hex);
    }
}

fn descent_streams_detail(kind: TopologyKind) {
    let recs = shared(kind, Path::Descent);
    let found = recs
        .iter()
        .enumerate()
        .skip(DESCENT_END_FRAME as usize)
        .find(|(_, r)| r.drawn == r.wanted)
        .map(|(i, _)| i);
    let f = found.expect("drawn never equalled wanted after the descent");
    println!(
        "{} converged at frame {f} ({} frames after descent end, bound {CONVERGE_WITHIN})",
        kind.name(),
        f - DESCENT_END_FRAME as usize
    );
    assert!(f < (DESCENT_END_FRAME + CONVERGE_WITHIN) as usize);

    let last = &recs[f];
    let lvl = last.look_drawn.expect("drawn piece at look-at point");
    println!("{} drawn level at look-at point: {lvl}", kind.name());
    assert_eq!(lvl, 16, "look-at piece did not reach max_level");

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
        let l = r.look_wanted.expect("wanted leaf at look-at point");
        assert!(
            l >= prev,
            "wanted level at look-at decreased {prev} -> {l} at f{i}"
        );
        prev = l;
    }
}

fn morph_first_frame_defects_are_zero(kind: TopologyKind) {
    for path in [Path::Descent, Path::Ascent] {
        let recs = shared(kind, path);
        assert!(
            recs.iter().all(|r| r.report.morph_first_frame_defects == 0),
            "{} refine-appearing piece not shown at morph 1",
            kind.name()
        );
        assert!(
            recs.iter()
                .all(|r| (0.0..=1.0).contains(&r.report.morph_min)
                    && (0.0..=1.0).contains(&r.report.handover_gap_max))
        );
    }
}

fn within_counted_caps(kind: TopologyKind) {
    let recs = shared(kind, Path::Descent);
    let cfg = real_terrain(kind);
    let (mut nodes, mut leaves, mut drawn, mut held, mut pending, mut culled) =
        (0u32, 0usize, 0usize, 0usize, 0usize, 0u32);
    let (mut started, mut uploads, mut items) = (0u32, 0u32, 0usize);
    for (i, r) in recs.iter().enumerate() {
        assert_caps(kind, r, &cfg, i);
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
        "{} descent maxima over {} frames: nodes_visited={nodes} leaves={leaves} drawn={drawn} held={held} pending={pending} culled_draws={culled} draw_items={items} started={started} uploads={uploads}",
        kind.name(),
        recs.len()
    );
}

fn cut_closed_every_frame(kind: TopologyKind) {
    let recs = shared(kind, Path::Descent);
    for (i, r) in recs.iter().enumerate().skip(WARM_UP) {
        assert!(!r.drawn.is_empty(), "f{i} empty drawn set");
        assert!(r.partition, "f{i} not a partition");
        assert!(r.cut_closed, "f{i} cut not closed");
        assert!(
            r.report.drawn_max_jump <= 1,
            "f{i} jump {}",
            r.report.drawn_max_jump
        );
    }
}

fn ascent_aggregates_for(kind: TopologyKind) {
    let recs = shared(kind, Path::Ascent);
    let cfg = real_terrain(kind);
    for (i, r) in recs.iter().enumerate() {
        assert_caps(kind, r, &cfg, i);
        if i >= WARM_UP {
            assert!(r.partition, "f{i} not a partition");
            assert!(r.report.drawn_max_jump <= 1, "f{i} balance");
        }
    }
    let last = recs.last().expect("frames");
    let max_wanted = last.wanted_levels.iter().copied().max().unwrap_or(0);
    let max_drawn = last.drawn_levels.iter().copied().max().unwrap_or(0);
    println!(
        "{} ascent end: altitude={:.0} max wanted level={max_wanted} max drawn level={max_drawn} wanted={} drawn={}",
        kind.name(),
        last.altitude,
        last.wanted.len(),
        last.drawn.len()
    );
    assert!(max_wanted <= 6, "max wanted level {max_wanted} at orbit");
    assert_eq!(last.drawn, last.wanted);
}

fn is_deterministic(kind: TopologyKind) {
    let key = |recs: &[FrameRecord]| -> Vec<(u32, u32, u32, Vec<u64>)> {
        recs.iter()
            .map(|r| {
                (
                    r.report.stream.started,
                    r.report.stream.completions_applied,
                    r.report.stream.uploads,
                    r.drawn.clone(),
                )
            })
            .collect()
    };
    let a = key(shared(kind, Path::Descent));
    let b = key(&run(kind, |_| {}, FRAMES));
    assert_eq!(a, b);
}

fn degrades_with_one_start_per_frame_for(kind: TopologyKind) {
    let recs = run(
        kind,
        |s| {
            s.terrain.stream.max_starts_per_frame = 1;
            s.terrain.stream.max_completions_per_frame = 1;
        },
        FRAMES,
    );
    // With one start per frame every root (cube faces, hex roots) needs its own frame
    // before the first complete cover can exist; roots are requested first, so the cover must
    // exist within a few frames of the root count (measured: cube 5, hex 11), and from then on the cut never has a hole.
    let cover_bound = match kind {
        Cube => WARM_UP,
        Hex => 16,
    };
    let mut covered_at: Option<usize> = None;
    for (i, r) in recs.iter().enumerate() {
        assert!(r.report.stream.started <= 1, "f{i} started");
        if covered_at.is_none() && !r.drawn.is_empty() && r.partition {
            covered_at = Some(i);
        }
        if covered_at.is_some() {
            assert!(!r.drawn.is_empty(), "f{i} empty drawn set");
            assert!(r.partition, "f{i} not a partition");
            assert!(r.report.drawn_max_jump <= 1, "f{i} balance");
        }
    }
    let at = covered_at.expect("a complete cover was never drawn");
    println!("{} first complete cover at frame {at}", kind.name());
    assert!(
        at <= cover_bound,
        "first cover at f{at}, bound {cover_bound}"
    );
}

fn no_gpu_leak(kind: TopologyKind) {
    let classes = vertex_classes(kind);
    let base_patches = match kind {
        Cube => 6 * ((1usize << (2 * (3 + 1))) - 1) / 3 + 16,
        // Hex base levels hold fewer cells than the cube's; the cube bound is a safe upper bound.
        Hex => 6 * ((1usize << (2 * (3 + 1))) - 1) / 3 + 16,
    };
    let cfg = real_terrain(kind);
    for (i, r) in shared(kind, Path::Descent).iter().enumerate() {
        // One vertex buffer per held cell plus one shared index buffer per vertex class.
        assert_eq!(r.buffer_count, r.held + classes, "f{i} buffers");
        assert!(
            r.held <= base_patches + cfg.detail_budget_patches as usize,
            "f{i} held {}",
            r.held
        );
    }
    let recs = run(kind, |s| s.terrain.detail_budget_patches = 60, FRAMES);
    let mut evicted = 0u64;
    for (i, r) in recs.iter().enumerate() {
        assert_eq!(
            r.buffer_count,
            r.held + classes,
            "f{i} buffers (small budget)"
        );
        assert!(r.held <= base_patches + 60, "f{i} held {}", r.held);
        evicted += r.report.stream.evicted as u64;
    }
    println!(
        "{} small-budget run: total evicted = {evicted}",
        kind.name()
    );
    assert!(evicted > 0, "forced eviction never happened");
}

fn production_path_never_blocks_for(kind: TopologyKind) {
    let recs = run(kind, |s| s.terrain.settle_each_frame = false, FRAMES);
    assert_eq!(recs.len(), FRAMES as usize);
    let cfg = settings_for(kind).terrain;
    let mut covered = false;
    for (i, r) in recs.iter().enumerate() {
        assert_caps(kind, r, &cfg, i);
        assert!(r.report.drawn_max_jump <= 1, "f{i} jump");
        // Once the whole planet is covered the cut stays an exact closed partition.
        covered |= r.cut_closed && r.partition;
        if covered {
            assert!(r.cut_closed && r.partition, "f{i} cover lost");
        }
    }
    let last = recs.last().expect("frames");
    assert!(last.report.drawn_leaves > 0 || last.report.stream.pending > 0);
}

// Cube oracle: original names.
#[test]
fn real_defaults_are_used() {
    real_defaults_are_used_for(Cube);
}
#[test]
fn descent_streams_detail_at_real_defaults() {
    descent_streams_detail(Cube);
}
#[test]
fn descent_within_counted_caps() {
    within_counted_caps(Cube);
}
#[test]
fn descent_crack_free_every_frame() {
    cut_closed_every_frame(Cube);
}
#[test]
fn ascent_aggregates() {
    ascent_aggregates_for(Cube);
}
#[test]
fn descent_is_deterministic() {
    is_deterministic(Cube);
}
#[test]
fn descent_degrades_with_one_start_per_frame() {
    degrades_with_one_start_per_frame_for(Cube);
}
#[test]
fn descent_no_gpu_leak() {
    no_gpu_leak(Cube);
}
#[test]
fn production_path_never_blocks() {
    production_path_never_blocks_for(Cube);
}

// Hex (default topology).
#[test]
fn morph_first_frame_defects_are_zero_cube() {
    morph_first_frame_defects_are_zero(Cube);
}

#[test]
fn morph_first_frame_defects_are_zero_hex() {
    morph_first_frame_defects_are_zero(Hex);
}

#[test]
fn hex_real_defaults_are_used() {
    real_defaults_are_used_for(Hex);
}
#[test]
fn hex_descent_streams_detail_at_real_defaults() {
    descent_streams_detail(Hex);
}
#[test]
fn hex_within_counted_caps() {
    within_counted_caps(Hex);
}
#[test]
fn hex_cut_closed_every_frame() {
    cut_closed_every_frame(Hex);
}
#[test]
fn hex_ascent_aggregates() {
    ascent_aggregates_for(Hex);
}
#[test]
fn hex_is_deterministic() {
    is_deterministic(Hex);
}
#[test]
fn hex_descent_degrades_with_one_start_per_frame() {
    degrades_with_one_start_per_frame_for(Hex);
}
#[test]
fn hex_no_gpu_leak() {
    no_gpu_leak(Hex);
}
#[test]
fn hex_production_path_never_blocks() {
    production_path_never_blocks_for(Hex);
}
