//! Streaming/prepare frame bench (QA-01): the real `PlanetaryApp` on the SDK `NullBackend`,
//! scripted orbit-to-surface descent, `settle_each_frame = true`.
//!
//! `stream_bench::run_stream` is deliberately NOT used as the body: it is paced by the wall
//! clock (`settle_each_frame: false`, worker threads racing the frame loop), so its numbers
//! depend on scheduler timing and sleeps. With `settle_each_frame` each frame waits for the
//! terrain workers to finish its requests, and `PlanetaryApp::new` uses a `SimClock`, so the
//! frame sequence is a pure function of the settings and the measured time is the CPU cost of
//! select + generate + mesh + extract per frame.
//!
//! Each sample builds a fresh app (untimed), advances `warm` frames of the 8 s script at
//! 1/60 s (untimed), then times `WINDOW` further frames. Divide by `WINDOW` for per-frame time.
//! A hex frame near the surface costs on the order of 14 ms in release (ADR-0015), hence the
//! small sample size. Nothing about timing is asserted.

use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use engine_sdk::{Engine, EngineConfig, NullBackend, Runtime};
use planetary_app::{AppSettings, DescentScript, PlanetaryApp, TopologyKind};

const DT: f64 = 1.0 / 60.0;
/// Frames timed per iteration.
const WINDOW: u32 = 30;

fn settings(kind: TopologyKind) -> AppSettings {
    let mut s = match kind {
        TopologyKind::Hex => AppSettings::default(),
        TopologyKind::Cube => AppSettings::cube_default(),
    };
    s.terrain.settle_each_frame = true;
    s.script = Some(DescentScript::orbit_to_surface(s.terrain.terrain.radius));
    s
}

fn start(kind: TopologyKind, warm: u32) -> (PlanetaryApp, Runtime<NullBackend>) {
    let mut app = PlanetaryApp::new(settings(kind)).expect("app");
    let mut rt =
        Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).expect("runtime");
    rt.setup(&mut app).expect("setup");
    for _ in 0..warm {
        rt.frame(&mut app, DT).expect("frame");
    }
    (app, rt)
}

fn bench_stream(c: &mut Criterion) {
    let mut g = c.benchmark_group("stream_frames_x30");
    g.sample_size(10);
    g.measurement_time(Duration::from_secs(10));
    // Frame 0..30 is the orbit start (initial generation burst); 400..430 is the last
    // ~10 percent of the descent (low altitude, deepest selection).
    for (label, kind) in [("hex", TopologyKind::Hex), ("cube", TopologyKind::Cube)] {
        for (phase, warm) in [("orbit_start", 0u32), ("low_altitude", 400)] {
            g.bench_function(format!("{label}/{phase}"), |b| {
                b.iter_batched(
                    || start(kind, warm),
                    |(mut app, mut rt)| {
                        for _ in 0..WINDOW {
                            rt.frame(&mut app, DT).expect("frame");
                        }
                        (app, rt) // dropped (worker shutdown) outside the timed region
                    },
                    BatchSize::PerIteration,
                );
            });
        }
    }
    g.finish();
}

criterion_group!(benches, bench_stream);
criterion_main!(benches);
