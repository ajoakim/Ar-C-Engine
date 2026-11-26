//! 07.1-19: windowed-realistic streaming lag at the REAL default terrain parameters.
//!
//! Async workers (`settle_each_frame: false`), a simulated 1/30 s step per frame and no wall
//! clock. After every simulated frame the harness waits for the jobs started so far to finish
//! (`wait_for_workers`, no sleep and no duration): workers finish within a 30 fps frame, as they
//! do in a release build (the paced `--bench-stream` run shows no worker backlog), and their
//! completions land in the next frame through the production polling path. That removes OS
//! scheduling and debug-build speed from the result, so the counts are reproducible. All
//! assertions are counted work.

use std::sync::OnceLock;

use engine_sdk::MAX_DRAWS_PER_FRAME;
use planetary_app::stream_bench::{StreamMetrics, StreamRun, analyze, report_text, run_stream};
use planetary_app::{AppSettings, DescentScript, SimClock};
use planetary_terrain::{DRAW_MARGIN, TerrainConfig};

/// Simulated frame time (windowed present pacing is about 31 ms).
const DT: f64 = 1.0 / 30.0;
/// Hover frames after the descent ends (counted).
const SETTLE_FRAMES: u64 = 300;
/// Simulated telemetry clock step: each frame measures about 0.2 ms of terrain work, so the
/// adaptive budget runs at its hard maxima deterministically (slow-frame throttling is covered
/// by the planetary-terrain budget tests).
const SIM_CLOCK_STEP_NS: u64 = 100_000;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flight {
    Ordinary,
    Fast,
}

fn execute(flight: Flight) -> (StreamRun, StreamMetrics) {
    let mut settings = AppSettings {
        terrain: TerrainConfig::default(),
        // Production configuration: caps adapt to the (simulated, cheap) measured frame cost.
        adaptive_streaming: true,
        ..Default::default()
    };
    let r = settings.terrain.terrain.radius;
    let script = match flight {
        Flight::Ordinary => DescentScript::orbit_to_surface(r),
        Flight::Fast => DescentScript::fast_free_flight(r),
    };
    settings.script = Some(script);
    let frames = (script.descent_seconds / DT).ceil() as u64 + SETTLE_FRAMES;
    let mut run = run_stream(
        settings,
        Box::new(SimClock::new(SIM_CLOCK_STEP_NS)),
        frames,
        DT,
        &mut |app| app.wait_for_workers(),
    )
    .expect("stream run");
    run.descent_frames = (script.descent_seconds / DT).ceil() as usize;
    let m = analyze(&run);
    (run, m)
}

fn shared(flight: Flight) -> &'static (StreamRun, StreamMetrics) {
    static FAST: OnceLock<(StreamRun, StreamMetrics)> = OnceLock::new();
    static ORD: OnceLock<(StreamRun, StreamMetrics)> = OnceLock::new();
    match flight {
        Flight::Fast => FAST.get_or_init(|| execute(Flight::Fast)),
        Flight::Ordinary => ORD.get_or_init(|| execute(Flight::Ordinary)),
    }
}

fn assert_invariants(run: &StreamRun, m: &StreamMetrics) {
    let cfg = TerrainConfig::default();
    let hard = cfg.budget;
    assert!(
        m.partition_every_frame,
        "drawn was not a partition after the first complete cover"
    );
    for (i, r) in run.frames.iter().enumerate() {
        assert!(r.started <= hard.max_starts, "f{i} started {}", r.started);
        assert!(r.uploads <= hard.max_uploads, "f{i} uploads {}", r.uploads);
        assert!(
            r.requests_issued <= hard.max_requests,
            "f{i} requests {}",
            r.requests_issued
        );
        assert!(r.draws <= MAX_DRAWS_PER_FRAME, "f{i} draws");
        assert!(
            r.draw_items <= MAX_DRAWS_PER_FRAME - DRAW_MARGIN,
            "f{i} draw items"
        );
        assert!(r.wanted_leaves <= cfg.lod.max_leaves, "f{i} leaves");
    }
}

/// Bounds fixed from the AFTER measurement (debug build, `wait_for_workers` pacing, identical on
/// repeated runs) plus about 25 percent for ordering differences between worker completions.
/// Measured (fast / ordinary): max lag 8 / 6, p95 lag 7 / 5, frames-to-detail 52 / 129, frames
/// from arrival to convergence 16 / 12, frames with lag > 1 level 16% / 25%. The release paced
/// run before the fix measured (fast / ordinary): lag max 10 / 9, p95 10 / 8, frames-to-detail
/// 198 / 224, convergence 169 / 133, lag > 1 on 60% / 63% of frames; every bound below is
/// tighter than that.
struct Bounds {
    max_lag: i32,
    p95_lag: i32,
    frames_to_detail: u64,
    frames_to_converge: usize,
    frac_lag_over_1: f64,
}

const FAST: Bounds = Bounds {
    max_lag: 10,
    p95_lag: 9,
    frames_to_detail: 65,
    frames_to_converge: 20,
    frac_lag_over_1: 0.21,
};

const ORDINARY: Bounds = Bounds {
    max_lag: 8,
    p95_lag: 7,
    frames_to_detail: 162,
    frames_to_converge: 15,
    frac_lag_over_1: 0.32,
};

fn assert_bounds(m: &StreamMetrics, b: &Bounds) {
    assert!(
        m.max_lag <= b.max_lag,
        "max lag {} > {}",
        m.max_lag,
        b.max_lag
    );
    assert!(
        m.p95_lag <= b.p95_lag,
        "p95 lag {} > {}",
        m.p95_lag,
        b.p95_lag
    );
    assert!(
        m.max_frames_to_detail <= b.frames_to_detail,
        "frames to detail {} > {}",
        m.max_frames_to_detail,
        b.frames_to_detail
    );
    let conv = m.frames_to_converge.expect("drawn never equalled wanted");
    assert!(
        conv <= b.frames_to_converge,
        "frames to converge {conv} > {}",
        b.frames_to_converge
    );
    assert!(
        m.frac_lag_over_1 <= b.frac_lag_over_1,
        "lag > 1 level on {:.1}% of frames (bound {:.1}%)",
        m.frac_lag_over_1 * 100.0,
        b.frac_lag_over_1 * 100.0
    );
    // The detail at the look-at point reaches the maximum level after the settle period.
    assert_eq!(
        m.final_look_drawn_level, 16,
        "look-at leaf not at max level"
    );
}

#[test]
fn fast_descent_lag_is_bounded() {
    let (run, m) = shared(Flight::Fast);
    println!("{}", report_text("fast free-flight, async", m));
    assert!(run.frames.len() as u64 > SETTLE_FRAMES);
    assert_invariants(run, m);
    assert_bounds(m, &FAST);
}

#[test]
fn ordinary_descent_async_lag_is_bounded() {
    let (run, m) = shared(Flight::Ordinary);
    println!("{}", report_text("scripted descent, async", m));
    assert_invariants(run, m);
    assert_bounds(m, &ORDINARY);
}

#[test]
fn adaptive_budget_respects_hard_max() {
    let hard = TerrainConfig::default().budget;
    let base = TerrainConfig::default();
    for flight in [Flight::Fast, Flight::Ordinary] {
        let (run, m) = shared(flight);
        let (mut up, mut st) = (0u32, 0u32);
        for (i, r) in run.frames.iter().enumerate() {
            assert!(r.uploads <= hard.max_uploads, "f{i} uploads");
            assert!(r.started <= hard.max_starts, "f{i} started");
            assert!(r.requests_issued <= hard.max_requests, "f{i} requests");
            assert!(r.caps.0 <= hard.max_uploads && r.caps.1 <= hard.max_starts);
            assert!(
                r.uploads <= r.caps.0.max(base.uploads_per_frame),
                "f{i} over cap"
            );
            up = up.max(r.uploads);
            st = st.max(r.started);
        }
        // Fast measured frames must actually raise the rates above the fixed base caps.
        assert!(
            up > base.uploads_per_frame,
            "uploads never exceeded the base ({up})"
        );
        assert!(
            st > base.stream.max_starts_per_frame,
            "starts never exceeded the base ({st})"
        );
        assert!(m.partition_every_frame);
    }
}

#[test]
fn small_detail_budget_keeps_partition_under_async_workers() {
    let mut settings = AppSettings {
        terrain: TerrainConfig {
            detail_budget_patches: 120,
            ..TerrainConfig::default()
        },
        adaptive_streaming: true,
        ..Default::default()
    };
    let script = DescentScript::fast_free_flight(settings.terrain.terrain.radius);
    settings.script = Some(script);
    let frames = (script.descent_seconds / DT).ceil() as u64 + 150;
    let run = run_stream(
        settings,
        Box::new(SimClock::new(SIM_CLOCK_STEP_NS)),
        frames,
        DT,
        &mut |app| app.wait_for_workers(),
    )
    .expect("stream run");
    let m = analyze(&run);
    println!(
        "{}",
        report_text("fast free-flight, async, detail budget 120", &m)
    );
    assert!(m.total_evicted > 0, "eviction never happened");
    assert!(
        m.partition_every_frame,
        "a hole appeared while the fallback chain was under eviction pressure"
    );
    assert!(m.max_draws <= MAX_DRAWS_PER_FRAME);
}
