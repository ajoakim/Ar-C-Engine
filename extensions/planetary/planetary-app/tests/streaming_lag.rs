//! 07.1-19: windowed-realistic streaming lag at the REAL default terrain parameters.
//!
//! Async workers (`settle_each_frame: false`), a simulated 1/30 s step per frame and no wall
//! clock. After every frame a COUNTED worker pump (a fixed number of `yield_now` calls, never a
//! duration) gives the worker threads a slice of CPU. All assertions are counted work.

use std::sync::OnceLock;

use engine_sdk::MAX_DRAWS_PER_FRAME;
use planetary_app::stream_bench::{StreamMetrics, StreamRun, analyze, report_text, run_stream};
use planetary_app::{AppSettings, DescentScript, SimClock};
use planetary_terrain::{DRAW_MARGIN, TerrainConfig};

/// Simulated frame time (windowed present pacing is about 31 ms).
const DT: f64 = 1.0 / 30.0;
/// Hover frames after the descent ends (counted).
const SETTLE_FRAMES: u64 = 300;
/// Yields per frame given to the worker threads (a count, not a time).
const PUMP_YIELDS: u32 = 200;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flight {
    Ordinary,
    Fast,
}

fn execute(flight: Flight) -> (StreamRun, StreamMetrics) {
    let mut settings = AppSettings {
        terrain: TerrainConfig::default(),
        ..Default::default()
    };
    let r = settings.terrain.terrain.radius;
    let script = match flight {
        Flight::Ordinary => DescentScript::orbit_to_surface(r),
        Flight::Fast => DescentScript::fast_free_flight(r),
    };
    settings.script = Some(script);
    let frames = (script.descent_seconds / DT).ceil() as u64 + SETTLE_FRAMES;
    let run = run_stream(
        settings,
        Box::new(SimClock::new(33_333_333)),
        frames,
        DT,
        &mut || {
            for _ in 0..PUMP_YIELDS {
                std::thread::yield_now();
            }
        },
    )
    .expect("stream run");
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
    assert!(m.partition_every_frame, "drawn was not a partition");
    for (i, r) in run.frames.iter().enumerate() {
        assert!(r.started <= cfg.stream.max_starts_per_frame, "f{i} started");
        assert!(r.uploads <= cfg.uploads_per_frame, "f{i} uploads");
        assert!(
            r.requests_issued <= cfg.max_requests_per_frame,
            "f{i} requests"
        );
        assert!(r.draws <= MAX_DRAWS_PER_FRAME, "f{i} draws");
        assert!(
            r.draw_items <= MAX_DRAWS_PER_FRAME - DRAW_MARGIN,
            "f{i} draw items"
        );
        assert!(r.wanted_leaves <= cfg.lod.max_leaves, "f{i} leaves");
    }
}

#[test]
fn fast_descent_lag_report() {
    let (run, m) = shared(Flight::Fast);
    println!("{}", report_text("fast free-flight, async", m));
    assert!(run.frames.len() as u64 > SETTLE_FRAMES);
    assert_invariants(run, m);
}

#[test]
fn ordinary_descent_lag_report() {
    let (run, m) = shared(Flight::Ordinary);
    println!("{}", report_text("scripted descent, async", m));
    assert_invariants(run, m);
}
