//! Selection parameters, statistics and the screen-space error of a cell. The selection itself is
//! generic over the topology: see `topology::select::select_cells`.

use crate::lod::cull::{HeightRange, cell_distance, patch_footprint};
use crate::lod::model::{LodModel, View};
use crate::patch_id::PatchId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodParams {
    pub tau_px: f64,
    pub hysteresis: f64,
    pub max_level: u8,
    /// Hard cap on the number of leaves (draws).
    pub max_leaves: usize,
    /// Cap on cells evaluated per call.
    pub max_nodes_visited: u32,
    pub min_distance: f64,
}

impl Default for LodParams {
    fn default() -> Self {
        Self {
            tau_px: 2.0,
            hysteresis: 0.2,
            max_level: 16,
            max_leaves: 900,
            max_nodes_visited: 8192,
            min_distance: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SelectStats {
    pub nodes_visited: u32,
    pub culled_horizon: u32,
    pub culled_frustum: u32,
    pub leaves: usize,
    pub forced_splits: u32,
    pub trimmed_splits: u32,
    pub node_budget_hit: bool,
    pub leaf_budget_hit: bool,
    /// Previously split cells kept split although culled this frame (set by the caller of
    /// `topology::select::retain_culled`, never by `select_cells`).
    pub retained_culled: u32,
    /// Cells that would have been retained but were dropped because they did not fit the piece
    /// budget.
    pub retain_rejected: u32,
    /// Equal to `tau_px` (kept for telemetry continuity).
    pub tau_eff: f64,
}

/// `eps(L) * proj_scale / max(distance to the extruded footprint, min_distance)`.
#[must_use]
pub fn screen_space_error(
    model: &LodModel,
    view: &View,
    id: PatchId,
    range: HeightRange,
    min_distance: f64,
) -> f64 {
    let d = cell_distance(model.radius, view.eye, &patch_footprint(id), range);
    let d = if d > min_distance { d } else { min_distance };
    model.geometric_error(id.level()) * view.proj_scale / d
}

/// Distance below which a cell of `level` is refined: the selector splits it when
/// `geometric_error(level) * proj_scale / d > tau_px`, i.e. `d < refine_distance(..)`
/// (ADR-0012). The single source of the per-level threshold used by the geomorph factor.
/// Returns `f64::INFINITY` when `tau_px <= 0` or not finite.
#[must_use]
pub fn refine_distance(model: &LodModel, proj_scale: f64, tau_px: f64, level: u8) -> f64 {
    if !(tau_px > 0.0 && tau_px.is_finite()) {
        return f64::INFINITY;
    }
    model.geometric_error(level) * proj_scale / tau_px
}

/// Geomorph factor: 1 at `d >= d_start`, 0 at `d <= d_end`, linear in `d` between. Returns 0.0
/// when `d_start <= d_end` or any input is not finite (never NaN).
#[must_use]
pub fn morph_factor(d: f64, d_start: f64, d_end: f64) -> f64 {
    if !(d.is_finite() && d_start.is_finite() && d_end.is_finite()) || d_start <= d_end {
        return 0.0;
    }
    if d >= d_start {
        1.0
    } else if d <= d_end {
        0.0
    } else {
        (d - d_end) / (d_start - d_end)
    }
}
