//! Pure request planning: ancestors-first, nearest and largest error first, deduplicated,
//! capped; stale detection.

use std::collections::{BTreeMap, BTreeSet};

use engine_sdk::Priority;
use planetary_core::Topology;
use planetary_core::lod::{Footprint, HeightRange, LodModel, View, cell_distance};

/// For each wanted piece, the coarsest non-resident ancestor-or-self along the primary-parent
/// chain whose parent is resident (or which is a root). Pending cells are skipped. Sorted by
/// (level, raw id), truncated to `max`.
///
/// Order-neutral wrapper of [`plan_requests_scored`] (every piece scores equally, so the
/// (level, raw id) tie-break decides).
#[must_use]
pub fn plan_requests<T: Topology>(
    topo: &T,
    wanted_pieces: &[T::Cell],
    is_resident: &dyn Fn(T::Cell) -> bool,
    is_pending: &dyn Fn(T::Cell) -> bool,
    max: usize,
) -> Vec<T::Cell> {
    let scored: Vec<(T::Cell, f64)> = wanted_pieces.iter().map(|&c| (c, 0.0)).collect();
    plan_requests_scored(topo, &scored, is_resident, is_pending, max)
}

/// Priority-ordered request planning.
///
/// `wanted` pairs each wanted piece with an urgency score (larger first; see
/// [`request_score`]). For each piece the same cell as [`plan_requests`] is chosen (the coarsest
/// non-resident ancestor-or-self whose parent is resident, so a leaf is never requested before
/// the parent that serves as its fallback) and it inherits the BEST score of every piece that
/// needs it, so a parent needed by an urgent leaf is urgent. Output order: level-0 roots first
/// (coverage), then score descending, then (level, raw id) ascending as the deterministic
/// tie-break; truncated to `max`. Non-finite
/// scores count as 0. Pure: no clock, no hashed containers.
#[must_use]
pub fn plan_requests_scored<T: Topology>(
    topo: &T,
    wanted: &[(T::Cell, f64)],
    is_resident: &dyn Fn(T::Cell) -> bool,
    is_pending: &dyn Fn(T::Cell) -> bool,
    max: usize,
) -> Vec<T::Cell> {
    let mut best: BTreeMap<(u8, u64), f64> = BTreeMap::new();
    // First non-resident cell on the chain root..cell, per cell (None: the whole chain is
    // resident). Pieces share ancestors, so each chain is walked once instead of once per piece;
    // the value is exactly what the root-first scan over the chain of each piece finds.
    let mut first_gap: BTreeMap<T::Cell, Option<T::Cell>> = BTreeMap::new();
    let mut stack: Vec<T::Cell> = Vec::new();
    for &(piece, score) in wanted {
        let score = if score.is_finite() { score } else { 0.0 };
        stack.clear();
        let mut cur = Some(piece);
        let mut below: Option<Option<T::Cell>> = None;
        while let Some(c) = cur {
            if let Some(g) = first_gap.get(&c) {
                below = Some(*g);
                break;
            }
            stack.push(c);
            cur = topo.primary_parent(c);
        }
        // `below` is the answer for the parent of the last pushed cell (None: reached a root).
        let mut gap: Option<T::Cell> = below.flatten();
        for &c in stack.iter().rev() {
            if gap.is_none() && !is_resident(c) {
                gap = Some(c);
            }
            first_gap.insert(c, gap);
        }
        if let Some(a) = gap
            && !is_pending(a)
        {
            let e = best.entry((topo.level(a), topo.raw(a))).or_insert(score);
            if score > *e {
                *e = score;
            }
        }
    }
    let mut order: Vec<((u8, u64), f64)> = best.into_iter().collect();
    // BTreeMap iteration is (level, raw) ascending and the sort is stable. Roots (level 0) sort
    // first: until every root is resident a hole is unavoidable, so coverage beats detail.
    order.sort_by(|a, b| {
        (a.0.0 != 0)
            .cmp(&(b.0.0 != 0))
            .then_with(|| b.1.total_cmp(&a.1))
    });
    order
        .into_iter()
        .take(max)
        .filter_map(|((_, raw), _)| topo.from_raw(raw))
        .collect()
}

/// Urgency of a wanted piece: its screen-space error, `geometric_error(level) * proj_scale /
/// max(distance, 1 m)` with the distance from the eye to the footprint extruded over `range`.
/// Nearer and coarser-error pieces score higher. Exact operations only (ADR-0008).
#[must_use]
pub fn request_score(
    model: &LodModel,
    view: &View,
    fp: &Footprint,
    range: HeightRange,
    level: u8,
) -> f64 {
    let d = cell_distance(model.radius, view.eye, fp, range);
    let d = if d > 1.0 { d } else { 1.0 };
    model.geometric_error(level) * view.proj_scale / d
}

/// The wanted pieces plus all their ancestors (every parent of a two-parent cell).
#[must_use]
pub fn wanted_closure<T: Topology>(topo: &T, wanted: &[T::Cell]) -> BTreeSet<T::Cell> {
    let mut out = BTreeSet::new();
    let mut work: Vec<T::Cell> = Vec::new();
    let mut parents: Vec<T::Cell> = Vec::new();
    for &cell in wanted {
        work.push(cell);
        while let Some(c) = work.pop() {
            if !out.insert(c) {
                continue;
            }
            parents.clear();
            topo.parents(c, &mut parents);
            work.extend(parents.iter().copied());
        }
    }
    out
}

/// Requested cells no longer in the wanted closure (candidates for cancellation), ascending.
#[must_use]
pub fn stale_requests<C: Ord + Copy>(requested: &BTreeSet<C>, closure: &BTreeSet<C>) -> Vec<C> {
    requested.difference(closure).copied().collect()
}

/// Coarse patches (level <= 3) are urgent; the rest are normal priority.
#[must_use]
pub fn priority_for_level(level: u8) -> Priority {
    if level <= 3 {
        Priority::High
    } else {
        Priority::Normal
    }
}
