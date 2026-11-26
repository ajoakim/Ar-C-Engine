//! Deterministic telemetry tests (SimClock only).

use engine_sdk::{Engine, EngineConfig, NullBackend, Runtime};
use planetary_app::telemetry::{csv_columns, csv_header, csv_row};
use planetary_app::{
    ALTITUDE_BANDS, AppSettings, Clock, FrameSample, PlanetaryApp, SimClock, band_of, summarize,
};

#[test]
fn p95_nearest_rank() {
    assert_eq!(summarize(&[]).count, 0);
    let v: Vec<f64> = (1..=100).map(f64::from).collect();
    let s = summarize(&v);
    assert_eq!(s.count, 100);
    assert!((s.avg_ms - 50.5).abs() < 1e-9);
    assert!((s.p95_ms - 95.0).abs() < 1e-9);
    assert!((s.max_ms - 100.0).abs() < 1e-9);
    assert!((s.fps - 1000.0 / 50.5).abs() < 1e-9);
}

#[test]
fn band_edges() {
    assert_eq!(band_of(2_000_000.0), 0);
    assert_eq!(band_of(500_000.0), 1);
    assert_eq!(band_of(50_000.0), 2);
    assert_eq!(band_of(5_000.0), 3);
    assert_eq!(band_of(500.0), 4);
    assert_eq!(band_of(50.0), 5);
    assert_eq!(band_of(-10.0), 5);
    assert_eq!(ALTITUDE_BANDS.len(), 6);
}

#[test]
fn sim_clock_steps() {
    let mut c = SimClock::new(1_000);
    assert_eq!(c.now_ns(), 0);
    assert_eq!(c.now_ns(), 1_000);
    assert_eq!(c.now_ns(), 2_000);
}

#[test]
fn csv_columns_match_header() {
    let row = csv_row(&FrameSample::default());
    let n = csv_header().split(',').count();
    assert_eq!(row.split(',').count(), n);
    assert_eq!(csv_columns(), n);
    assert!(csv_header().starts_with("frame,sim_time_s,frame_ms"));
    let churn = [
        "level_changes",
        "drawn_appeared",
        "drawn_disappeared",
        "cull_toggles",
        "overlap_pairs",
        "flipflops",
        "flipflops_unexplained",
    ];
    let cols: Vec<&str> = csv_header().split(',').collect();
    assert_eq!(&cols[cols.len() - 7..], &churn);
    let s = FrameSample {
        level_changes: 1,
        drawn_appeared: 2,
        drawn_disappeared: 3,
        cull_toggles: 4,
        overlap_pairs: 5,
        flipflops: 6,
        flipflops_unexplained: 7,
        ..FrameSample::default()
    };
    let row = csv_row(&s);
    let cells: Vec<&str> = row.split(',').collect();
    assert_eq!(
        &cells[cells.len() - 7..],
        &["1", "2", "3", "4", "5", "6", "7"]
    );
}

#[test]
fn churn_counters_default_to_zero_without_diagnostics() {
    for s in run() {
        assert_eq!(
            (
                s.level_changes,
                s.drawn_appeared,
                s.drawn_disappeared,
                s.cull_toggles,
                s.overlap_pairs,
                s.flipflops,
                s.flipflops_unexplained
            ),
            (0, 0, 0, 0, 0, 0, 0)
        );
    }
}

fn run() -> Vec<FrameSample> {
    let mut settings = AppSettings::default();
    settings.terrain.settle_each_frame = true;
    settings.terrain.cells_per_edge = 8;
    let mut app = PlanetaryApp::new(settings).unwrap();
    let mut rt = Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).unwrap();
    rt.setup(&mut app).unwrap();
    for _ in 0..20 {
        rt.frame(&mut app, 1.0 / 60.0).unwrap();
    }
    let out: Vec<FrameSample> = app.telemetry().recent().cloned().collect();
    assert_eq!(app.frames_recorded(), 20);
    app.shutdown();
    rt.shutdown().unwrap();
    out
}

#[test]
fn headless_samples_are_deterministic() {
    let a = run();
    let b = run();
    assert_eq!(a.len(), 20);
    let frames: Vec<u64> = a.iter().map(|s| s.frame).collect();
    assert_eq!(frames, (1..=20).collect::<Vec<u64>>());
    assert_eq!(a, b);
}
