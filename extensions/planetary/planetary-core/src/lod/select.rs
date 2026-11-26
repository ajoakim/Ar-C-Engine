//! Best-first, budgeted screen-space-error selection with culling and hysteresis.
//!
//! One pass, no tau escalation: the highest-error cell is refined first, traversal stops at a node
//! budget, and the split count is capped by a hard leaf budget, so the cost per frame is bounded
//! and the result is always a balanced partition.

use std::collections::BTreeSet;

use crate::lod::cull::{HeightRange, cell_distance, patch_footprint};
use crate::lod::model::{LodModel, View};
use crate::patch_id::PatchId;
use crate::topology::cube::CubeSphere;
use crate::topology::select::select_cells;

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
    /// Equal to `tau_px` (kept for telemetry continuity).
    pub tau_eff: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    pub split: BTreeSet<PatchId>,
    pub leaves: Vec<PatchId>,
    pub stats: SelectStats,
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

/// Selects the split set for `view`.
///
/// Cube-sphere convenience wrapper; removed in 07.1-16 once planetary-terrain uses `select_cells`.
///
/// A node splits when `rho > tau`, or, if it was split in `prev_split`, while
/// `rho > tau * (1 - hysteresis)`. Cells that cannot be seen (beyond the horizon, outside the
/// frustum) are not refined. `resident_range` returns the measured height range of a resident
/// patch. The result is balanced (2:1), a partition, and has at most `max_leaves` leaves.
#[must_use]
pub fn select(
    model: &LodModel,
    params: &LodParams,
    view: &View,
    prev_split: &BTreeSet<PatchId>,
    resident_range: &dyn Fn(PatchId) -> Option<HeightRange>,
) -> Selection {
    let topo = CubeSphere::new(model.cells_per_edge);
    let sel = select_cells(&topo, model, params, view, prev_split, resident_range);
    let leaves = sel.pieces.iter().map(|p| p.cell).collect();
    Selection {
        split: sel.split,
        leaves,
        stats: sel.stats,
    }
}
