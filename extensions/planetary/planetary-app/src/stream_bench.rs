//! Windowed-realistic streaming harness (07.1-19).
//!
//! Runs the real [`PlanetaryApp`] on the null backend with `settle_each_frame: false` (workers
//! run freely), a SIMULATED frame time (the script advances by `dt` per frame, never by wall
//! clock) and records, per frame, the wanted and drawn level at the camera. This module reads no
//! clock. Pacing is injected: the example passes a closure that spins on the telemetry wall
//! clock so workers get realistic real time per frame; tests pass a counted yield loop.

use std::collections::BTreeSet;

use crate::telemetry::{ALTITUDE_BANDS, band_of};
use crate::{AppSettings, Clock, PlanetaryApp};
use engine_sdk::{Engine, EngineConfig, MAX_DRAWS_PER_FRAME, NullBackend, Runtime, SdkError};

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
    pub closure_cells: usize,
    pub closure_resident: usize,
    pub draws: usize,
    pub draw_items: usize,
    pub partition: bool,
    /// Measured prepare plus gpu_frame cost (ms) of the frame (telemetry clock).
    pub cost_ms: f64,
    /// Prepare time (ms) per scope (`Scope::index` order) and in total (last element).
    pub prep_ms: [f64; 7],
    /// The selection was reused this frame (07.1-27).
    pub reselect_skipped: bool,
    /// Adaptive budget caps in force (uploads, starts, requests).
    pub caps: (u32, u32, u32),
    /// True when drawn equals wanted exactly (converged).
    pub converged: bool,
    /// Churn counters (zero unless diagnostics are on): level_changes, appeared, disappeared,
    /// cull_toggles, overlap_pairs, flipflops, flipflops_unexplained.
    pub churn: [u32; 7],
    /// Select flags of the frame: trimmed_splits, node_budget_hit, leaf_budget_hit, coarsened.
    pub sel: [u32; 4],
    pub destroyed: u32,
    /// Selection retained / rejected culled cells (see `SelectStats`).
    pub retained: [u32; 3],
    /// Why cells split last frame are not split now: total, culled, below band, in band or above, cascade.
    pub unsplit: [u32; 6],
    /// Culled-by-selection cells that the draw pass drew visibly (selection/draw cull mismatch).
    pub unsplit_culled_drawn_visible: u32,
}

/// A whole run.
#[derive(Clone, Debug, Default)]
pub struct StreamRun {
    pub frames: Vec<StreamFrame>,
    /// Distinct wanted leaves seen over the run (demand).
    pub distinct_wanted: usize,
    pub dt: f64,
    /// Frames of the scripted descent (the hover starts here); set by the caller.
    pub descent_frames: usize,
}

/// Runs `frames` frames with a simulated step `dt` seconds each; `pace` is called after every
/// frame (it may spin so workers progress, or call [`PlanetaryApp::wait_for_workers`]).
///
/// # Errors
/// Propagates app and runtime errors.
#[allow(clippy::result_large_err)]
pub fn run_stream(
    settings: AppSettings,
    clock: Box<dyn Clock>,
    frames: u64,
    dt: f64,
    pace: &mut dyn FnMut(&mut PlanetaryApp),
) -> Result<StreamRun, SdkError> {
    let mut app = PlanetaryApp::with_clock(settings, clock)?;
    let mut rt = Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new())?;
    rt.setup(&mut app)?;
    let mut out = Vec::new();
    let mut seen: BTreeSet<u64> = BTreeSet::new();
    for _ in 0..frames {
        rt.frame(&mut app, dt)?;
        let rep = app.report().clone();
        let drawn = app.drawn_raw();
        let wanted = app.wanted_raw();
        for p in &wanted {
            seen.insert(*p);
        }
        let pos = app.camera().position();
        let nadir = pos.normalize();
        let ahead = (pos + app.camera().forward() * 10.0).normalize();
        let draws = rt
            .backend()
            .last_frame()
            .map_or(0, |f| f.passes.iter().map(|p| p.draws.len()).sum());
        let altitude_m = app.camera().altitude(&|d| app.surface_radius_at(d));
        let converged = !drawn.is_empty() && {
            let (mut a, mut b) = (drawn.clone(), wanted.clone());
            a.sort_unstable();
            b.sort_unstable();
            a == b
        };
        out.push(StreamFrame {
            frame: rep.frame,
            sim_time_s: app.simulated_time(),
            altitude_m,
            wanted_leaves: wanted.len(),
            drawn_leaves: drawn.len(),
            wanted_level: app.wanted_level_at(nadir).map_or(-1, i32::from),
            drawn_level: app.piece_level_at(nadir).map_or(-1, i32::from),
            look_wanted_level: app.wanted_level_at(ahead).map_or(-1, i32::from),
            look_drawn_level: app.piece_level_at(ahead).map_or(-1, i32::from),
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
            closure_cells: rep.closure_cells,
            closure_resident: rep.closure_resident,
            draws,
            draw_items: rep.draw_items,
            partition: drawn.is_empty() || app.drawn_is_partition(),
            reselect_skipped: rep.reselect_skipped,
            prep_ms: app.telemetry().last().map_or([0.0; 7], |t| {
                let mut o = [0.0; 7];
                o[..6].copy_from_slice(&t.prep_scope_ms);
                o[6] = t.prepare_ms;
                o
            }),
            cost_ms: app
                .telemetry()
                .last()
                .map_or(0.0, |t| t.prepare_ms + t.gpu_frame_ms),
            caps: {
                let b = app.stream_budget();
                (b.uploads, b.starts, b.requests)
            },
            converged,
            churn: [
                rep.level_changes,
                rep.drawn_appeared,
                rep.drawn_disappeared,
                rep.cull_toggles,
                rep.overlap_pairs,
                rep.flipflops,
                rep.flipflops_unexplained,
            ],
            sel: [
                rep.select.trimmed_splits,
                u32::from(rep.select.node_budget_hit),
                u32::from(rep.select.leaf_budget_hit),
                rep.effective_coarsened,
            ],
            destroyed: rep.destroyed,
            retained: [
                rep.select.retained_culled,
                rep.select.retain_rejected,
                rep.retain_expired,
            ],
            unsplit: rep.unsplit,
            unsplit_culled_drawn_visible: rep.unsplit_culled_drawn_visible,
        });
        pace(&mut app);
    }
    let distinct_wanted = seen.len();
    app.shutdown();
    rt.shutdown()?;
    Ok(StreamRun {
        frames: out,
        distinct_wanted,
        dt,
        descent_frames: 0,
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
    /// Per altitude band: (average, p95) prepare ms per scope and in total (last element).
    pub band_prep: Vec<[(f64, f64); 7]>,
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
    /// Frames after the descent ends until drawn equals wanted (`None`: never within the run).
    pub frames_to_converge: Option<usize>,
    /// Level of the drawn leaf at the look-at point on the last frame (-1 if none).
    pub final_look_drawn_level: i32,
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
        band_prep: vec![[(0.0, 0.0); 7]; ALTITUDE_BANDS.len()],
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
    for (b, row) in m.band_prep.iter_mut().enumerate() {
        let sel: Vec<&StreamFrame> = f
            .iter()
            .skip(WARM_UP)
            .filter(|r| band_of(r.altitude_m) == b)
            .collect();
        for (k, slot) in row.iter_mut().enumerate() {
            let mut v: Vec<f64> = sel.iter().map(|r| r.prep_ms[k]).collect();
            if v.is_empty() {
                continue;
            }
            v.sort_by(f64::total_cmp);
            let avg = v.iter().sum::<f64>() / v.len() as f64;
            let rank = (v.len() * 95).div_ceil(100).max(1);
            *slot = (avg, v[rank - 1]);
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
    m.frames_to_converge = f
        .iter()
        .enumerate()
        .skip(run.descent_frames)
        .find(|(_, r)| r.converged)
        .map(|(i, _)| i - run.descent_frames);
    m.final_look_drawn_level = f.last().map_or(-1, |r| r.look_drawn_level);
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
pub const CSV_HEADER: &str = "frame,sim_time_s,altitude_m,wanted_leaves,drawn_leaves,wanted_level,drawn_level,look_wanted_level,look_drawn_level,held,pending_requests,queued,started,uploads,in_flight,awaiting_upload,evicted,cancelled,requests_issued,closure_cells,closure_resident,draws,partition,cost_ms,cap_uploads,cap_starts,cap_requests,converged,level_changes,drawn_appeared,drawn_disappeared,cull_toggles,overlap_pairs,flipflops,flipflops_unexplained,trimmed_splits,node_budget_hit,leaf_budget_hit,coarsened,destroyed,retained_culled,retain_rejected,retain_expired,unsplit_total,unsplit_frustum,unsplit_horizon,unsplit_below_band,unsplit_in_band_or_above,unsplit_cascade,unsplit_culled_drawn_visible";

/// CSV text of a run (header plus one row per frame).
#[must_use]
pub fn csv(run: &StreamRun) -> String {
    let mut s = String::from(CSV_HEADER);
    s.push('\n');
    for r in &run.frames {
        let cols: Vec<String> = vec![
            r.frame.to_string(),
            format!("{:.4}", r.sim_time_s),
            format!("{:.3}", r.altitude_m),
            r.wanted_leaves.to_string(),
            r.drawn_leaves.to_string(),
            r.wanted_level.to_string(),
            r.drawn_level.to_string(),
            r.look_wanted_level.to_string(),
            r.look_drawn_level.to_string(),
            r.held_meshes.to_string(),
            r.pending_requests.to_string(),
            r.queued.to_string(),
            r.started.to_string(),
            r.uploads.to_string(),
            r.in_flight.to_string(),
            r.awaiting_upload.to_string(),
            r.evicted.to_string(),
            r.cancelled.to_string(),
            r.requests_issued.to_string(),
            r.closure_cells.to_string(),
            r.closure_resident.to_string(),
            r.draws.to_string(),
            u8::from(r.partition).to_string(),
            format!("{:.3}", r.cost_ms),
            r.caps.0.to_string(),
            r.caps.1.to_string(),
            r.caps.2.to_string(),
            u8::from(r.converged).to_string(),
        ]
        .into_iter()
        .chain(r.churn.iter().map(ToString::to_string))
        .chain(r.sel.iter().map(ToString::to_string))
        .chain(std::iter::once(r.destroyed.to_string()))
        .chain(r.retained.iter().map(ToString::to_string))
        .chain(r.unsplit.iter().map(ToString::to_string))
        .chain(std::iter::once(r.unsplit_culled_drawn_visible.to_string()))
        .collect();
        s.push_str(&cols.join(","));
        s.push('\n');
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
    s.push_str(&format!(
        "frames from descent end to drawn == wanted: {}; final look-at drawn level {}
",
        m.frames_to_converge
            .map_or_else(|| "never".to_string(), |v| v.to_string()),
        m.final_look_drawn_level
    ));
    for (i, (label, _, _)) in ALTITUDE_BANDS.iter().enumerate() {
        let (mx, p95, n) = m.band_lag[i];
        s.push_str(&format!("  {label}: frames {n} lag max {mx} p95 {p95}\n"));
    }
    s.push_str(
        "prepare ms per band, avg/p95: total | select closure score cull setbuild requests
",
    );
    for (i, (label, _, _)) in ALTITUDE_BANDS.iter().enumerate() {
        let r = &m.band_prep[i];
        s.push_str(&format!(
            "  {label}: {:.2}/{:.2} | {:.2}/{:.2} {:.2}/{:.2} {:.2}/{:.2} {:.2}/{:.2} {:.2}/{:.2} {:.2}/{:.2}
",
            r[6].0, r[6].1, r[0].0, r[0].1, r[1].0, r[1].1, r[2].0, r[2].1, r[3].0, r[3].1,
            r[4].0, r[4].1, r[5].0, r[5].1
        ));
    }
    s
}
