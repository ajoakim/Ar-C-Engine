//! Planetary reference app: orbit-to-surface flight over streamed terrain, SDK only.
//!
//! Controls: WASD/arrows move, E/Space up, Q/Ctrl down, Shift boost,
//! hold right mouse button to look, Esc quits.
//!
//! Recommended: `cargo run --release -p planetary-app --example planetary_flight -- --descent --perf-log flight.csv`
//!
//! Usage: `planetary_flight [--headless <frames>] [--descent] [--ascent] [--frames <n>]
//! [--perf-log <file.csv>] [--no-console] [--bench-descent] [--budget-ms <ms>] [--strict]`
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
//! `--out` writes the per-frame CSV. It prints lag at the camera, frames-to-detail and the
//! upload-versus-demand throughput ratio.
//!
//! Use `cargo run --release -p planetary-app --example planetary_flight -- --descent` for
//! flights; debug builds are several times slower.
#![allow(clippy::result_large_err)]

use engine_sdk::{
    AppConfig, Engine, EngineConfig, NullBackend, Runtime, SdkError, WindowConfig, run_app,
};
use planetary_app::stream_bench::{analyze, csv, report_text, run_stream};
use planetary_app::{
    ALTITUDE_BANDS, AppSettings, DescentScript, PlanetaryApp, WallClock, band_of, spin_for_ms,
    summarize,
};

const USAGE: &str = "usage: planetary_flight [--headless <frames>] [--descent] [--ascent] [--frames <n>] [--perf-log <file.csv>] [--no-console] [--bench-descent] [--budget-ms <ms>] [--strict] [--bench-stream] [--fast] [--settle] [--frame-ms <ms>] [--out <file.csv>]";

#[derive(Default, Clone, Copy)]
struct BandMax {
    nodes: u32,
    wanted: usize,
    drawn: usize,
    held: usize,
    pending: usize,
}

fn bench(mut settings: AppSettings, n: u64, budget: f64, strict: bool) -> Result<(), SdkError> {
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
        "build: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
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
    let run = run_stream(settings, Box::new(WallClock::new()), n, DT, &mut || {
        spin_for_ms(pace_ms)
    })?;
    let m = analyze(&run);
    println!(
        "build: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
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

    let mut settings = AppSettings::default();
    settings.perf.exit_after_frames = frames;
    settings.perf.csv_path = perf_log;
    if descent {
        settings.script = Some(DescentScript::orbit_to_surface(
            settings.terrain.terrain.radius,
        ));
    }
    if ascent {
        settings.script = Some(DescentScript::surface_to_orbit(
            settings.terrain.terrain.radius,
        ));
    }

    if bench_stream_flag {
        return bench_stream(settings, frames, &so);
    }

    if bench_descent {
        settings.perf.console_summary = console;
        // Bench runs to its own frame count; the app must not exit early.
        settings.perf.exit_after_frames = None;
        return bench(settings, frames.unwrap_or(600), budget, strict);
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
            "planetary_flight headless: {n} frames, {draws} draws in last frame, {held} meshes"
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
        },
    )
}
