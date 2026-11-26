//! Best-first, budgeted screen-space-error selection and balance closure over any [`Topology`].
//!
//! One pass, no tau escalation: the highest-error cell is refined first, traversal stops at a node
//! budget, and the piece count is capped by a hard budget, so the cost per frame is bounded and the
//! result is always a closed (balanced) split set.

use core::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};

use crate::lod::{
    HeightRange, LodModel, LodParams, SelectStats, View, cell_bound, cell_distance, eye_in_cone,
    frustum_visible, horizon_visible,
};
use crate::topology::{Piece, Topology};

/// Result of [`select_cells`].
#[derive(Clone, Debug, PartialEq)]
pub struct CellSelection<C> {
    pub split: BTreeSet<C>,
    pub pieces: Vec<Piece<C>>,
    pub stats: SelectStats,
}

/// Inserts every cell the closure rule requires until a fixpoint. Only inserts. Returns the number
/// of cells inserted.
pub fn close_balance<T: Topology>(topo: &T, split: &mut BTreeSet<T::Cell>) -> u32 {
    let mut work: VecDeque<T::Cell> = split.iter().copied().collect();
    let mut req = Vec::new();
    let mut forced = 0u32;
    while let Some(c) = work.pop_front() {
        req.clear();
        topo.required_splits(c, &mut req);
        for &r in &req {
            debug_assert!(
                topo.level(r) < topo.level(c),
                "required split {r:?} is not shallower than {c:?}"
            );
            if split.insert(r) {
                forced += 1;
                work.push_back(r);
            }
        }
    }
    forced
}

/// Height range of `id`: the nearest resident ancestor's measured range plus the residual of that
/// ancestor's level, or the global range when no ancestor is resident.
fn range_for<T: Topology>(
    topo: &T,
    model: &LodModel,
    id: T::Cell,
    resident_range: &dyn Fn(T::Cell) -> Option<HeightRange>,
) -> HeightRange {
    let mut cur = Some(id);
    while let Some(c) = cur {
        if let Some(r) = resident_range(c) {
            return model.derived_range(r, topo.level(c));
        }
        cur = topo.primary_parent(c);
    }
    model.height
}

/// Whether removing split `s` keeps `split` closed: no split child, and no split cell of the next
/// level touching a child of `s` from outside (the closure would require `s` again).
fn removable<T: Topology>(topo: &T, split: &BTreeSet<T::Cell>, s: T::Cell) -> bool {
    let mut children = Vec::new();
    topo.children(s, &mut children);
    if children.is_empty() || children.iter().any(|c| split.contains(c)) {
        return false;
    }
    let mut nbs = Vec::new();
    let mut parents = Vec::new();
    for &c in &children {
        nbs.clear();
        topo.neighbors(c, &mut nbs);
        for &nb in &nbs {
            if !split.contains(&nb) {
                continue;
            }
            parents.clear();
            topo.parents(nb, &mut parents);
            if !parents.contains(&s) {
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

fn piece_count<T: Topology>(
    topo: &T,
    split: &BTreeSet<T::Cell>,
    buf: &mut Vec<Piece<T::Cell>>,
) -> usize {
    buf.clear();
    topo.pieces(split, buf);
    buf.len()
}

/// Selects the split set for `view`.
///
/// A node splits when `rho > tau`, or, if it was split in `prev_split`, while
/// `rho > tau * (1 - hysteresis)`. Cells that cannot be seen (beyond the horizon, outside the
/// frustum) are not refined. `resident_range` returns the measured height range of a resident
/// cell. The split set is closed under `required_splits` and has at most `max_leaves` pieces.
#[must_use]
pub fn select_cells<T: Topology>(
    topo: &T,
    model: &LodModel,
    params: &LodParams,
    view: &View,
    prev_split: &BTreeSet<T::Cell>,
    resident_range: &dyn Fn(T::Cell) -> Option<HeightRange>,
) -> CellSelection<T::Cell> {
    let mut stats = SelectStats {
        tau_eff: params.tau_px,
        ..SelectStats::default()
    };
    let occluder = model.radius + model.height.min;
    let low = params.tau_px * (1.0 - params.hysteresis);
    let mut split: BTreeSet<T::Cell> = BTreeSet::new();
    let mut heap: BinaryHeap<(u64, Reverse<u64>)> = BinaryHeap::new();

    // Evaluates a cell: `Some(rho)` when it is a refinement candidate, `None` when culled.
    let eval = |id: T::Cell, stats: &mut SelectStats| -> Option<f64> {
        stats.nodes_visited = stats.nodes_visited.saturating_add(1);
        let range = range_for(topo, model, id, resident_range);
        let fp = topo.footprint(id);
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
        Some(model.geometric_error(topo.level(id)) * view.proj_scale / d)
    };

    if params.max_level > 0 {
        for &id in topo.roots() {
            if stats.nodes_visited >= params.max_nodes_visited {
                stats.node_budget_hit = true;
                break;
            }
            if let Some(rho) = eval(id, &mut stats) {
                heap.push((key_of(rho), Reverse(topo.raw(id))));
            }
        }
    }

    // Piece estimate: each split replaces one piece by its children.
    let mut estimate = topo.roots().len();
    let mut children: Vec<T::Cell> = Vec::new();
    while let Some((bits, Reverse(raw))) = heap.pop() {
        let rho = f64::from_bits(bits);
        if rho <= low {
            // Best-first: every remaining candidate is below the lowest threshold.
            break;
        }
        let Some(id) = topo.from_raw(raw) else {
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
        children.clear();
        topo.children(id, &mut children);
        let Some(&first) = children.first() else {
            continue;
        };
        let gain = children.len() - 1;
        if estimate + gain > params.max_leaves {
            stats.leaf_budget_hit = true;
            continue;
        }
        let grandchildren = topo.level(first) < params.max_level;
        let need = u32::try_from(children.len()).unwrap_or(u32::MAX);
        if grandchildren && stats.nodes_visited.saturating_add(need) > params.max_nodes_visited {
            stats.node_budget_hit = true;
            break;
        }
        split.insert(id);
        estimate += gain;
        if grandchildren {
            for &c in &children {
                if let Some(r) = eval(c, &mut stats) {
                    heap.push((key_of(r), Reverse(topo.raw(c))));
                }
            }
        }
    }

    stats.forced_splits = close_balance(topo, &mut split);

    // Trim to the hard piece budget by removing the lowest-error split that is safe to remove.
    // Splits at the deepest level are always removable, so this terminates at the roots.
    let mut pieces: Vec<Piece<T::Cell>> = Vec::new();
    if piece_count(topo, &split, &mut pieces) > params.max_leaves {
        stats.leaf_budget_hit = true;
        let mut rhos: BTreeMap<T::Cell, u64> = BTreeMap::new();
        for &s in &split {
            let range = range_for(topo, model, s, resident_range);
            let fp = topo.footprint(s);
            let d = cell_distance(model.radius, view.eye, &fp, range);
            let d = if d > params.min_distance {
                d
            } else {
                params.min_distance
            };
            let rho = model.geometric_error(topo.level(s)) * view.proj_scale / d;
            rhos.insert(s, key_of(rho));
        }
        while piece_count(topo, &split, &mut pieces) > params.max_leaves {
            let mut victim: Option<(u64, T::Cell)> = None;
            for (s, key) in &rhos {
                if victim.is_some_and(|v| (*key, *s) >= v) || !removable(topo, &split, *s) {
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
    debug_assert_eq!(
        close_balance(topo, &mut split.clone()),
        0,
        "split not closed"
    );

    piece_count(topo, &split, &mut pieces);
    stats.leaves = pieces.len();
    CellSelection {
        split,
        pieces,
        stats,
    }
}
