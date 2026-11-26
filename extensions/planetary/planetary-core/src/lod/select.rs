//! Best-first, budgeted screen-space-error selection with culling and hysteresis.
//!
//! One pass, no tau escalation: the highest-error cell is refined first, traversal stops at a node
//! budget, and the split count is capped by a hard leaf budget, so the cost per frame is bounded
//! and the result is always a balanced partition.

use core::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use crate::adjacency::neighbor;
use crate::face::{Edge, Face};
use crate::lod::balance::{balance, leaves_of};
use crate::lod::cull::{
    HeightRange, cell_bound, cell_distance, eye_in_cone, frustum_visible, horizon_visible,
    patch_footprint,
};
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

/// Height range of `id`: the nearest resident ancestor's measured range plus the residual of that
/// ancestor's level, or the global range when no ancestor is resident.
fn range_for(
    model: &LodModel,
    id: PatchId,
    resident_range: &dyn Fn(PatchId) -> Option<HeightRange>,
) -> HeightRange {
    let mut cur = Some(id);
    while let Some(c) = cur {
        if let Some(r) = resident_range(c) {
            return model.derived_range(r, c.level());
        }
        cur = c.parent();
    }
    model.height
}

/// Whether removing split `s` keeps `split` closed and 2:1 balanced (see `select`).
fn removable(split: &BTreeSet<PatchId>, s: PatchId) -> bool {
    let Some(children) = s.children() else {
        return false;
    };
    if children.iter().any(|c| split.contains(c)) {
        return false;
    }
    for c in children {
        for e in Edge::ALL {
            let nb = neighbor(c, e).patch;
            if nb.parent() != Some(s) && split.contains(&nb) {
                return false;
            }
        }
    }
    true
}

/// Finite, non-negative key whose bit order equals its numeric order.
fn key_of(rho: f64) -> u64 {
    let r = if rho.is_nan() || rho < 0.0 {
        0.0
    } else if rho > f64::MAX {
        f64::MAX
    } else {
        rho
    };
    r.to_bits()
}

/// Selects the split set for `view`.
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
    let mut stats = SelectStats {
        tau_eff: params.tau_px,
        ..SelectStats::default()
    };
    let occluder = model.radius + model.height.min;
    let low = params.tau_px * (1.0 - params.hysteresis);
    let mut split: BTreeSet<PatchId> = BTreeSet::new();
    let mut heap: BinaryHeap<(u64, Reverse<u64>)> = BinaryHeap::new();

    // Evaluates a cell: `Some(rho)` when it is a refinement candidate, `None` when culled.
    let eval = |id: PatchId, stats: &mut SelectStats| -> Option<f64> {
        stats.nodes_visited = stats.nodes_visited.saturating_add(1);
        let range = range_for(model, id, resident_range);
        let fp = patch_footprint(id);
        if !horizon_visible(model.radius, view.eye, &fp, range, occluder) {
            stats.culled_horizon += 1;
            return None;
        }
        // The cell under the camera is never culled by the frustum.
        if !eye_in_cone(view.eye, &fp)
            && !frustum_visible(view, &cell_bound(model.radius, &fp, range))
        {
            stats.culled_frustum += 1;
            return None;
        }
        let d = cell_distance(model.radius, view.eye, &fp, range);
        let d = if d > params.min_distance {
            d
        } else {
            params.min_distance
        };
        Some(model.geometric_error(id.level()) * view.proj_scale / d)
    };

    if params.max_level > 0 {
        for f in Face::ALL {
            let id = PatchId::root(f);
            if stats.nodes_visited >= params.max_nodes_visited {
                stats.node_budget_hit = true;
                break;
            }
            if let Some(rho) = eval(id, &mut stats) {
                heap.push((key_of(rho), Reverse(id.raw())));
            }
        }
    }

    while let Some((bits, Reverse(raw))) = heap.pop() {
        let rho = f64::from_bits(bits);
        if rho <= low {
            // Best-first: every remaining candidate is below the lowest threshold.
            break;
        }
        let Ok(id) = PatchId::from_raw(raw) else {
            continue;
        };
        let thresh = if prev_split.contains(&id) {
            low
        } else {
            params.tau_px
        };
        if rho <= thresh {
            continue;
        }
        if 6 + 3 * split.len() + 3 > params.max_leaves {
            stats.leaf_budget_hit = true;
            continue;
        }
        let Some(children) = id.children() else {
            continue;
        };
        let grandchildren = children[0].level() < params.max_level;
        if grandchildren && stats.nodes_visited.saturating_add(4) > params.max_nodes_visited {
            stats.node_budget_hit = true;
            break;
        }
        split.insert(id);
        if grandchildren {
            for c in children {
                if let Some(r) = eval(c, &mut stats) {
                    heap.push((key_of(r), Reverse(c.raw())));
                }
            }
        }
    }

    stats.forced_splits = balance(&mut split);

    // Trim to the hard leaf budget by removing the lowest-error split that is safe to remove.
    // A split `s` is removable when it has no split child (set stays closed) and no split cell of
    // level `s.level + 1` touches a child of `s` from outside (balance requires `s` then).
    // The deepest-level splits are always removable, so this terminates at the 6 roots.
    if 6 + 3 * split.len() > params.max_leaves {
        stats.leaf_budget_hit = true;
        let mut rhos: BTreeMap<PatchId, u64> = BTreeMap::new();
        for s in &split {
            let range = range_for(model, *s, resident_range);
            let rho = screen_space_error(model, view, *s, range, params.min_distance);
            rhos.insert(*s, key_of(rho));
        }
        while 6 + 3 * split.len() > params.max_leaves {
            let mut victim: Option<(u64, PatchId)> = None;
            for (s, key) in &rhos {
                if victim.is_some_and(|v| (*key, *s) >= v) || !removable(&split, *s) {
                    continue;
                }
                victim = Some((*key, *s));
            }
            let Some((_, id)) = victim else {
                break;
            };
            split.remove(&id);
            rhos.remove(&id);
            stats.trimmed_splits += 1;
        }
    }

    let leaves = leaves_of(&split);
    stats.leaves = leaves.len();
    Selection {
        split,
        leaves,
        stats,
    }
}
