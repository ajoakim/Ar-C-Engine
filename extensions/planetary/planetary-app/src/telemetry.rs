//! Frame telemetry for the planetary app.
//!
//! The ONLY module in the planetary crates allowed to read wall-clock time; see tests/sdk_only.rs.
//! Time is read through the [`Clock`] trait so deterministic tests use [`SimClock`].

use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;

/// Number of most recent samples kept in memory.
pub const RECENT_CAP: usize = 600;

/// Monotonic nanosecond clock.
pub trait Clock: Send {
    fn now_ns(&mut self) -> u64;
}

/// Real time since construction.
pub struct WallClock {
    start: std::time::Instant,
}

impl WallClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }
}

impl Default for WallClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for WallClock {
    fn now_ns(&mut self) -> u64 {
        u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}

/// Deterministic clock advancing a fixed step per read.
pub struct SimClock {
    t_ns: u64,
    step_ns: u64,
}

impl SimClock {
    #[must_use]
    pub fn new(step_ns: u64) -> Self {
        Self { t_ns: 0, step_ns }
    }
}

impl Clock for SimClock {
    fn now_ns(&mut self) -> u64 {
        let t = self.t_ns;
        self.t_ns = self.t_ns.saturating_add(self.step_ns);
        t
    }
}

/// One frame of measurements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameSample {
    pub frame: u64,
    pub sim_time_s: f64,
    pub frame_ms: f64,
    pub prepare_ms: f64,
    pub gpu_frame_ms: f64,
    /// `|eye| - surface_radius_at(dir)`.
    pub altitude_m: f64,
    /// `|eye| - nominal radius`.
    pub nominal_altitude_m: f64,
    pub center_distance_m: f64,
    pub nodes_visited: u32,
    pub leaves: usize,
    pub wanted_leaves: usize,
    pub drawn_leaves: usize,
    pub held_meshes: usize,
    pub pending: usize,
    pub in_flight: usize,
    pub awaiting_upload: usize,
    pub started: u32,
    pub completions: u32,
    pub uploads: u32,
    pub uploaded_bytes: u64,
    pub evicted: u32,
    pub cancelled: u32,
    pub destroyed: u32,
    pub upload_rejects: u32,
    pub draw_overflow: u32,
    pub tau_eff: f64,
    pub tau_steps: u32,
}

/// Aggregate frame-time statistics.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PerfSummary {
    pub count: usize,
    pub avg_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub fps: f64,
}

/// Average, nearest-rank p95, max and FPS of `frame_ms`.
#[must_use]
pub fn summarize(frame_ms: &[f64]) -> PerfSummary {
    if frame_ms.is_empty() {
        return PerfSummary::default();
    }
    let mut v = frame_ms.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    let avg = v.iter().sum::<f64>() / n as f64;
    // ceil(0.95 * n) in integer arithmetic.
    let rank = (n * 95).div_ceil(100).max(1);
    PerfSummary {
        count: n,
        avg_ms: avg,
        p95_ms: v[rank - 1],
        max_ms: v[n - 1],
        fps: if avg > 0.0 { 1000.0 / avg } else { 0.0 },
    }
}

/// Altitude bands: (label, lower inclusive, upper exclusive), highest first.
pub const ALTITUDE_BANDS: [(&str, f64, f64); 6] = [
    (">1000 km", 1.0e6, f64::INFINITY),
    ("100-1000 km", 1.0e5, 1.0e6),
    ("10-100 km", 1.0e4, 1.0e5),
    ("1-10 km", 1.0e3, 1.0e4),
    ("100 m-1 km", 1.0e2, 1.0e3),
    ("<100 m", f64::NEG_INFINITY, 1.0e2),
];

/// Index into [`ALTITUDE_BANDS`] for an altitude.
#[must_use]
pub fn band_of(altitude_m: f64) -> usize {
    ALTITUDE_BANDS
        .iter()
        .position(|&(_, lo, hi)| altitude_m >= lo && altitude_m < hi)
        .unwrap_or(ALTITUDE_BANDS.len() - 1)
}

const COLUMNS: [&str; 27] = [
    "frame",
    "sim_time_s",
    "frame_ms",
    "prepare_ms",
    "gpu_frame_ms",
    "altitude_m",
    "nominal_altitude_m",
    "center_distance_m",
    "nodes_visited",
    "leaves",
    "wanted_leaves",
    "drawn_leaves",
    "held_meshes",
    "pending",
    "in_flight",
    "awaiting_upload",
    "started",
    "completions",
    "uploads",
    "uploaded_bytes",
    "evicted",
    "cancelled",
    "destroyed",
    "upload_rejects",
    "draw_overflow",
    "tau_eff",
    "tau_steps",
];

/// CSV header line (no newline).
#[must_use]
pub fn csv_header() -> &'static str {
    "frame,sim_time_s,frame_ms,prepare_ms,gpu_frame_ms,altitude_m,nominal_altitude_m,center_distance_m,nodes_visited,leaves,wanted_leaves,drawn_leaves,held_meshes,pending,in_flight,awaiting_upload,started,completions,uploads,uploaded_bytes,evicted,cancelled,destroyed,upload_rejects,draw_overflow,tau_eff,tau_steps"
}

/// One CSV row (no newline).
#[must_use]
pub fn csv_row(s: &FrameSample) -> String {
    format!(
        "{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{}",
        s.frame,
        s.sim_time_s,
        s.frame_ms,
        s.prepare_ms,
        s.gpu_frame_ms,
        s.altitude_m,
        s.nominal_altitude_m,
        s.center_distance_m,
        s.nodes_visited,
        s.leaves,
        s.wanted_leaves,
        s.drawn_leaves,
        s.held_meshes,
        s.pending,
        s.in_flight,
        s.awaiting_upload,
        s.started,
        s.completions,
        s.uploads,
        s.uploaded_bytes,
        s.evicted,
        s.cancelled,
        s.destroyed,
        s.upload_rejects,
        s.draw_overflow,
        s.tau_eff,
        s.tau_steps,
    )
}

/// Telemetry options carried in `AppSettings`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PerfSettings {
    pub csv_path: Option<PathBuf>,
    pub console_summary: bool,
    pub exit_after_frames: Option<u64>,
}

/// Records per-frame samples, CSV rows and console summaries.
pub struct Telemetry {
    clock: Box<dyn Clock>,
    recent: VecDeque<FrameSample>,
    csv: Option<std::io::BufWriter<std::fs::File>>,
    last_frame_end_ns: Option<u64>,
    last_console_ns: u64,
    console: bool,
    phase_start_ns: u64,
    prepare_ms: f64,
    gpu_start_ns: u64,
    rows: u64,
    since_console: usize,
}

impl Telemetry {
    /// # Errors
    /// I/O error creating the CSV file.
    pub fn new(mut clock: Box<dyn Clock>, settings: &PerfSettings) -> std::io::Result<Self> {
        let csv = match &settings.csv_path {
            Some(p) => {
                let mut w = std::io::BufWriter::new(std::fs::File::create(p)?);
                writeln!(w, "{}", csv_header())?;
                Some(w)
            }
            None => None,
        };
        let now = clock.now_ns();
        Ok(Self {
            clock,
            recent: VecDeque::new(),
            csv,
            last_frame_end_ns: None,
            last_console_ns: now,
            console: settings.console_summary,
            phase_start_ns: now,
            prepare_ms: 0.0,
            gpu_start_ns: now,
            rows: 0,
            since_console: 0,
        })
    }

    /// Marks the start of the prepare phase.
    pub fn begin_phase(&mut self) {
        self.phase_start_ns = self.clock.now_ns();
    }

    /// Marks the end of the prepare phase.
    pub fn end_prepare(&mut self) {
        let now = self.clock.now_ns();
        self.prepare_ms = ns_to_ms(now.saturating_sub(self.phase_start_ns));
    }

    /// Marks the start of `gpu_frame`; returns the mark.
    pub fn measure_gpu_frame_start(&mut self) -> u64 {
        self.gpu_start_ns = self.clock.now_ns();
        self.gpu_start_ns
    }

    /// Completes a sample: fills timings, stores, writes CSV and console output.
    pub fn record(&mut self, mut sample: FrameSample) {
        let now = self.clock.now_ns();
        sample.gpu_frame_ms = ns_to_ms(now.saturating_sub(self.gpu_start_ns));
        sample.prepare_ms = self.prepare_ms;
        let frame_start = self.last_frame_end_ns.unwrap_or(self.phase_start_ns);
        sample.frame_ms = ns_to_ms(now.saturating_sub(frame_start));
        self.last_frame_end_ns = Some(now);
        self.prepare_ms = 0.0;

        if let Some(w) = self.csv.as_mut() {
            // Telemetry must never take the app down; a failed write drops the log.
            let ok = writeln!(w, "{}", csv_row(&sample)).is_ok();
            self.rows += 1;
            if !ok || (self.rows.is_multiple_of(60) && w.flush().is_err()) {
                self.csv = None;
            }
        }
        self.recent.push_back(sample);
        while self.recent.len() > RECENT_CAP {
            self.recent.pop_front();
        }
        self.since_console += 1;

        if self.console && now.saturating_sub(self.last_console_ns) >= 1_000_000_000 {
            self.last_console_ns = now;
            let n = self.since_console.min(self.recent.len());
            self.since_console = 0;
            let ms: Vec<f64> = self
                .recent
                .iter()
                .skip(self.recent.len() - n)
                .map(|s| s.frame_ms)
                .collect();
            let sum = summarize(&ms);
            if let Some(l) = self.recent.back() {
                println!(
                    "perf: frames={} avg={:.2}ms p95={:.2}ms max={:.2}ms fps={:.1} alt={:.0}m nodes={} wanted={} drawn={} held={} pending={}",
                    sum.count,
                    sum.avg_ms,
                    sum.p95_ms,
                    sum.max_ms,
                    sum.fps,
                    l.altitude_m,
                    l.nodes_visited,
                    l.wanted_leaves,
                    l.drawn_leaves,
                    l.held_meshes,
                    l.pending
                );
            }
        }
    }

    #[must_use]
    pub fn last(&self) -> Option<&FrameSample> {
        self.recent.back()
    }

    pub fn recent(&self) -> impl Iterator<Item = &FrameSample> {
        self.recent.iter()
    }

    pub fn flush(&mut self) {
        if let Some(w) = self.csv.as_mut() {
            let _ = w.flush();
        }
    }
}

fn ns_to_ms(ns: u64) -> f64 {
    ns as f64 / 1.0e6
}

/// Number of CSV columns.
#[must_use]
pub fn csv_columns() -> usize {
    COLUMNS.len()
}
