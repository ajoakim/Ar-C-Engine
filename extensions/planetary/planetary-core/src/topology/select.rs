//! Best-first, budgeted screen-space-error selection and balance closure over any [`Topology`].
//!
//! One pass, no tau escalation: the highest-error cell is refined first, traversal stops at a node
//! budget, and the piece count is capped by a hard budget, so the cost per frame is bounded and the
//! result is always a closed (balanced) split set.

use core::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};

use crate::lod::{
    HeightRange, LodModel, LodParams, SelectSlack, SelectStats, View, cell_bound, cell_distance,
    cone_margin_m, eye_in_cone, frustum_margin, frustum_visible, horizon_eye_cap, horizon_margin_m,
    horizon_visible, sse_margin_m,
};
use crate::topology::{Piece, Topology};

/// Result of [`select_cells`].
#[derive(Clone, Debug, PartialEq)]
pub struct CellSelection<C> {
    pub split: BTreeSet<C>,
    pub pieces: Vec<Piece<C>>,
    pub stats: SelectStats,
    /// How far the view may change before this selection could change (07.1-27).
    pub slack: SelectSlack,
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

/// Memo of the nearest resident ancestor-or-self per cell for one selection (07.1-27). The
/// resident ranges do not change during a call, so a memoised answer is the answer the plain
/// parent walk of [`range_for`] gives; it only avoids walking the same chain for every cell.
struct RangeMemo<C> {
    map: BTreeMap<C, Option<(HeightRange, u8)>>,
    path: Vec<C>,
}

impl<C: Copy + Ord> RangeMemo<C> {
    fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            path: Vec::new(),
        }
    }

    /// Same value as `range_for(topo, model, id, resident_range)`.
    fn range<T: Topology<Cell = C>>(
        &mut self,
        topo: &T,
        model: &LodModel,
        id: C,
        resident_range: &dyn Fn(C) -> Option<HeightRange>,
    ) -> HeightRange {
        self.path.clear();
        let mut found = None;
        let mut cur = Some(id);
        while let Some(c) = cur {
            if let Some(m) = self.map.get(&c) {
                found = *m;
                break;
            }
            if let Some(r) = resident_range(c) {
                found = Some((r, topo.level(c)));
                self.map.insert(c, found);
                break;
            }
            self.path.push(c);
            cur = topo.primary_parent(c);
        }
        for &c in &self.path {
            self.map.insert(c, found);
        }
        match found {
            Some((r, level)) => model.derived_range(r, level),
            None => model.height,
        }
    }
}

/// Why a cell cannot be seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cull {
    Horizon,
    Frustum,
}

/// Whether `id` cannot be seen: beyond the horizon, or outside the frustum while not under the eye.
fn cell_culled<T: Topology>(
    topo: &T,
    model: &LodModel,
    view: &View,
    id: T::Cell,
    range: HeightRange,
    occluder: f64,
) -> Option<Cull> {
    let fp = topo.footprint(id);
    if !horizon_visible(model.radius, view.eye, &fp, range, occluder) {
        Some(Cull::Horizon)
    } else if !eye_in_cone(view.eye, &fp)
        && !frustum_visible(view, &cell_bound(model.radius, &fp, range))
    {
        Some(Cull::Frustum)
    } else {
        None
    }
}

/// Whether removing split `s` keeps `split` closed: no split child, and no split cell of the next
/// level that requires `s`. Such a cell is a child of `s` or a child of a neighbour of `s`, because
/// `required_splits` of a cell lists its parents and cells next to them.
fn removable<T: Topology>(topo: &T, split: &BTreeSet<T::Cell>, s: T::Cell) -> bool {
    let mut children = Vec::new();
    topo.children(s, &mut children);
    if children.is_empty() || children.iter().any(|c| split.contains(c)) {
        return false;
    }
    let mut nbs = Vec::new();
    topo.neighbors(s, &mut nbs);
    let mut cand = Vec::new();
    let mut req = Vec::new();
    for &n in &nbs {
        cand.clear();
        topo.children(n, &mut cand);
        for &x in &cand {
            if !split.contains(&x) {
                continue;
            }
            req.clear();
            topo.required_splits(x, &mut req);
            if req.contains(&s) {
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

/// Why cells split in the previous selection are not split in this one (diagnostics, 07.1-20).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnsplitReasons {
    /// Cells in `prev_split` and not in the new split.
    pub total: u32,
    /// Cell outside the frustum this frame (not refined while culled).
    pub culled: u32,
    /// Cell beyond the horizon this frame.
    pub culled_horizon: u32,
    /// Not culled, parent still split, and `rho <= tau * (1 - hysteresis)`: a legitimate drop.
    pub below_band: u32,
    /// Not culled, `rho` inside the hysteresis band or above tau, parent split, yet unsplit:
    /// budgets, trimming or a stop of the best-first loop.
    pub within_band_or_above: u32,
    /// The parent (or one of the parents) is no longer split: a cascade.
    pub cascade: u32,
}

/// Classifies every cell of `prev_split` that is not in `split` (see [`UnsplitReasons`]) and
/// returns the ones dropped because they are culled. Pure
/// diagnostics: it repeats the cull and error evaluation of [`select_cells`] and changes nothing.
#[must_use]
pub fn unsplit_reasons<T: Topology>(
    topo: &T,
    model: &LodModel,
    params: &LodParams,
    view: &View,
    prev_split: &BTreeSet<T::Cell>,
    split: &BTreeSet<T::Cell>,
    resident_range: &dyn Fn(T::Cell) -> Option<HeightRange>,
) -> (UnsplitReasons, Vec<T::Cell>) {
    let occluder = model.radius + model.height.min;
    let low = params.tau_px * (1.0 - params.hysteresis);
    let mut out = UnsplitReasons::default();
    let mut culled_cells = Vec::new();
    let mut ps = Vec::new();
    for &c in prev_split.difference(split) {
        out.total += 1;
        ps.clear();
        topo.parents(c, &mut ps);
        if ps.iter().any(|p| !split.contains(p)) {
            out.cascade += 1;
            continue;
        }
        let range = range_for(topo, model, c, resident_range);
        match cell_culled(topo, model, view, c, range, occluder) {
            Some(Cull::Frustum) => {
                out.culled += 1;
                culled_cells.push(c);
                continue;
            }
            Some(Cull::Horizon) => {
                out.culled_horizon += 1;
                culled_cells.push(c);
                continue;
            }
            None => {}
        }
        let fp = topo.footprint(c);
        let d = cell_distance(model.radius, view.eye, &fp, range);
        let d = if d > params.min_distance {
            d
        } else {
            params.min_distance
        };
        let rho = model.geometric_error(topo.level(c)) * view.proj_scale / d;
        if rho <= low {
            out.below_band += 1;
        } else {
            out.within_band_or_above += 1;
        }
    }
    (out, culled_cells)
}

/// Result of [`retain_culled`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Retained<C> {
    /// Cells added to the split set (culled last-frame splits that were kept), ascending
    /// (level, raw). The closure forced by them is in the split set too but not listed.
    pub cells: Vec<C>,
    /// Candidates that did not fit the piece budget.
    pub rejected: u32,
    /// The pieces of the returned split set when `cells` is not empty (07.1-27: the verification
    /// of the retained set already computed them), else `None`.
    pub pieces: Option<Vec<Piece<C>>>,
}

/// Keeps cells of `candidates` (the previous split set) that are culled this frame (outside the
/// frustum or beyond the horizon) split, after [`select_cells`] dropped them (07.1-20).
///
/// A culled cell is never refined by the best-first loop, so without this a cell leaving the view
/// would drop out of the split set and, when it returned a few frames later, be split again: its
/// children disappear and reappear. `select_cells` itself is unchanged; the caller owns the
/// retention age (it passes only candidates that may still be retained). Cells are retained in
/// ascending (level, raw) order, only while all their parents are split, so the set stays closed
/// upwards, and only within the piece budget that the visible selection left over (retaining
/// never costs visible detail). The prefix that fits is found by a per-split estimate and
/// verified exactly; if it does not fit it is halved. `split` must be closed on entry (as
/// returned by `select_cells`) and is closed on exit.
pub fn retain_culled<T: Topology>(
    topo: &T,
    model: &LodModel,
    params: &LodParams,
    view: &View,
    candidates: &BTreeSet<T::Cell>,
    resident_range: &dyn Fn(T::Cell) -> Option<HeightRange>,
    split: &mut BTreeSet<T::Cell>,
) -> Retained<T::Cell> {
    retain_culled_known(
        topo,
        model,
        params,
        view,
        candidates,
        resident_range,
        split,
        None,
    )
}

/// [`retain_culled`] when the caller already knows the piece count of `split` (`base`, for
/// example `CellSelection::pieces.len()`): it is not recomputed. `None` computes it.
#[allow(clippy::too_many_arguments)]
pub fn retain_culled_known<T: Topology>(
    topo: &T,
    model: &LodModel,
    params: &LodParams,
    view: &View,
    candidates: &BTreeSet<T::Cell>,
    resident_range: &dyn Fn(T::Cell) -> Option<HeightRange>,
    split: &mut BTreeSet<T::Cell>,
    base: Option<usize>,
) -> Retained<T::Cell> {
    let mut cand: Vec<T::Cell> = candidates
        .iter()
        .copied()
        .filter(|c| !split.contains(c))
        .collect();
    if cand.is_empty() {
        return Retained {
            cells: Vec::new(),
            rejected: 0,
            pieces: None,
        };
    }
    cand.sort_by_key(|c| (topo.level(*c), topo.raw(*c)));
    let occluder = model.radius + model.height.min;
    // Culled candidates whose parents are all split (including earlier retained ones).
    let mut chain = split.clone();
    let mut kept: Vec<T::Cell> = Vec::new();
    let mut ps = Vec::new();
    for c in cand {
        ps.clear();
        topo.parents(c, &mut ps);
        if ps.iter().any(|p| !chain.contains(p)) {
            continue;
        }
        let range = range_for(topo, model, c, resident_range);
        if cell_culled(topo, model, view, c, range, occluder).is_some() {
            chain.insert(c);
            kept.push(c);
        }
    }
    if kept.is_empty() {
        return Retained {
            cells: Vec::new(),
            rejected: 0,
            pieces: None,
        };
    }
    let mut buf: Vec<Piece<T::Cell>> = Vec::new();
    let base = base.unwrap_or_else(|| piece_count(topo, split, &mut buf));
    let room = params.max_leaves.saturating_sub(base);
    // Per-split estimate: a split replaces one piece by its children.
    let mut take = 0usize;
    let mut est = 0usize;
    let mut kids: Vec<T::Cell> = Vec::new();
    for &c in &kept {
        kids.clear();
        topo.children(c, &mut kids);
        let add = kids.len().saturating_sub(1);
        if est + add > room {
            break;
        }
        est += add;
        take += 1;
    }
    while take > 0 {
        let mut trial = split.clone();
        trial.extend(kept[..take].iter().copied());
        close_balance(topo, &mut trial);
        if piece_count(topo, &trial, &mut buf) <= params.max_leaves {
            let rejected = u32::try_from(kept.len() - take).unwrap_or(u32::MAX);
            kept.truncate(take);
            *split = trial;
            return Retained {
                cells: kept,
                rejected,
                pieces: Some(buf),
            };
        }
        take /= 2;
    }
    Retained {
        cells: Vec::new(),
        rejected: u32::try_from(kept.len()).unwrap_or(u32::MAX),
        pieces: None,
    }
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
    select_cells_with(topo, model, params, view, prev_split, resident_range, true)
}

/// [`select_cells`] with the decision margins optional. With `want_slack` false the margins are
/// not computed (about 5 percent of a hex selection) and `CellSelection::slack` is the all-zero
/// default, which allows no motion; everything else is identical.
#[must_use]
pub fn select_cells_with<T: Topology>(
    topo: &T,
    model: &LodModel,
    params: &LodParams,
    view: &View,
    prev_split: &BTreeSet<T::Cell>,
    resident_range: &dyn Fn(T::Cell) -> Option<HeightRange>,
    want_slack: bool,
) -> CellSelection<T::Cell> {
    let mut stats = SelectStats {
        tau_eff: params.tau_px,
        ..SelectStats::default()
    };
    let occluder = model.radius + model.height.min;
    let low = params.tau_px * (1.0 - params.hysteresis);
    let mut split: BTreeSet<T::Cell> = BTreeSet::new();
    let mut heap: BinaryHeap<(u64, Reverse<u64>)> = BinaryHeap::new();

    let memo: core::cell::RefCell<RangeMemo<T::Cell>> = core::cell::RefCell::new(RangeMemo::new());
    // Margins of every decision taken (07.1-27): minimum over all evaluated cells.
    let mut slack = SelectSlack {
        margin_eye: f64::INFINITY,
        margin_plane: f64::INFINITY,
        d_plane_max: 0.0,
        eye_cap: horizon_eye_cap(view.eye, occluder),
    };

    // Evaluates a cell: `Some(rho)` when it is a refinement candidate, `None` when culled.
    let eval = |id: T::Cell, stats: &mut SelectStats, slack: &mut SelectSlack| -> Option<f64> {
        stats.nodes_visited = stats.nodes_visited.saturating_add(1);
        let range = memo.borrow_mut().range(topo, model, id, resident_range);
        let fp = topo.footprint(id);
        let note_eye = |slack: &mut SelectSlack, m: f64| {
            if m < slack.margin_eye || m.is_nan() {
                slack.margin_eye = if m.is_nan() { 0.0 } else { m };
            }
        };
        if want_slack {
            note_eye(
                slack,
                horizon_margin_m(model.radius, view.eye, &fp, range, occluder),
            );
        }
        if !horizon_visible(model.radius, view.eye, &fp, range, occluder) {
            stats.culled_horizon += 1;
            return None;
        }
        // The cell under the camera is never culled by the frustum.
        if want_slack {
            note_eye(slack, cone_margin_m(view.eye, &fp));
        }
        if !eye_in_cone(view.eye, &fp) {
            let b = cell_bound(model.radius, &fp, range);
            if let Some(m) = frustum_margin(view, &b).filter(|_| want_slack) {
                if m < slack.margin_plane || m.is_nan() {
                    slack.margin_plane = if m.is_nan() { 0.0 } else { m };
                }
                let dx = b.center.x - view.eye.x;
                let dy = b.center.y - view.eye.y;
                let dz = b.center.z - view.eye.z;
                let d = (dx * dx + dy * dy + dz * dz).sqrt();
                if d > slack.d_plane_max || d.is_nan() {
                    slack.d_plane_max = if d.is_nan() { f64::INFINITY } else { d };
                }
            }
            if !frustum_visible(view, &b) {
                stats.culled_frustum += 1;
                return None;
            }
        }
        let d = cell_distance(model.radius, view.eye, &fp, range);
        let d = if d > params.min_distance {
            d
        } else {
            params.min_distance
        };
        let num = model.geometric_error(topo.level(id)) * view.proj_scale;
        if want_slack {
            note_eye(slack, sse_margin_m(num, d, params.tau_px, low));
        }
        Some(num / d)
    };

    if params.max_level > 0 {
        for &id in topo.roots() {
            if stats.nodes_visited >= params.max_nodes_visited {
                stats.node_budget_hit = true;
                break;
            }
            if let Some(rho) = eval(id, &mut stats, &mut slack) {
                heap.push((key_of(rho), Reverse(topo.raw(id))));
            }
        }
    }

    // Piece estimate: each split replaces one piece by its children (exact in eager mode).
    let mut estimate = topo.roots().len();
    let mut children: Vec<T::Cell> = Vec::new();
    // Eager mode: the cells that are a child of a split cell (drawn unless split themselves).
    // Drawn pieces are the unsplit roots plus the unsplit cells of this set.
    let mut drawn_kids: BTreeSet<T::Cell> = BTreeSet::new();
    let mut new_kids: BTreeSet<T::Cell> = BTreeSet::new();
    let mut expanded: BTreeSet<T::Cell> = BTreeSet::new();
    let mut forced_group: Vec<T::Cell> = Vec::new();
    let mut kids: Vec<T::Cell> = Vec::new();
    let mut req: Vec<T::Cell> = Vec::new();
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
        // A cell reachable through two parents is queued twice; refine it once.
        if !expanded.insert(id) {
            continue;
        }
        let grandchildren = topo.level(first) < params.max_level;
        let need = u32::try_from(children.len()).unwrap_or(u32::MAX);
        if split.contains(&id) {
            // Already split by an earlier forced split (eager mode): only evaluate its children.
            if grandchildren {
                if stats.nodes_visited.saturating_add(need) > params.max_nodes_visited {
                    stats.node_budget_hit = true;
                    break;
                }
                for &c in &children {
                    if let Some(r) = eval(c, &mut stats, &mut slack) {
                        heap.push((key_of(r), Reverse(topo.raw(c))));
                    }
                }
            }
            continue;
        }
        let eager = topo.eager_closure();
        // Piece-count change of splitting `id`: exact under the eager rule (see below), the
        // replace-one-by-its-children estimate otherwise.
        forced_group.clear();
        new_kids.clear();
        let delta: isize = if eager {
            forced_group.push(id);
            let mut at = 0;
            while at < forced_group.len() {
                let f = forced_group[at];
                at += 1;
                req.clear();
                topo.required_splits(f, &mut req);
                for &r in &req {
                    if !split.contains(&r) && !forced_group.contains(&r) {
                        forced_group.push(r);
                    }
                }
            }
            for &f in &forced_group {
                kids.clear();
                topo.children(f, &mut kids);
                new_kids.extend(kids.iter().copied().filter(|x| !drawn_kids.contains(x)));
            }
            let added = new_kids
                .iter()
                .filter(|x| !forced_group.contains(x))
                .count();
            let removed = forced_group
                .iter()
                .filter(|f| topo.level(**f) == 0 || drawn_kids.contains(f))
                .count();
            isize::try_from(added).unwrap_or(isize::MAX)
                - isize::try_from(removed).unwrap_or(isize::MAX)
        } else {
            isize::try_from(children.len() - 1).unwrap_or(isize::MAX)
        };
        let est_after = isize::try_from(estimate)
            .unwrap_or(isize::MAX)
            .saturating_add(delta);
        if est_after > isize::try_from(params.max_leaves).unwrap_or(isize::MAX) {
            stats.leaf_budget_hit = true;
            continue;
        }
        if grandchildren && stats.nodes_visited.saturating_add(need) > params.max_nodes_visited {
            stats.node_budget_hit = true;
            break;
        }
        split.insert(id);
        if eager {
            split.extend(forced_group.iter().copied());
            drawn_kids.extend(new_kids.iter().copied());
        }
        estimate = usize::try_from(est_after).unwrap_or(0);
        if grandchildren {
            for &c in &children {
                if let Some(r) = eval(c, &mut stats, &mut slack) {
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
            let range = memo.borrow_mut().range(topo, model, s, resident_range);
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

    // `pieces` already belongs to the final split: it was filled by the budget check above or by
    // the last evaluation of the trim loop condition (07.1-27: no recomputation).
    stats.leaves = pieces.len();
    CellSelection {
        split,
        pieces,
        stats,
        slack: if want_slack {
            slack
        } else {
            SelectSlack::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CubeSphere, HexSphere};

    fn splitmix(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// The memoised nearest-resident-ancestor range equals the plain parent walk for every cell,
    /// whatever the order of the queries and whichever cells are resident.
    fn memo_equals_walk<T: Topology>(topo: &T, model: &LodModel, seed: u64) {
        let mut st = seed;
        let mut cells: Vec<T::Cell> = Vec::new();
        let mut kids = Vec::new();
        let mut frontier: Vec<T::Cell> = topo.roots().to_vec();
        for _ in 0..5 {
            let mut next = Vec::new();
            for &c in &frontier {
                kids.clear();
                topo.children(c, &mut kids);
                next.extend(kids.iter().copied());
            }
            cells.extend(frontier.iter().copied());
            frontier = next;
        }
        cells.extend(frontier.iter().copied());
        // Roughly a third of the cells are resident with a range that depends on the cell.
        let mut resident: BTreeMap<T::Cell, HeightRange> = BTreeMap::new();
        for &c in &cells {
            if splitmix(&mut st).is_multiple_of(3) {
                let h = (splitmix(&mut st) % 4000) as f64;
                resident.insert(
                    c,
                    HeightRange {
                        min: h - 900.0,
                        max: h,
                    },
                );
            }
        }
        let lookup = |c: T::Cell| resident.get(&c).copied();
        let mut memo = RangeMemo::new();
        // Shuffled query order, each cell twice (a hit after a miss).
        let mut order: Vec<usize> = (0..cells.len()).collect();
        for i in (1..order.len()).rev() {
            let j = (splitmix(&mut st) % (i as u64 + 1)) as usize;
            order.swap(i, j);
        }
        for &i in order.iter().chain(order.iter()) {
            let c = cells[i];
            let a = memo.range(topo, model, c, &lookup);
            let b = range_for(topo, model, c, &lookup);
            assert_eq!(
                (a.min.to_bits(), a.max.to_bits()),
                (b.min.to_bits(), b.max.to_bits())
            );
        }
        // No resident cell at all: the global range.
        let none = |_: T::Cell| None;
        let mut memo = RangeMemo::new();
        for &c in &cells {
            assert_eq!(memo.range(topo, model, c, &none), model.height);
        }
    }

    fn model_for<T: Topology>(t: &T) -> LodModel {
        LodModel {
            radius: 6_371_000.0,
            height: HeightRange {
                min: -3800.0,
                max: 6800.0,
            },
            residual: [120.0; 25],
            cells_per_edge: 16,
            sample_angle0: t.sample_angle0(),
            kappa: 0.25,
        }
    }

    #[test]
    fn range_memo_equals_the_parent_walk_hex_and_cube() {
        let h = HexSphere::new(16).unwrap();
        memo_equals_walk(&h, &model_for(&h), 11);
        let c = CubeSphere::new(16);
        memo_equals_walk(&c, &model_for(&c), 12);
    }
}
