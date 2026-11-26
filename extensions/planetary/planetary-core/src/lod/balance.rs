//! Restricted-quadtree (2:1) balancing across cube faces.

use std::collections::{BTreeSet, VecDeque};

use crate::adjacency::neighbor;
use crate::face::{Edge, Face};
use crate::patch_id::PatchId;

/// Inserts splits until all edge-neighbors differ by at most one level. Only inserts.
/// Returns the number of forced splits.
pub fn balance(split: &mut BTreeSet<PatchId>) -> u32 {
    let mut work: VecDeque<PatchId> = VecDeque::new();
    for f in Face::ALL {
        work.push_back(PatchId::root(f));
    }
    for s in split.iter() {
        if let Some(ch) = s.children() {
            work.extend(ch);
        }
    }
    let mut forced = 0u32;
    while let Some(p) = work.pop_front() {
        let level = p.level();
        if level < 2 {
            continue;
        }
        for e in Edge::ALL {
            let q = neighbor(p, e).patch;
            let Some(g) = q.ancestor_at(level - 2) else {
                continue;
            };
            if split.contains(&g) {
                continue;
            }
            // Insert g and all missing ancestors (top-down so children get queued).
            let mut chain = Vec::new();
            let mut cur = Some(g);
            while let Some(c) = cur {
                if split.contains(&c) {
                    break;
                }
                chain.push(c);
                cur = c.parent();
            }
            for c in chain.into_iter().rev() {
                split.insert(c);
                forced += 1;
                if let Some(ch) = c.children() {
                    work.extend(ch);
                }
            }
        }
    }
    forced
}

/// Leaves of the tree described by `split` (roots not split plus unsplit children), ascending.
#[must_use]
pub fn leaves_of(split: &BTreeSet<PatchId>) -> Vec<PatchId> {
    let mut out = Vec::new();
    for f in Face::ALL {
        let r = PatchId::root(f);
        if !split.contains(&r) {
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
