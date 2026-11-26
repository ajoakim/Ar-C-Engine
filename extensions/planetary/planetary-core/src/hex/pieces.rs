//! Hex balance closure and drawn pieces (half-cell rule, HEX-02). Integer only.
//!
//! A cell may be split only if all its parents are split ([`required_splits`], parents-only
//! closure, ADR-0011); [`close`] inserts until that holds. The drawn pieces of
//! a closed split set tile the sphere exactly once: unsplit roots are whole; every unsplit child
//! of a split parent is drawn with the sectors owned by its split parents. The centre child is
//! whole, an edge child is half (3 sectors on the side of its split parent) when only one of its
//! two parents is split and whole when both are. Pentagons are centre children (or roots), so they
//! are never halved.
//!
//! Sector `k` of a cell is the sector toward `ring(cell)[k]` (see [`crate::hex::mapping::CellFrame`]).
//! A half variant is defined over ring indices only, so it does not depend on where the ring starts.

use super::adjacency::{children, is_pentagon, parents};
use super::cell_id::CellId;
use super::mapping::ring;
use crate::topology::Piece;
use std::collections::BTreeSet;

/// Whole hexagon.
pub const VARIANT_FULL_HEX: u16 = 0;
/// Whole pentagon.
pub const VARIANT_FULL_PENTAGON: u16 = 1;

/// Half cell: the three sectors `s, s+1, s+2` (mod 6) with `s = diag + 3 * side`. `diag` in `0..3`
/// is the corner-centre-corner diagonal (through corners `diag` and `diag + 3`), `side` in `0..2`.
pub const fn half_variant(diag: u8, side: u8) -> u16 {
    2 + 2 * diag as u16 + side as u16
}

const SECTORS_HEX: [u8; 6] = [0, 1, 2, 3, 4, 5];
const SECTORS_PENTAGON: [u8; 5] = [0, 1, 2, 3, 4];
const SECTORS_HALF: [[u8; 3]; 6] = [
    [0, 1, 2],
    [3, 4, 5],
    [1, 2, 3],
    [4, 5, 0],
    [2, 3, 4],
    [5, 0, 1],
];

/// Sector indices (ring order) drawn by variant `v`; empty for an unknown variant.
pub fn variant_sectors(v: u16) -> &'static [u8] {
    match v {
        VARIANT_FULL_HEX => &SECTORS_HEX,
        VARIANT_FULL_PENTAGON => &SECTORS_PENTAGON,
        2..=7 => &SECTORS_HALF[usize::from(v - 2)],
        _ => &[],
    }
}

/// Appends the cells that must be split for `c` to be split: its parents, ascending raw. All are
/// at `level(c) - 1`. There is no neighbour rule (ADR-0011): jump-1 balance is empirical.
pub fn required_splits(c: CellId, out: &mut Vec<CellId>) {
    let first = out.len();
    let mut ps = Vec::with_capacity(2);
    parents(c, &mut ps);
    let set: BTreeSet<CellId> = ps.into_iter().collect();
    debug_assert!(set.iter().all(|x| x.level() + 1 == c.level()));
    out.extend(set);
    debug_assert!(out[first..].windows(2).all(|w| w[0].raw() < w[1].raw()));
}

/// Inserts every required split until a fixpoint; returns the number of inserted cells.
/// Idempotent; only shallower cells are inserted, so it terminates.
pub fn close(split: &mut BTreeSet<CellId>) -> u32 {
    let mut inserted = 0u32;
    let mut work: Vec<CellId> = split.iter().copied().collect();
    let mut req = Vec::with_capacity(12);
    while let Some(x) = work.pop() {
        req.clear();
        required_splits(x, &mut req);
        for r in &req {
            if split.insert(*r) {
                inserted += 1;
                work.push(*r);
            }
        }
    }
    inserted
}

fn whole_variant(c: CellId) -> u16 {
    if is_pentagon(c) {
        VARIANT_FULL_PENTAGON
    } else {
        VARIANT_FULL_HEX
    }
}

/// The half variant of edge child `x` on the side of its split parent whose centre child is `pf`.
fn half_toward(x: CellId, pf: CellId) -> u16 {
    let r = ring(x);
    let n = r.len() as u8;
    let idx = r.iter().position(|y| *y == pf).unwrap_or(0) as u8;
    let s = (idx + n - 1) % n;
    half_variant(s % 3, s / 3)
}

/// Drawn pieces of a closed split set, sorted by (raw, variant).
pub fn pieces(split: &BTreeSet<CellId>, out: &mut Vec<Piece<CellId>>) {
    let first = out.len();
    let mut drawn: BTreeSet<(CellId, u16)> = BTreeSet::new();
    for root in CellId::level_iter(0) {
        if !split.contains(&root) {
            drawn.insert((root, whole_variant(root)));
        }
    }
    let mut kids = Vec::with_capacity(7);
    let mut ps = Vec::with_capacity(2);
    for p in split {
        kids.clear();
        children(*p, &mut kids);
        let Some(pf) = kids.first().copied() else {
            continue;
        };
        for x in &kids {
            if split.contains(x) {
                continue;
            }
            ps.clear();
            parents(*x, &mut ps);
            if ps.len() < 2 || ps.iter().all(|q| split.contains(q)) {
                drawn.insert((*x, whole_variant(*x)));
            } else {
                drawn.insert((*x, half_toward(*x, pf)));
            }
        }
    }
    out.extend(
        drawn
            .into_iter()
            .map(|(cell, variant)| Piece { cell, variant }),
    );
    debug_assert!(out[first..].windows(2).all(|w| w[0] <= w[1]));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcs(split: &BTreeSet<CellId>) -> Vec<Piece<CellId>> {
        let mut v = Vec::new();
        pieces(split, &mut v);
        v
    }

    #[test]
    fn required_splits_are_the_parents() {
        for level in 1..=3u8 {
            for c in CellId::level_iter(level) {
                let mut req = Vec::new();
                required_splits(c, &mut req);
                let mut ps = Vec::new();
                parents(c, &mut ps);
                let expect: BTreeSet<CellId> = ps.iter().copied().collect();
                assert!(req.iter().all(|r| r.level() + 1 == level));
                assert_eq!(req, expect.into_iter().collect::<Vec<_>>(), "{c:?}");
            }
        }
        let mut req = Vec::new();
        required_splits(CellId::north_pole(0), &mut req);
        assert!(req.is_empty());
    }

    #[test]
    fn close_is_idempotent_and_reaches_fixpoint() {
        let mut split: BTreeSet<CellId> = BTreeSet::new();
        split.insert(CellId::new(3, 4, 5, 9).unwrap());
        let n = close(&mut split);
        assert!(n > 0);
        assert_eq!(close(&mut split), 0);
        let mut req = Vec::new();
        for c in &split {
            req.clear();
            required_splits(*c, &mut req);
            assert!(req.iter().all(|r| split.contains(r)));
        }
    }

    #[test]
    fn empty_split_is_the_twelve_roots() {
        let p = pcs(&BTreeSet::new());
        assert_eq!(p.len(), 12);
        assert!(
            p.iter()
                .all(|x| x.cell.level() == 0 && x.variant == VARIANT_FULL_PENTAGON)
        );
    }

    #[test]
    fn single_split_root_draws_six_children() {
        let root = CellId::north_pole(0);
        let split: BTreeSet<CellId> = [root].into_iter().collect();
        let p = pcs(&split);
        let kids: Vec<_> = p.iter().filter(|x| x.cell.level() == 1).collect();
        assert_eq!(kids.len(), 6);
        // The centre child is the north pole (pentagon, whole); the 5 edge children are halves.
        let centre = CellId::north_pole(1);
        assert!(
            kids.iter()
                .any(|x| x.cell == centre && x.variant == VARIANT_FULL_PENTAGON)
        );
        let halves = kids.iter().filter(|x| x.variant >= 2).count();
        assert_eq!(halves, 5);
        assert_eq!(p.len(), 6 + 11);
    }

    #[test]
    fn variant_tables() {
        assert_eq!(variant_sectors(VARIANT_FULL_HEX).len(), 6);
        assert_eq!(variant_sectors(VARIANT_FULL_PENTAGON).len(), 5);
        assert!(variant_sectors(8).is_empty());
        for diag in 0..3u8 {
            let a = variant_sectors(half_variant(diag, 0));
            let b = variant_sectors(half_variant(diag, 1));
            assert_eq!((a.len(), b.len()), (3, 3));
            assert!(a.iter().all(|s| !b.contains(s)));
            let all: BTreeSet<u8> = a.iter().chain(b).copied().collect();
            assert_eq!(all.len(), 6);
        }
    }

    #[test]
    fn pentagons_are_never_halved() {
        let mut split: BTreeSet<CellId> = CellId::level_iter(0).collect();
        split.insert(CellId::north_pole(1));
        split.insert(CellId::south_pole(1));
        close(&mut split);
        for p in pcs(&split) {
            if is_pentagon(p.cell) {
                assert_eq!(p.variant, VARIANT_FULL_PENTAGON);
            }
        }
    }
}
