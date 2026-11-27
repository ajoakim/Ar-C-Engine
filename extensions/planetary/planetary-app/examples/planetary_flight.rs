//! Planetary reference app: orbit-to-surface flight over streamed terrain, SDK only.
//!
//! Controls: WASD/arrows move, E/Space up, Q/Ctrl down, Shift boost,
//! hold right mouse button to look, Esc quits. With `--features debug-overlay` (windowed), F3
//! toggles a read-only overlay: LOD histograms, streaming queue, memory and upload budgets,
//! frame and prepare times.
//!
//! Recommended: `cargo run --release -p planetary-app --example planetary_flight -- --descent --perf-log flight.csv`
//!
//! Usage: `planetary_flight [--topology <hex|cube>] [--debug-cells] [--headless <frames>] [--descent] [--ascent] [--frames <n>]
//! [--perf-log <file.csv>] [--no-console] [--bench-descent] [--budget-ms <ms>] [--strict]`
//!
//! `--topology` selects the planet: `hex` (default, icosahedral hexagonal cells, 16 cells per
//! cell edge) or `cube` (cube-sphere reference, 32 cells per edge); any other value is a usage
//! error. `--debug-cells` colours every cell by level and id so hexes visibly give way to their
//! parts while descending.
//!
//! `--present-mode <fifo|mailbox|immediate|auto>` selects the surface present mode (default
//! fifo; an unsupported mode falls back to fifo with a warning) and `--frame-latency <1|2|3>`
//! the maximum frames in flight (default 2); other values are usage errors. `--no-haze` turns
//! the altitude haze off and `--no-atmosphere` keeps the plain haze but drops the sun-tinted sky
//! and the limb ring (ADR-0019). `--always-reselect` runs the LOD selection every frame instead of reusing a
//! selection that provably still holds (same result, only slower; for comparisons). The startup line prints the adapter, requested and active mode,
//! supported modes, latency and monitor refresh rate; the window title shows altitude, speed
//! and level.
//!
//! `--diagnostics` turns on the per-frame churn counters (drawn cells appearing, disappearing,
//! flip-flopping, cull toggles, overlaps) in the perf log, the console line and `--out`.
//! `--retain-frames <n>` sets how many frames a split cell that is culled stays split (default 8,
//! 0 switches the retention off to compare).
//!
//! `--headless` runs on the null backend with `settle_each_frame` enabled so the smoke run
//! is deterministic (workers are awaited every frame); the windowed run never blocks.
//! `--frames <n>` exits after n frames; `--perf-log` writes one CSV row per frame;
//! `--bench-descent` runs the scripted descent headless at the production defaults and prints
//! per-altitude-band frame-time statistics with PASS/FAIL against `--budget-ms` (default 16.7).
//!
//! `--bench-stream` runs the windowed-realistic streaming harness headless: workers run freely
//! (no settle unless `--settle`), the script advances by a simulated 1/30 s per frame, and each
//! frame is paced by `--frame-ms` (default 33) of real time spent spinning so workers get the
//! time a window frame would give them. `--fast` uses the fast free-flight descent instead of
//! the scripted one. `--frames <n>` bounds the run (default descent frames plus 300 hover);
//! `--out` writes the per-frame CSV. Streaming caps adapt to the measured frame cost by default;
//! `--no-adaptive` keeps the fixed caps (ordering stays nearest-first). It prints lag at the camera, frames-to-detail and the
//! upload-versus-demand throughput ratio.
//!
//! Use `cargo run --release -p planetary-app --example planetary_flight -- --descent` for
//! flights; debug builds are several times slower.
#![allow(clippy::result_large_err)]

use engine_sdk::{
    AppConfig, Engine, EngineConfig, FrameLatency, NullBackend, PresentConfig, PresentModeSetting,
    Runtime, SdkError, WindowConfig, run_app,
};
use planetary_app::stream_bench::{analyze, csv, report_text, run_stream};
use planetary_app::{
    ALTITUDE_BANDS, AppSettings, DescentScript, PlanetaryApp, TopologyKind, WallClock, band_of,
    spin_for_ms, summarize,
};

const USAGE: &str = "usage: planetary_flight [--topology <hex|cube>] [--debug-cells] [--headless <frames>] [--descent] [--ascent] [--frames <n>] [--perf-log <file.csv>] [--no-console] [--bench-descent] [--budget-ms <ms>] [--strict] [--bench-stream] [--fast] [--settle] [--no-adaptive] [--frame-ms <ms>] [--out <file.csv>] [--diagnostics] [--retain-frames <n>] [--present-mode <fifo|mailbox|immediate|auto>] [--frame-latency <1|2|3>] [--no-haze] [--no-atmosphere] [--always-reselect]";

#[derive(Default, Clone, Copy)]
struct BandMax {
    nodes: u32,
    wanted: usize,
    drawn: usize,
    held: usize,
    pending: usize,
}

fn bench(
    mut settings: AppSettings,
    topology: TopologyKind,
    n: u64,
    budget: f64,
    strict: bool,
) -> Result<(), SdkError> {
    settings.script = Some(DescentScript::orbit_to_surface(
        settings.terrain.terrain.radius,
    ));
    let mut app = PlanetaryApp::with_clock(settings, Box::new(WallClock::new()))?;
    let mut rt = Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new())?;
    rt.setup(&mut app)?;
    let nb = ALTITUDE_BANDS.len();
    let mut times: Vec<Vec<f64>> = vec![Vec::new(); nb];
    let mut maxes = vec![BandMax::default(); nb];
    for _ in 0..n {
        rt.frame(&mut app, 1.0 / 60.0)?;
        if let Some(s) = app.telemetry().last() {
            let b = band_of(s.altitude_m);
            times[b].push(s.frame_ms);
            let m = &mut maxes[b];
            m.nodes = m.nodes.max(s.nodes_visited);
            m.wanted = m.wanted.max(s.wanted_leaves);
            m.drawn = m.drawn.max(s.drawn_leaves);
            m.held = m.held.max(s.held_meshes);
            m.pending = m.pending.max(s.pending);
        }
    }
    println!(
        "build: {}, topology: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        topology.name()
    );
    println!(
        "band | frames | avg ms | p95 ms | max ms | fps | stalls(>100 ms) | max nodes | max wanted | max drawn | max held | max pending"
    );
    let mut failed = Vec::new();
    for (i, (label, _, _)) in ALTITUDE_BANDS.iter().enumerate() {
        let t = &times[i];
        if t.is_empty() {
            println!("{label} | 0 | - | - | - | - | - | - | - | - | - | -");
            continue;
        }
        let s = summarize(t);
        let stalls = t.iter().filter(|&&x| x > 100.0).count();
        let m = maxes[i];
        println!(
            "{label} | {} | {:.2} | {:.2} | {:.2} | {:.1} | {stalls} | {} | {} | {} | {} | {}",
            s.count,
            s.avg_ms,
            s.p95_ms,
            s.max_ms,
            s.fps,
            m.nodes,
            m.wanted,
            m.drawn,
            m.held,
            m.pending
        );
        if s.p95_ms > budget {
            failed.push(*label);
        }
    }
    println!("budget: p95 <= {budget} ms per band");
    let ok = failed.is_empty();
    if ok {
        println!("RESULT: PASS");
    } else {
        println!("RESULT: FAIL (bands: {})", failed.join(", "));
    }
    app.shutdown();
    rt.shutdown()?;
    if !ok && strict {
        return Err(SdkError::App("bench budget exceeded".into()));
    }
    Ok(())
}

struct StreamOpts {
    fast: bool,
    settle: bool,
    frame_ms: f64,
    out: Option<std::path::PathBuf>,
}

fn bench_stream(
    mut settings: AppSettings,
    topology: TopologyKind,
    frames: Option<u64>,
    o: &StreamOpts,
) -> Result<(), SdkError> {
    const DT: f64 = 1.0 / 30.0;
    let r = settings.terrain.terrain.radius;
    let script = if o.fast {
        DescentScript::fast_free_flight(r)
    } else {
        DescentScript::orbit_to_surface(r)
    };
    settings.script = Some(script);
    settings.terrain.settle_each_frame = o.settle;
    settings.perf.exit_after_frames = None;
    settings.perf.console_summary = false;
    settings.perf.csv_path = None;
    let n = frames.unwrap_or_else(|| (script.descent_seconds / DT).ceil() as u64 + 300);
    let pace_ms = o.frame_ms;
    let mut run = run_stream(settings, Box::new(WallClock::new()), n, DT, &mut |_| {
        spin_for_ms(pace_ms)
    })?;
    run.descent_frames = (script.descent_seconds / DT).ceil() as usize;
    let m = analyze(&run);
    println!(
        "build: {}, topology: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        topology.name()
    );
    let label = format!(
        "{} descent, {}, {} ms/frame paced, {} frames",
        if o.fast {
            "fast free-flight"
        } else {
            "scripted"
        },
        if o.settle {
            "settle (control)"
        } else {
            "async workers"
        },
        o.frame_ms,
        n
    );
    print!("{}", report_text(&label, &m));
    if let Some(path) = &o.out {
        std::fs::write(path, csv(&run)).map_err(|e| SdkError::App(format!("write csv: {e}")))?;
    }
    Ok(())
}

/// Strictly parse the value of `--present-mode`; `None` for a missing or unknown value.
fn parse_present_mode(v: Option<String>) -> Option<PresentModeSetting> {
    PresentModeSetting::parse(&v?).ok()
}

/// Strictly parse the value of `--frame-latency` (1..=3); `None` otherwise.
fn parse_frame_latency(v: Option<String>) -> Option<FrameLatency> {
    FrameLatency::parse(&v?).ok()
}

fn main() -> Result<(), SdkError> {
    let mut headless: Option<u32> = None;
    let mut descent = false;
    let mut ascent = false;
    let mut frames: Option<u64> = None;
    let mut perf_log: Option<std::path::PathBuf> = None;
    let mut console = true;
    let mut bench_descent = false;
    let mut budget = 16.7_f64;
    let mut strict = false;
    let mut bench_stream_flag = false;
    let mut adaptive = true;
    let mut topology = TopologyKind::Hex;
    let mut debug_cells = false;
    let mut present = PresentConfig::default();
    let mut haze = true;
    let mut atmosphere = true;
    let mut always_reselect = false;
    let mut diagnostics = false;
    let mut retain_frames: Option<u32> = None;
    let mut so = StreamOpts {
        fast: false,
        settle: false,
        frame_ms: 33.0,
        out: None,
    };
    let usage = || SdkError::App(USAGE.into());
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--headless" => {
                let n = args
                    .next()
                    .and_then(|v| v.parse::<u32>().ok())
                    .ok_or_else(usage)?;
                headless = Some(n);
            }
            "--topology" => {
                topology = args
                    .next()
                    .and_then(|v| TopologyKind::parse(&v))
                    .ok_or_else(usage)?;
            }
            "--present-mode" => {
                present.mode = parse_present_mode(args.next()).ok_or_else(usage)?;
            }
            "--frame-latency" => {
                present.frame_latency = parse_frame_latency(args.next()).ok_or_else(usage)?;
            }
            "--no-haze" => haze = false,
            "--no-atmosphere" => atmosphere = false,
            "--always-reselect" => always_reselect = true,
            "--debug-cells" => debug_cells = true,
            "--diagnostics" => diagnostics = true,
            "--retain-frames" => {
                retain_frames = Some(
                    args.next()
                        .and_then(|v| v.parse::<u32>().ok())
                        .ok_or_else(usage)?,
                );
            }
            "--descent" => descent = true,
            "--ascent" => ascent = true,
            "--frames" => {
                frames = Some(
                    args.next()
                        .and_then(|v| v.parse::<u64>().ok())
                        .ok_or_else(usage)?,
                );
            }
            "--perf-log" => perf_log = Some(args.next().ok_or_else(usage)?.into()),
            "--no-console" => console = false,
            "--bench-descent" => bench_descent = true,
            "--budget-ms" => {
                let v = args
                    .next()
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .ok_or_else(usage)?;
                budget = v;
            }
            "--strict" => strict = true,
            "--bench-stream" => bench_stream_flag = true,
            "--fast" => so.fast = true,
            "--no-adaptive" => adaptive = false,
            "--settle" => so.settle = true,
            "--frame-ms" => {
                so.frame_ms = args
                    .next()
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite() && *v >= 0.0)
                    .ok_or_else(usage)?;
            }
            "--out" => so.out = Some(args.next().ok_or_else(usage)?.into()),
            _ => return Err(usage()),
        }
    }

    let mut settings = match topology {
        TopologyKind::Hex => AppSettings::default(),
        TopologyKind::Cube => AppSettings::cube_default(),
    };
    settings.terrain.debug_cells = debug_cells;
    settings.diagnostics = diagnostics;
    settings.haze = haze;
    settings.atmosphere = atmosphere;
    settings.terrain.always_reselect = always_reselect;
    if let Some(n) = retain_frames {
        settings.terrain.retain_frames = n;
    }
    settings.perf.exit_after_frames = frames;
    settings.perf.csv_path = perf_log;
    if descent {
        let r = settings.terrain.terrain.radius;
        settings.script = Some(if so.fast {
            DescentScript::fast_free_flight(r)
        } else {
            DescentScript::orbit_to_surface(r)
        });
    }
    if ascent {
        settings.script = Some(DescentScript::surface_to_orbit(
            settings.terrain.terrain.radius,
        ));
    }

    // Adaptive streaming caps for every wall-clock run (headless smoke keeps the fixed caps).
    settings.adaptive_streaming = adaptive && headless.is_none();

    if bench_stream_flag {
        return bench_stream(settings, topology, frames, &so);
    }

    if bench_descent {
        settings.perf.console_summary = console;
        // Bench runs to its own frame count; the app must not exit early.
        settings.perf.exit_after_frames = None;
        return bench(settings, topology, frames.unwrap_or(600), budget, strict);
    }

    if let Some(n) = headless {
        settings.terrain.settle_each_frame = true;
        let mut app = PlanetaryApp::new(settings)?;
        let mut rt = Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new())?;
        rt.setup(&mut app)?;
        for _ in 0..n {
            rt.frame(&mut app, 1.0 / 60.0)?;
        }
        let draws = rt
            .backend()
            .last_frame()
            .map_or(0, |f| f.passes.iter().map(|p| p.draws.len()).sum::<usize>());
        let held = app.report().held_meshes;
        println!(
            "planetary_flight headless ({}): {n} frames, {draws} draws in last frame, {held} meshes",
            topology.name()
        );
        app.shutdown();
        rt.shutdown()?;
        if draws == 0 {
            return Err(SdkError::App("headless run produced zero draws".into()));
        }
        return Ok(());
    }

    println!(
        "Controls: WASD/arrows move, E/Space up, Q/Ctrl down, Shift boost, right mouse look, Esc exit"
    );
    settings.perf.console_summary = console;
    let app = PlanetaryApp::with_clock(settings, Box::new(WallClock::new()))?;
    run_app(
        app,
        AppConfig {
            window: WindowConfig {
                title: "planetary_flight".into(),
                ..Default::default()
            },
            engine: EngineConfig::default(),
            present,
        },
    )
}

#[cfg(test)]
mod tests {
    fn some(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn present_mode_flag_is_strict() {
        assert_eq!(super::parse_present_mode(some("bogus")), None);
        assert_eq!(super::parse_present_mode(None), None);
        assert_eq!(super::parse_present_mode(some("Fifo")), None);
        assert_eq!(
            super::parse_present_mode(some("mailbox")),
            Some(engine_sdk::PresentModeSetting::Mailbox)
        );
        assert_eq!(
            super::parse_present_mode(some("auto")),
            Some(engine_sdk::PresentModeSetting::Auto)
        );
    }

    #[test]
    fn frame_latency_flag_is_strict() {
        assert!(super::parse_frame_latency(some("0")).is_none());
        assert!(super::parse_frame_latency(some("4")).is_none());
        assert!(super::parse_frame_latency(None).is_none());
        for n in ["1", "2", "3"] {
            assert!(super::parse_frame_latency(some(n)).is_some());
        }
    }

    #[test]
    fn usage_names_the_new_flags() {
        for f in [
            "--present-mode",
            "--frame-latency",
            "--no-haze",
            "--no-atmosphere",
            "--always-reselect",
            "--topology",
        ] {
            assert!(super::USAGE.contains(f), "{f}");
        }
    }
}
