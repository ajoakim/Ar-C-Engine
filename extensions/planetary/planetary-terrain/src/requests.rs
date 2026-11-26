//! Pure request planning: ancestors-first, coarse-first, deduplicated, capped; stale detection.

use std::collections::BTreeSet;

use engine_sdk::Priority;
use planetary_core::PatchId;

/// For each wanted leaf, the coarsest non-resident ancestor-or-self whose parent is resident (or
/// which is a root). Pending ids are skipped. Sorted by (level, raw id), truncated to `max`.
#[must_use]
pub fn plan_requests(
    wanted_leaves: &[PatchId],
    is_resident: &dyn Fn(PatchId) -> bool,
    is_pending: &dyn Fn(PatchId) -> bool,
    max: usize,
) -> Vec<PatchId> {
    let mut out: BTreeSet<(u8, u64)> = BTreeSet::new();
    for &leaf in wanted_leaves {
        // Walk from the root down; the first non-resident node has a resident parent (or is a root).
        for lvl in 0..=leaf.level() {
            let Some(a) = leaf.ancestor_at(lvl) else {
                break;
            };
            if is_resident(a) {
                continue;
            }
            if !is_pending(a) {
                out.insert((a.level(), a.raw()));
            }
            break;
        }
    }
    out.into_iter()
        .take(max)
        .filter_map(|(_, raw)| PatchId::from_raw(raw).ok())
        .collect()
}

/// The wanted leaves plus all their ancestors.
#[must_use]
pub fn wanted_closure(wanted_leaves: &[PatchId]) -> BTreeSet<PatchId> {
    let mut out = BTreeSet::new();
    for &leaf in wanted_leaves {
        let mut cur = Some(leaf);
        while let Some(c) = cur {
            if !out.insert(c) {
                break;
            }
            cur = c.parent();
        }
    }
    out
}

/// Requested ids no longer in the wanted closure (candidates for cancellation), ascending.
#[must_use]
pub fn stale_requests(requested: &BTreeSet<PatchId>, closure: &BTreeSet<PatchId>) -> Vec<PatchId> {
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
