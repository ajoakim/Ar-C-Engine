//! Windowed-realistic streaming harness (07.1-19).
//!
//! Runs the real [`PlanetaryApp`] on the null backend with `settle_each_frame: false` (workers
//! run freely), a SIMULATED frame time (the script advances by `dt` per frame, never by wall
//! clock) and records, per frame, the wanted and drawn level at the camera. This module reads no
//! clock. Pacing is injected: the example passes a closure that spins on the telemetry wall
//! clock so workers get realistic real time per frame; tests pass a counted yield loop.

use std::collections::BTreeSet;

use engine_sdk::{
    DVec3, Engine, EngineConfig, MAX_DRAWS_PER_FRAME, NullBackend, Runtime, SdkError,
};
use planetary_core::lod::is_partition;
use planetary_core::{PatchId, locate};

use crate::telemetry::{ALTITUDE_BANDS, band_of};
use crate::{AppSettings, Clock, PlanetaryApp};

/// One recorded frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StreamFrame {
    pub frame: u64,
    pub sim_time_s: f64,
    pub altitude_m: f64,
    pub wanted_leaves: usize,
    pub drawn_leaves: usize,
    /// Wanted leaf level under the camera (nadir); -1 if none.
    pub wanted_level: i32,
    /// Drawn leaf level under the camera (nadir); -1 if none.
    pub drawn_level: i32,
    /// Wanted and drawn level at the look-at point (10 m ahead of the eye).
    pub look_wanted_level: i32,
    pub look_drawn_level: i32,
    pub held_meshes: usize,
    pub pending_requests: usize,
    pub queued: usize,
    pub started: u32,
    pub uploads: u32,
    pub in_flight: usize,
    pub awaiting_upload: usize,
    pub evicted: u32,
    pub cancelled: u32,
    pub requests_issued: u32,
    pub draws: usize,
    pub draw_items: usize,
    pub partition: bool,
    /// True when drawn equals wanted exactly (converged).
    pub converged: bool,
}

/// A whole run.
#[derive(Clone, Debug, Default)]
pub struct StreamRun {
    pub frames: Vec<StreamFrame>,
    /// Distinct wanted leaves seen over the run (demand).
    pub distinct_wanted: usize,
    pub dt: f64,
}

fn level_at(set: &[PatchId], d: DVec3) -> i32 {
    set.iter()
        .find(|p| locate(d, p.level()) == **p)
        .map_or(-1, |p| i32::from(p.level()))
}

fn sorted_raw(v: &[PatchId]) -> Vec<u64> {
    let mut r: Vec<u64> = v.iter().map(|p| p.raw()).collect();
    r.sort_unstable();
    r
}

/// Runs `frames` frames with a simulated step `dt` seconds each; `pace` is called after every
/// frame (it may spin or yield so workers progress; it must not change app state).
///
/// # Errors
/// Propagates app and runtime errors.
#[allow(clippy::result_large_err)]
pub fn run_stream(
    settings: AppSettings,
    clock: Box<dyn Clock>,
    frames: u64,
    dt: f64,
    pace: &mut dyn FnMut(),
) -> Result<StreamRun, SdkError> {
    let mut app = PlanetaryApp::with_clock(settings, clock)?;
    let mut rt = Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new())?;
    rt.setup(&mut app)?;
    let mut out = Vec::new();
    let mut seen: BTreeSet<u64> = BTreeSet::new();
    for _ in 0..frames {
        rt.frame(&mut app, dt)?;
        let rep = app.report().clone();
        let drawn: Vec<PatchId> = app.terrain().drawn().to_vec();
        let wanted: Vec<PatchId> = app.terrain().wanted_leaves().to_vec();
        for p in &wanted {
            seen.insert(p.raw());
        }
        let pos = app.camera().position();
        let nadir = pos.normalize();
        let ahead = (pos + app.camera().forward() * 10.0).normalize();
        let draws = rt
            .backend()
            .last_frame()
            .map_or(0, |f| f.passes.iter().map(|p| p.draws.len()).sum());
        let altitude_m = app
            .camera()
            .altitude(&|d| app.terrain().surface_radius_at(d));
        let converged = !drawn.is_empty() && sorted_raw(&drawn) == sorted_raw(&wanted);
        out.push(StreamFrame {
            frame: rep.frame,
            sim_time_s: app.simulated_time(),
            altitude_m,
            wanted_leaves: wanted.len(),
            drawn_leaves: drawn.len(),
            wanted_level: level_at(&wanted, nadir),
            drawn_level: level_at(&drawn, nadir),
            look_wanted_level: level_at(&wanted, ahead),
            look_drawn_level: level_at(&drawn, ahead),
            held_meshes: rep.held_meshes,
            pending_requests: rep.pending_requests,
            queued: rep.stream.pending,
            started: rep.stream.started,
            uploads: rep.stream.uploads,
            in_flight: rep.stream.in_flight,
            awaiting_upload: rep.stream.awaiting_upload,
            evicted: rep.stream.evicted,
            cancelled: rep.stream.cancelled,
            requests_issued: rep.requests_issued,
            draws,
            draw_items: rep.draw_items,
            partition: drawn.is_empty() || is_partition(&drawn),
            converged,
        });
        pace();
    }
    let distinct_wanted = seen.len();
    app.shutdown();
    rt.shutdown()?;
    Ok(StreamRun {
        frames: out,
        distinct_wanted,
        dt,
    })
}

/// Derived metrics of a run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StreamMetrics {
    /// Max `wanted_level - drawn_level` at the camera over frames after the warm-up.
    pub max_lag: i32,
    pub p95_lag: i32,
    /// Per altitude band: (max lag, p95 lag, frames).
    pub band_lag: Vec<(i32, i32, usize)>,
    /// Max over levels of frames from first wanted-at-camera to first drawn-at-camera.
    pub max_frames_to_detail: u64,
    /// Level that produced `max_frames_to_detail`.
    pub worst_detail_level: i32,
    /// Fraction of frames (after warm-up) with drawn level < wanted level - 1 at the camera.
    pub frac_lag_over_1: f64,
    pub total_uploads: u64,
    pub sim_seconds: f64,
    /// Patches uploaded per simulated second.
    pub uploads_per_s: f64,
    /// Distinct wanted leaves per simulated second.
    pub wanted_per_s: f64,
    /// `total_uploads / distinct_wanted` (throughput ratio; < 1 means supply is short).
    pub throughput_ratio: f64,
    pub max_held: usize,
    pub max_pending_requests: usize,
    pub max_queued: usize,
    pub max_awaiting_upload: usize,
    pub max_started: u32,
    pub max_uploads: u32,
    pub total_cancelled: u64,
    pub total_evicted: u64,
    /// Drawn is a partition on every frame from `bootstrap_frames` on (no hole once the first
    /// complete cover exists).
    pub partition_every_frame: bool,
    /// Frames until the first complete cover is drawn (the coarsest cells are not resident
    /// before it, so holes are unavoidable until then).
    pub bootstrap_frames: usize,
    pub max_draws: usize,
    pub max_wanted_leaves: usize,
}

fn percentile95(v: &mut [i32]) -> i32 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    let rank = (v.len() * 95).div_ceil(100).max(1);
    v[rank - 1]
}

/// Frames ignored at the start (the first drawn set must exist).
pub const WARM_UP: usize = 10;

/// Computes the metrics of `run`.
#[must_use]
pub fn analyze(run: &StreamRun) -> StreamMetrics {
    let f = &run.frames;
    let mut m = StreamMetrics {
        band_lag: vec![(0, 0, 0); ALTITUDE_BANDS.len()],
        partition_every_frame: true,
        ..StreamMetrics::default()
    };
    m.bootstrap_frames = f
        .iter()
        .position(|r| r.drawn_leaves > 0 && r.partition)
        .unwrap_or(f.len());
    let mut lags: Vec<i32> = Vec::new();
    let mut by_band: Vec<Vec<i32>> = vec![Vec::new(); ALTITUDE_BANDS.len()];
    let mut over1 = 0usize;
    let mut counted = 0usize;
    for (i, r) in f.iter().enumerate() {
        m.total_uploads += u64::from(r.uploads);
        m.total_cancelled += u64::from(r.cancelled);
        m.total_evicted += u64::from(r.evicted);
        m.max_held = m.max_held.max(r.held_meshes);
        m.max_pending_requests = m.max_pending_requests.max(r.pending_requests);
        m.max_queued = m.max_queued.max(r.queued);
        m.max_awaiting_upload = m.max_awaiting_upload.max(r.awaiting_upload);
        m.max_started = m.max_started.max(r.started);
        m.max_uploads = m.max_uploads.max(r.uploads);
        m.max_draws = m.max_draws.max(r.draws);
        m.max_wanted_leaves = m.max_wanted_leaves.max(r.wanted_leaves);
        if i >= m.bootstrap_frames {
            m.partition_every_frame &= r.partition;
        }
        if i >= WARM_UP {
            let lag = (r.wanted_level - r.drawn_level).max(0);
            lags.push(lag);
            by_band[band_of(r.altitude_m)].push(lag);
            counted += 1;
            if r.drawn_level < r.wanted_level - 1 {
                over1 += 1;
            }
        }
    }
    m.max_lag = lags.iter().copied().max().unwrap_or(0);
    m.p95_lag = percentile95(&mut lags);
    for (b, v) in by_band.iter_mut().enumerate() {
        let mx = v.iter().copied().max().unwrap_or(0);
        let n = v.len();
        m.band_lag[b] = (mx, percentile95(v), n);
    }
    m.frac_lag_over_1 = if counted > 0 {
        over1 as f64 / counted as f64
    } else {
        0.0
    };
    let top = f.iter().map(|r| r.wanted_level).max().unwrap_or(-1);
    for l in 1..=top {
        let Some(fw) = f.iter().position(|r| r.wanted_level >= l) else {
            continue;
        };
        let fd = f[fw..]
            .iter()
            .position(|r| r.drawn_level >= l)
            .unwrap_or(f.len() - fw);
        let d = fd as u64;
        if d > m.max_frames_to_detail {
            m.max_frames_to_detail = d;
            m.worst_detail_level = l;
        }
    }
    m.sim_seconds = f.len() as f64 * run.dt;
    if m.sim_seconds > 0.0 {
        m.uploads_per_s = m.total_uploads as f64 / m.sim_seconds;
        m.wanted_per_s = run.distinct_wanted as f64 / m.sim_seconds;
    }
    if run.distinct_wanted > 0 {
        m.throughput_ratio = m.total_uploads as f64 / run.distinct_wanted as f64;
    }
    debug_assert!(m.max_draws <= MAX_DRAWS_PER_FRAME || m.max_draws == 0);
    m
}

/// CSV header of [`csv`].
pub const CSV_HEADER: &str = "frame,sim_time_s,altitude_m,wanted_leaves,drawn_leaves,wanted_level,drawn_level,look_wanted_level,look_drawn_level,held,pending_requests,queued,started,uploads,in_flight,awaiting_upload,evicted,cancelled,requests_issued,draws,partition,converged";

/// CSV text of a run (header plus one row per frame).
#[must_use]
pub fn csv(run: &StreamRun) -> String {
    let mut s = String::from(CSV_HEADER);
    s.push('\n');
    for r in &run.frames {
        s.push_str(&format!(
            "{},{:.4},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            r.frame,
            r.sim_time_s,
            r.altitude_m,
            r.wanted_leaves,
            r.drawn_leaves,
            r.wanted_level,
            r.drawn_level,
            r.look_wanted_level,
            r.look_drawn_level,
            r.held_meshes,
            r.pending_requests,
            r.queued,
            r.started,
            r.uploads,
            r.in_flight,
            r.awaiting_upload,
            r.evicted,
            r.cancelled,
            r.requests_issued,
            r.draws,
            u8::from(r.partition),
            u8::from(r.converged),
        ));
    }
    s
}

/// Human-readable summary lines for console output.
#[must_use]
pub fn report_text(label: &str, m: &StreamMetrics) -> String {
    let mut s = format!("== {label} ==\n");
    s.push_str(&format!(
        "lag at camera: max {} p95 {}; frames-to-detail max {} (level {}); frames with lag>1: {:.1}%\n",
        m.max_lag,
        m.p95_lag,
        m.max_frames_to_detail,
        m.worst_detail_level,
        m.frac_lag_over_1 * 100.0
    ));
    s.push_str(&format!(
        "uploads {} in {:.1} sim s = {:.1}/s; distinct wanted leaves/s {:.1}; throughput ratio {:.2}\n",
        m.total_uploads, m.sim_seconds, m.uploads_per_s, m.wanted_per_s, m.throughput_ratio
    ));
    s.push_str(&format!(
        "max held {} pending_requests {} queued {} awaiting_upload {} started/frame {} uploads/frame {}; cancelled {} evicted {}; partition every frame after bootstrap ({} frames): {}; max draws {}; max wanted {}\n",
        m.max_held,
        m.max_pending_requests,
        m.max_queued,
        m.max_awaiting_upload,
        m.max_started,
        m.max_uploads,
        m.total_cancelled,
        m.total_evicted,
        m.bootstrap_frames,
        m.partition_every_frame,
        m.max_draws,
        m.max_wanted_leaves
    ));
    for (i, (label, _, _)) in ALTITUDE_BANDS.iter().enumerate() {
        let (mx, p95, n) = m.band_lag[i];
        s.push_str(&format!("  {label}: frames {n} lag max {mx} p95 {p95}\n"));
    }
    s
}
