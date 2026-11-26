//! Effective (residency-limited) drawn cut with a coarsen-to-balance stage.
//!
//! The wanted split set is 2:1 balanced, but while streaming is incomplete only resident patches
//! can be drawn. A node is split only when it is wanted and all four children are resident, so the
//! coarse parent stays on screen until its children arrive. That alone can break the 2:1 rule
//! between drawn neighbors, so a coarsen stage removes splits until the drawn cut is balanced
//! again, keeping the skirt bound valid every frame.

use std::collections::{BTreeMap, BTreeSet};

use planetary_core::{Edge, Face, PatchId, neighbor};

/// The drawn cut.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveCut {
    /// Nodes of the drawn tree that are split (their children are drawn or split further).
    pub split: BTreeSet<PatchId>,
    /// Drawn leaves, ascending.
    pub leaves: Vec<PatchId>,
    /// Number of splits removed by the coarsen stage.
    pub coarsened: u32,
}

fn build_split(
    wanted_split: &BTreeSet<PatchId>,
    is_resident: &dyn Fn(PatchId) -> bool,
) -> BTreeSet<PatchId> {
    let mut split = BTreeSet::new();
    let mut stack: Vec<PatchId> = Face::ALL
        .iter()
        .rev()
        .map(|f| PatchId::root(*f))
        .filter(|r| is_resident(*r))
        .collect();
    while let Some(p) = stack.pop() {
        if !wanted_split.contains(&p) {
            continue;
        }
        let Some(ch) = p.children() else { continue };
        if ch.iter().all(|c| is_resident(*c)) {
            split.insert(p);
            stack.extend(ch);
        }
    }
    split
}

/// Leaves of the drawn tree: resident roots not split plus unsplit children of split nodes.
fn drawn_leaves(split: &BTreeSet<PatchId>, is_resident: &dyn Fn(PatchId) -> bool) -> Vec<PatchId> {
    let mut out = Vec::new();
    for f in Face::ALL {
        let r = PatchId::root(f);
        if !split.contains(&r) && is_resident(r) {
            out.push(r);
        }
    }
    for s in split {
        if let Some(ch) = s.children() {
            for c in ch {
                if !split.contains(&c) {
                    out.push(c);
                }
            }
        }
    }
    out.sort();
    out
}

/// Residency-limited cut without the coarsen stage (negative control and diagnostics).
#[must_use]
pub fn naive_cut(
    wanted_split: &BTreeSet<PatchId>,
    is_resident: &dyn Fn(PatchId) -> bool,
) -> EffectiveCut {
    let split = build_split(wanted_split, is_resident);
    let leaves = drawn_leaves(&split, is_resident);
    EffectiveCut {
        split,
        leaves,
        coarsened: 0,
    }
}

/// Parents of drawn leaves that have a drawn edge-neighbor at least two levels coarser.
fn violating_parents(leaves: &[PatchId]) -> BTreeSet<PatchId> {
    let level_of: BTreeMap<PatchId, u8> = leaves.iter().map(|l| (*l, l.level())).collect();
    let mut parents = BTreeSet::new();
    for &p in leaves {
        let level = p.level();
        if level < 2 {
            continue;
        }
        for e in Edge::ALL {
            let mut cur = Some(neighbor(p, e).patch);
            while let Some(q) = cur {
                if let Some(&ql) = level_of.get(&q) {
                    if ql + 2 <= level {
                        if let Some(par) = p.parent() {
                            parents.insert(par);
                        }
                    }
                    break;
                }
                cur = q.parent();
            }
        }
    }
    parents
}

/// Residency-limited cut that is a partition of the resident faces, resident, and 2:1 balanced.
#[must_use]
pub fn effective_cut(
    wanted_split: &BTreeSet<PatchId>,
    is_resident: &dyn Fn(PatchId) -> bool,
) -> EffectiveCut {
    let mut split = build_split(wanted_split, is_resident);
    let mut coarsened = 0u32;
    loop {
        let leaves = drawn_leaves(&split, is_resident);
        let bad = violating_parents(&leaves);
        if bad.is_empty() {
            return EffectiveCut {
                split,
                leaves,
                coarsened,
            };
        }
        let before = split.len();
        split.retain(|s| !bad.iter().any(|b| s == b || b.is_ancestor_of(*s)));
        // Each violating parent is in `split`, so the set strictly shrinks (terminates).
        coarsened += u32::try_from(before - split.len()).unwrap_or(u32::MAX);
    }
}
