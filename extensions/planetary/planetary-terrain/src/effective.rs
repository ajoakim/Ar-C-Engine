//! Effective (residency-limited) drawn cut with a coarsen-to-balance stage, generic over the
//! topology.
//!
//! The wanted split set is balanced, but while streaming is incomplete only resident cells can be
//! drawn. A cell is split only when it is wanted and all its children are resident, so the coarse
//! parent stays on screen until its children arrive. That alone can break the balance rule between
//! drawn neighbours, so a coarsen stage removes splits until every split cell's
//! `required_splits` are present (a fixpoint), keeping the skirt bound valid every frame.

use std::collections::{BTreeMap, BTreeSet};

use planetary_core::{Piece, Topology};

/// The drawn cut.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveCut<C> {
    /// Cells of the drawn tree that are split (their children are drawn or split further).
    pub split: BTreeSet<C>,
    /// Drawn pieces (resident only), ascending as produced by `Topology::pieces`.
    pub pieces: Vec<Piece<C>>,
    /// Number of splits removed by the coarsen stage.
    pub coarsened: u32,
}

fn build_split<T: Topology>(
    topo: &T,
    wanted_split: &BTreeSet<T::Cell>,
    is_resident: &dyn Fn(T::Cell) -> bool,
) -> BTreeSet<T::Cell> {
    let mut split = BTreeSet::new();
    let mut stack: Vec<T::Cell> = topo
        .roots()
        .iter()
        .rev()
        .copied()
        .filter(|r| is_resident(*r))
        .collect();
    let mut ch: Vec<T::Cell> = Vec::new();
    while let Some(p) = stack.pop() {
        if !wanted_split.contains(&p) || split.contains(&p) {
            continue;
        }
        ch.clear();
        topo.children(p, &mut ch);
        if ch.is_empty() {
            continue;
        }
        if ch.iter().all(|c| is_resident(*c)) {
            split.insert(p);
            stack.extend(ch.iter().copied());
        }
    }
    split
}

/// Pieces of a split set, restricted to resident cells. A piece can be non-resident only when a
/// root is missing, in which case nothing is drawn there.
fn resident_pieces<T: Topology>(
    topo: &T,
    split: &BTreeSet<T::Cell>,
    is_resident: &dyn Fn(T::Cell) -> bool,
) -> Vec<Piece<T::Cell>> {
    let mut pieces = Vec::new();
    topo.pieces(split, &mut pieces);
    pieces.retain(|p| is_resident(p.cell));
    pieces
}

/// Residency-limited cut without the coarsen stage (negative control and diagnostics).
#[must_use]
pub fn naive_cut<T: Topology>(
    topo: &T,
    wanted_split: &BTreeSet<T::Cell>,
    is_resident: &dyn Fn(T::Cell) -> bool,
) -> EffectiveCut<T::Cell> {
    let split = build_split(topo, wanted_split, is_resident);
    let pieces = resident_pieces(topo, &split, is_resident);
    EffectiveCut {
        split,
        pieces,
        coarsened: 0,
    }
}

/// True when `r` is split, or when nothing resident is drawn where `r` lies (a missing root).
fn satisfied<T: Topology>(
    topo: &T,
    split: &BTreeSet<T::Cell>,
    is_resident: &dyn Fn(T::Cell) -> bool,
    r: T::Cell,
) -> bool {
    if split.contains(&r) {
        return true;
    }
    // The drawn cell covering `r`: climb until the parent is split (or a root is reached).
    let mut d = r;
    while let Some(p) = topo.primary_parent(d) {
        if split.contains(&p) {
            break;
        }
        d = p;
    }
    !is_resident(d)
}

/// Residency-limited cut that is a partition of the resident roots, resident, and balanced: every
/// split cell has all its `required_splits` split.
#[must_use]
pub fn effective_cut<T: Topology>(
    topo: &T,
    wanted_split: &BTreeSet<T::Cell>,
    is_resident: &dyn Fn(T::Cell) -> bool,
) -> EffectiveCut<T::Cell> {
    let mut split = build_split(topo, wanted_split, is_resident);
    let mut required: BTreeMap<T::Cell, Vec<T::Cell>> = BTreeMap::new();
    let mut buf: Vec<T::Cell> = Vec::new();
    for &s in &split {
        buf.clear();
        topo.required_splits(s, &mut buf);
        required.insert(s, buf.clone());
    }
    let mut coarsened = 0u32;
    loop {
        let bad: Vec<T::Cell> = split
            .iter()
            .copied()
            .filter(|s| {
                required
                    .get(s)
                    .is_some_and(|rs| rs.iter().any(|r| !satisfied(topo, &split, is_resident, *r)))
            })
            .collect();
        if bad.is_empty() {
            break;
        }
        // Each violator is a member of `split`, so the set strictly shrinks (terminates). Split
        // descendants of a removed cell fail on their next pass (their parent is a requirement).
        for b in &bad {
            split.remove(b);
        }
        coarsened += u32::try_from(bad.len()).unwrap_or(u32::MAX);
    }
    let pieces = resident_pieces(topo, &split, is_resident);
    EffectiveCut {
        split,
        pieces,
        coarsened,
    }
}
