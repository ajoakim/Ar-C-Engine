//! Pure request planning: ancestors-first, coarse-first, deduplicated, capped; stale detection.

use std::collections::BTreeSet;

use engine_sdk::Priority;
use planetary_core::Topology;

/// For each wanted piece, the coarsest non-resident ancestor-or-self along the primary-parent
/// chain whose parent is resident (or which is a root). Pending cells are skipped. Sorted by
/// (level, raw id), truncated to `max`.
#[must_use]
pub fn plan_requests<T: Topology>(
    topo: &T,
    wanted_pieces: &[T::Cell],
    is_resident: &dyn Fn(T::Cell) -> bool,
    is_pending: &dyn Fn(T::Cell) -> bool,
    max: usize,
) -> Vec<T::Cell> {
    let mut out: BTreeSet<(u8, u64)> = BTreeSet::new();
    let mut chain: Vec<T::Cell> = Vec::new();
    for &piece in wanted_pieces {
        chain.clear();
        let mut cur = Some(piece);
        while let Some(c) = cur {
            chain.push(c);
            cur = topo.primary_parent(c);
        }
        // Root first; the first non-resident node has a resident parent (or is a root).
        for &a in chain.iter().rev() {
            if is_resident(a) {
                continue;
            }
            if !is_pending(a) {
                out.insert((topo.level(a), topo.raw(a)));
            }
            break;
        }
    }
    out.into_iter()
        .take(max)
        .filter_map(|(_, raw)| topo.from_raw(raw))
        .collect()
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
