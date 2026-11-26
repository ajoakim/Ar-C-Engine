//! HEX-02 coverage proof: the drawn pieces of a balanced split set cover the sphere exactly once
//! and neighbouring pieces differ by at most one level. Promoted from spike S3; runs on the
//! production `hex::{adjacency, mapping, pieces}` APIs.
//!
//! Method (combinatorial, integers + ordered maps only). A level-L hex cell has 6 sectors (5 for a
//! pentagon); sector `(X, n)` is the one toward lattice neighbour `n = ring(X)[k]`. Cutting every
//! level-L lattice triangle `T = (p, n, q)` along its medians gives 6 ATOMS per triangle, named by
//! the ordered triple `(p, n, q)`. A sector `(X, n)` is exactly the two atoms `(X, n, q1)`,
//! `(X, n, q2)` (q1, q2 = the two common neighbours of X and n). Atoms nest dyadically: a coarse
//! atom `(A, B, C)` is the union of the 4 fine atoms `(Af, mAB, mAC)`, `(mAB, Af, mAC)`,
//! `(mAB, mAC, Af)`, `(mAB, mAC, mBC)` (`Af` = centre child of A, `mAB` = edge child of A and B).
//! Coverage is counted exactly on the atoms of the finest level, so no sample ties exist.

use engine_sdk::DVec3;
use planetary_core::hex::CellId;
use planetary_core::hex::adjacency::{children, is_pentagon, neighbors, parents};
use planetary_core::hex::mapping::{cell_center_dir, ring};
use planetary_core::hex::pieces::{
    VARIANT_FULL_HEX, VARIANT_FULL_PENTAGON, close, pieces, variant_sectors,
};
use planetary_core::topology::Piece;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

type Atom = [CellId; 3];

const TOPO_LEVELS: u8 = 5;

fn nb(c: CellId) -> Vec<CellId> {
    let mut v = Vec::with_capacity(6);
    neighbors(c, &mut v);
    v
}

// ---------------------------------------------------------------- topology tables

struct Topo {
    nbrs: BTreeMap<CellId, Vec<CellId>>,
    kids: BTreeMap<CellId, Vec<CellId>>,
    pars: BTreeMap<CellId, Vec<CellId>>,
}

fn topo() -> &'static Topo {
    static T: OnceLock<Topo> = OnceLock::new();
    T.get_or_init(|| {
        let mut nbrs = BTreeMap::new();
        let mut kids = BTreeMap::new();
        let mut pars = BTreeMap::new();
        for level in 0..=TOPO_LEVELS {
            for c in CellId::level_iter(level) {
                nbrs.insert(c, nb(c));
                if level > 0 {
                    let mut p = Vec::new();
                    parents(c, &mut p);
                    pars.insert(c, p);
                }
                if level < TOPO_LEVELS {
                    let mut k = Vec::new();
                    children(c, &mut k);
                    kids.insert(c, k);
                }
            }
        }
        Topo { nbrs, kids, pars }
    })
}

impl Topo {
    fn common(&self, a: CellId, b: CellId) -> Vec<CellId> {
        let nbb = &self.nbrs[&b];
        let out: Vec<CellId> = self.nbrs[&a]
            .iter()
            .copied()
            .filter(|x| nbb.contains(x))
            .collect();
        assert_eq!(out.len(), 2, "{a:?} {b:?} must share exactly 2 neighbours");
        out
    }

    fn edge_child(&self, a: CellId, b: CellId) -> CellId {
        self.kids[&a]
            .iter()
            .skip(1)
            .copied()
            .find(|x| {
                let p = &self.pars[x];
                p.len() == 2 && p.contains(&a) && p.contains(&b)
            })
            .unwrap_or_else(|| panic!("no edge child of {a:?} {b:?}"))
    }

    fn atom_children(&self, [a, b, c]: Atom) -> [Atom; 4] {
        let af = self.kids[&a][0];
        let mab = self.edge_child(a, b);
        let mac = self.edge_child(a, c);
        let mbc = self.edge_child(b, c);
        [
            [af, mab, mac],
            [mab, af, mac],
            [mab, mac, af],
            [mab, mac, mbc],
        ]
    }

    /// All atoms of one level, enumerated independently of the hierarchy.
    fn atoms_direct(&self, level: u8) -> BTreeSet<Atom> {
        let mut out = BTreeSet::new();
        for p in CellId::level_iter(level) {
            for n in &self.nbrs[&p] {
                for q in self.common(p, *n) {
                    out.insert([p, *n, q]);
                }
            }
        }
        out
    }
}

struct Hier {
    atoms: Vec<Vec<Atom>>,
    index: Vec<BTreeMap<Atom, u32>>,
    kids: Vec<Vec<[u32; 4]>>,
    /// Per level: the two atoms sharing a border with each atom (across the hex edge and across a
    /// sector boundary of the same cell).
    adj: Vec<Vec<[u32; 2]>>,
}

fn hier() -> &'static Hier {
    static H: OnceLock<Hier> = OnceLock::new();
    H.get_or_init(|| {
        let t = topo();
        let mut atoms: Vec<Vec<Atom>> = vec![t.atoms_direct(0).into_iter().collect()];
        let mut index: Vec<BTreeMap<Atom, u32>> = Vec::new();
        let mut kids: Vec<Vec<[u32; 4]>> = Vec::new();
        let idx_of = |v: &Vec<Atom>| -> BTreeMap<Atom, u32> {
            let m: BTreeMap<Atom, u32> = v
                .iter()
                .enumerate()
                .map(|(i, a)| (*a, u32::try_from(i).expect("atom index")))
                .collect();
            assert_eq!(m.len(), v.len(), "duplicate atoms");
            m
        };
        index.push(idx_of(&atoms[0]));
        for level in 0..TOPO_LEVELS {
            let mut next: Vec<Atom> = Vec::new();
            let mut k: Vec<[u32; 4]> = Vec::new();
            for a in &atoms[usize::from(level)] {
                let ch = t.atom_children(*a);
                let base = u32::try_from(next.len()).expect("atom index");
                next.extend(ch);
                k.push([base, base + 1, base + 2, base + 3]);
            }
            index.push(idx_of(&next));
            atoms.push(next);
            kids.push(k);
        }
        let adj = (0..=TOPO_LEVELS)
            .map(|l| {
                atoms[usize::from(l)]
                    .iter()
                    .map(|[p, n, q]| {
                        [
                            index[usize::from(l)][&[*n, *p, *q]],
                            index[usize::from(l)][&[*p, *q, *n]],
                        ]
                    })
                    .collect()
            })
            .collect();
        Hier {
            atoms,
            index,
            kids,
            adj,
        }
    })
}

// ---------------------------------------------------------------- pieces as atoms, coverage

struct Cover {
    samples: usize,
    zero: usize,
    multi: usize,
    max_cover: u32,
    max_jump: u8,
    jump_pairs: BTreeMap<u8, usize>,
}

/// A drawn piece as raw atoms of one level: `(level, atom indices at that level)`.
type RawPiece = (u8, Vec<u32>);

fn raw_pieces(t: &Topo, h: &Hier, pcs: &[Piece<CellId>]) -> Vec<RawPiece> {
    pcs.iter()
        .map(|p| {
            let x = p.cell;
            let l = usize::from(x.level());
            let r = ring(x);
            let mut v = Vec::new();
            for k in variant_sectors(p.variant) {
                let n = r[usize::from(*k)];
                for q in t.common(x, n) {
                    v.push(h.index[l][&[x, n, q]]);
                }
            }
            (x.level(), v)
        })
        .collect()
}

fn cover(t: &Topo, h: &Hier, pcs: &[Piece<CellId>], m: u8) -> Cover {
    cover_raw(h, &raw_pieces(t, h, pcs), m)
}

fn cover_raw(h: &Hier, raw: &[RawPiece], m: u8) -> Cover {
    let m = usize::from(m);
    let mut dcnt: Vec<Vec<u32>> = (0..=m).map(|l| vec![0; h.atoms[l].len()]).collect();
    let mut dpid: Vec<Vec<u32>> = (0..=m).map(|l| vec![u32::MAX; h.atoms[l].len()]).collect();
    let mut plevel = Vec::new();
    for (pid, (level, idxs)) in raw.iter().enumerate() {
        plevel.push(*level);
        let l = usize::from(*level);
        assert!(l <= m);
        for i in idxs {
            dcnt[l][*i as usize] += 1;
            dpid[l][*i as usize] = u32::try_from(pid).expect("piece id");
        }
    }
    let mut cnt = dcnt[0].clone();
    let mut pid = dpid[0].clone();
    for l in 0..m {
        let mut nc = vec![0u32; h.atoms[l + 1].len()];
        let mut np = vec![u32::MAX; h.atoms[l + 1].len()];
        for (i, ks) in h.kids[l].iter().enumerate() {
            for k in ks {
                let k = *k as usize;
                nc[k] = cnt[i] + dcnt[l + 1][k];
                np[k] = if dcnt[l + 1][k] > 0 {
                    dpid[l + 1][k]
                } else {
                    pid[i]
                };
            }
        }
        cnt = nc;
        pid = np;
    }
    let zero = cnt.iter().filter(|c| **c == 0).count();
    let multi = cnt.iter().filter(|c| **c > 1).count();
    let max_cover = cnt.iter().copied().max().unwrap_or(0);
    let mut jump_pairs = BTreeMap::new();
    let mut max_jump = 0u8;
    for (i, adjs) in h.adj[m].iter().enumerate() {
        for j in adjs {
            let j = *j as usize;
            if j < i || cnt[i] != 1 || cnt[j] != 1 || pid[i] == pid[j] {
                continue;
            }
            let d = plevel[pid[i] as usize].abs_diff(plevel[pid[j] as usize]);
            *jump_pairs.entry(d).or_insert(0usize) += 1;
            max_jump = max_jump.max(d);
        }
    }
    Cover {
        samples: cnt.len(),
        zero,
        multi,
        max_cover,
        max_jump,
        jump_pairs,
    }
}

// ---------------------------------------------------------------- split sets

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Closure {
    /// No closure at all (negative control).
    None,
    /// Only "all parents of a split cell are split" (negative control for balance).
    ParentsOnly,
    /// The production hex balance rule ([`close`]).
    Full,
}

fn close_mode(t: &Topo, split: &mut BTreeSet<CellId>, mode: Closure) -> u32 {
    match mode {
        Closure::None => 0,
        Closure::Full => close(split),
        Closure::ParentsOnly => {
            let before = split.len();
            let mut work: Vec<CellId> = split.iter().copied().collect();
            while let Some(x) = work.pop() {
                if x.level() == 0 {
                    continue;
                }
                for p in &t.pars[&x] {
                    if split.insert(*p) {
                        work.push(*p);
                    }
                }
            }
            u32::try_from(split.len() - before).expect("count")
        }
    }
}

fn pcs_of(split: &BTreeSet<CellId>) -> Vec<Piece<CellId>> {
    let mut v = Vec::new();
    pieces(split, &mut v);
    v
}

struct SetResult {
    max_level: u8,
    deepest: u8,
    picks: u32,
    closure_inserts: u32,
    split: usize,
    pieces: usize,
    half_pieces: usize,
    cover: Cover,
    pcs: Vec<Piece<CellId>>,
}

fn evaluate(
    split: &BTreeSet<CellId>,
    max_level: u8,
    picks: u32,
    closure_inserts: u32,
) -> SetResult {
    let t = topo();
    let h = hier();
    let pcs = pcs_of(split);
    let deepest = pcs.iter().map(|p| p.cell.level()).max().unwrap_or(0);
    let half_pieces = pcs.iter().filter(|p| p.variant >= 2).count();
    let cover = cover(t, h, &pcs, max_level);
    SetResult {
        max_level,
        deepest,
        picks,
        closure_inserts,
        split: split.len(),
        pieces: pcs.len(),
        half_pieces,
        cover,
        pcs,
    }
}

fn random_set(seed: u64, max_level: u8, mode: Closure) -> SetResult {
    let t = topo();
    let mut rng = Rng(seed);
    let mut split: BTreeSet<CellId> = BTreeSet::new();
    let roots: Vec<CellId> = CellId::level_iter(0).collect();
    if seed.is_multiple_of(2) {
        split.extend(roots.iter().copied());
    } else {
        split.insert(roots[rng.below(12)]);
    }
    let mut closure_inserts = close_mode(t, &mut split, mode);
    let picks = 6 + u32::try_from(rng.below(30)).expect("picks");
    for _ in 0..picks {
        let cand: Vec<CellId> = pcs_of(&split)
            .iter()
            .map(|p| p.cell)
            .filter(|c| c.level() < max_level)
            .collect();
        if cand.is_empty() {
            break;
        }
        let pool: Vec<CellId> = if rng.below(2) == 0 {
            let deepest = cand.iter().map(|c| c.level()).max().unwrap_or(0);
            cand.iter()
                .copied()
                .filter(|c| c.level() == deepest)
                .collect()
        } else {
            cand
        };
        let x = pool[rng.below(pool.len())];
        split.insert(x);
        closure_inserts += close_mode(t, &mut split, mode);
    }
    evaluate(&split, max_level, picks, closure_inserts)
}

/// Distance rule: split every cell whose centre is within an angle that halves per level of the
/// pentagon `v` (levels `0..max_level`), then close.
fn pentagon_set(v: CellId, max_level: u8) -> SetResult {
    let vd: DVec3 = cell_center_dir(v, 2);
    let mut split: BTreeSet<CellId> = BTreeSet::new();
    for level in 0..max_level {
        let ang = 1.6 / f64::from(1u32 << level);
        let cos_t = ang.cos();
        for c in CellId::level_iter(level) {
            if cell_center_dir(c, 2).dot(vd) >= cos_t {
                split.insert(c);
            }
        }
    }
    let ins = close(&mut split);
    evaluate(&split, max_level, 0, ins)
}

fn results(mode: Closure) -> &'static Vec<SetResult> {
    static NONE: OnceLock<Vec<SetResult>> = OnceLock::new();
    static PARENTS: OnceLock<Vec<SetResult>> = OnceLock::new();
    static FULL: OnceLock<Vec<SetResult>> = OnceLock::new();
    let cell = match mode {
        Closure::None => &NONE,
        Closure::ParentsOnly => &PARENTS,
        Closure::Full => &FULL,
    };
    cell.get_or_init(|| {
        let mut out = Vec::new();
        for s in 0..30u64 {
            out.push(random_set(0x5EED_0000 + s, 4, mode));
        }
        for s in 0..10u64 {
            out.push(random_set(0x5EED_1000 + s, 5, mode));
        }
        out
    })
}

fn pentagon_results() -> &'static Vec<SetResult> {
    static R: OnceLock<Vec<SetResult>> = OnceLock::new();
    R.get_or_init(|| {
        CellId::level_iter(0)
            .map(|v| pentagon_set(v, 4))
            .collect::<Vec<_>>()
    })
}

fn summarize(label: &str, rs: &[SetResult]) {
    let samples: usize = rs.iter().map(|r| r.cover.samples).sum();
    let zero: usize = rs.iter().map(|r| r.cover.zero).sum();
    let multi: usize = rs.iter().map(|r| r.cover.multi).sum();
    let bad_sets = rs
        .iter()
        .filter(|r| r.cover.zero + r.cover.multi > 0)
        .count();
    let max_cover = rs.iter().map(|r| r.cover.max_cover).max().unwrap_or(0);
    let max_jump = rs.iter().map(|r| r.cover.max_jump).max().unwrap_or(0);
    let jump2_sets = rs.iter().filter(|r| r.cover.max_jump >= 2).count();
    let reached = rs.iter().filter(|r| r.deepest == r.max_level).count();
    let picks: u32 = rs.iter().map(|r| r.picks).sum();
    let ins: u32 = rs.iter().map(|r| r.closure_inserts).sum();
    let splits: usize = rs.iter().map(|r| r.split).sum();
    let npieces: usize = rs.iter().map(|r| r.pieces).sum();
    let halves: usize = rs.iter().map(|r| r.half_pieces).sum();
    let mut jumps: BTreeMap<u8, usize> = BTreeMap::new();
    for r in rs {
        for (d, n) in &r.cover.jump_pairs {
            *jumps.entry(*d).or_insert(0) += n;
        }
    }
    println!(
        "HEX-02 [{label}] sets={} samples(atoms)={samples} uncovered={zero} multi_covered={multi} bad_sets={bad_sets} max_cover={max_cover}",
        rs.len()
    );
    println!(
        "HEX-02 [{label}] max_jump={max_jump} sets_with_jump>=2={jump2_sets} adjacent-piece pairs by level diff={jumps:?}"
    );
    let ratio = if picks == 0 {
        0.0
    } else {
        f64::from(ins) / f64::from(picks)
    };
    println!(
        "HEX-02 [{label}] sets reaching max level={reached}/{} pieces={npieces} half_pieces={halves} splits={splits} random_picks={picks} closure_inserts={ins} forced-split ratio={ratio:.2} (closure inserts per random pick)",
        rs.len()
    );
}

// ---------------------------------------------------------------- tests

#[test]
fn coverage_exactly_once() {
    let t = topo();
    let h = hier();

    // Bookkeeping: atom hierarchy is a partition and equals the independently enumerated atoms.
    for level in 0..=TOPO_LEVELS {
        let direct = t.atoms_direct(level);
        let hier_set: BTreeSet<Atom> = h.atoms[usize::from(level)].iter().copied().collect();
        assert_eq!(
            h.atoms[usize::from(level)].len(),
            120 * 4usize.pow(u32::from(level)),
            "atom count at L{level}"
        );
        assert_eq!(
            hier_set, direct,
            "descendant atoms != all atoms at L{level}"
        );
    }
    // A sector (ring index k) must be the two atoms toward ring[k-1] and ring[k+1] of the ring, the
    // layout CellFrame uses (sector k spans the lattice triangles on either side of ring[k]).
    for level in 0..=3u8 {
        for c in CellId::level_iter(level) {
            let r = ring(c);
            let m = r.len();
            for k in 0..m {
                let common: BTreeSet<CellId> = t.common(c, r[k]).into_iter().collect();
                let expect: BTreeSet<CellId> =
                    [r[(k + m - 1) % m], r[(k + 1) % m]].into_iter().collect();
                assert_eq!(common, expect, "sector {k} of {c:?}");
            }
        }
    }

    let rs = results(Closure::Full);
    summarize("closed+balanced", rs);
    let prs = pentagon_results();
    summarize("pentagon distance rule", prs);
    for (n, r) in rs.iter().chain(prs.iter()).enumerate() {
        assert_eq!(
            (r.cover.zero, r.cover.multi),
            (0, 0),
            "set {n} (maxL {}): coverage != 1",
            r.max_level
        );
        assert_eq!(r.cover.max_cover, 1);
    }
    assert!(
        rs.iter().filter(|r| r.deepest == r.max_level).count() >= rs.len() / 2,
        "test power: too few sets reach their maximum level"
    );
    assert!(
        prs.iter().all(|r| r.deepest == r.max_level),
        "pentagon sets must reach their maximum level"
    );
    assert!(rs.iter().any(|r| r.half_pieces > 0), "no half pieces drawn");
    let total: usize = rs.iter().chain(prs.iter()).map(|r| r.cover.samples).sum();
    println!(
        "HEX-02 TOTAL: {} sets, {total} atom samples checked, 0 violations",
        rs.len() + prs.len()
    );
}

#[test]
fn level_jump_is_at_most_one() {
    let rs = results(Closure::Full);
    let prs = pentagon_results();
    let all = rs.iter().chain(prs.iter());
    let max_jump = all.clone().map(|r| r.cover.max_jump).max().unwrap_or(0);
    let pairs1: usize = all
        .clone()
        .map(|r| r.cover.jump_pairs.get(&1).copied().unwrap_or(0))
        .sum();
    let pairs2: usize = all
        .map(|r| r.cover.jump_pairs.get(&2).copied().unwrap_or(0))
        .sum();
    println!(
        "HEX-02 balance (full closure): max jump {max_jump}, level-diff-1 adjacencies {pairs1}, level-diff-2 adjacencies {pairs2}"
    );
    assert!(pairs1 > 0, "no mixed-level borders were exercised");
    assert_eq!(pairs2, 0, "level jumps of 2 present");
    assert_eq!(max_jump, 1, "max level jump across drawn-piece borders");
}

#[test]
fn negative_control_no_closure_detected() {
    let none = results(Closure::None);
    summarize("no closure", none);
    let cov_bad = none
        .iter()
        .filter(|r| r.cover.zero + r.cover.multi > 0)
        .count();
    let jump_bad = none.iter().filter(|r| r.cover.max_jump >= 2).count();
    println!(
        "HEX-02 negative control (no closure): sets with coverage != 1: {cov_bad}, sets with jump >= 2: {jump_bad}"
    );
    assert!(
        cov_bad + jump_bad > 0,
        "negative control: the checker found no violation without closure"
    );

    // Finding (informational): parents-only closure, i.e. without the neighbour rule.
    let po = results(Closure::ParentsOnly);
    summarize("parents-only closure", po);

    // Power of the jump detector: a synthetic exact cover that is NOT balanced. Replace one whole
    // drawn cell X by its level+2 descendant atoms (one piece per atom). Coverage stays exactly 1,
    // so any detected jump comes from the level difference alone and must be 2.
    let t = topo();
    let h = hier();
    let base = &results(Closure::Full)[0];
    let m = base.max_level;
    let vi = base
        .pcs
        .iter()
        .position(|p| p.cell.level() + 2 <= m && p.variant == VARIANT_FULL_HEX)
        .expect("a whole drawn hex with room for two more levels");
    let mut raw = raw_pieces(t, h, &base.pcs);
    let (vlevel, vatoms) = raw.remove(vi);
    let mut cur: Vec<u32> = vatoms;
    for l in 0..2 {
        cur = cur
            .iter()
            .flat_map(|i| h.kids[usize::from(vlevel) + l][*i as usize])
            .collect();
    }
    for a in cur {
        raw.push((vlevel + 2, vec![a]));
    }
    let synth = cover_raw(h, &raw, m);
    println!(
        "HEX-02 jump-detector control: whole cell replaced by its L+2 atoms: uncovered={} multi={} max_jump={} pairs={:?}",
        synth.zero, synth.multi, synth.max_jump, synth.jump_pairs
    );
    assert_eq!((synth.zero, synth.multi), (0, 0));
    assert_eq!(
        synth.max_jump, 2,
        "negative control: the jump detector must see a level jump of 2"
    );
}

#[test]
fn negative_control_dropped_half_detected() {
    let t = topo();
    let h = hier();
    let rs = results(Closure::Full);
    let mut detected = 0usize;
    let mut tried = 0usize;
    for r in rs {
        let Some(i) = r.pcs.iter().position(|p| p.variant >= 2) else {
            continue;
        };
        let mut pcs = r.pcs.clone();
        pcs.remove(i);
        let c = cover(t, h, &pcs, r.max_level);
        tried += 1;
        if c.zero > 0 {
            detected += 1;
        }
    }
    println!("HEX-02 negative control (dropped half): detected in {detected}/{tried} sets");
    assert!(tried > 0);
    assert_eq!(detected, tried, "a dropped half must leave a hole");
}

#[test]
fn negative_control_duplicate_detected() {
    let t = topo();
    let h = hier();
    let rs = results(Closure::Full);
    let mut detected = 0usize;
    let mut tried = 0usize;
    for r in rs {
        // The parent of a drawn child is split, so adding it whole double-covers its children.
        let Some(child) = r.pcs.iter().find(|p| p.cell.level() > 0) else {
            continue;
        };
        let parent = t.pars[&child.cell][0];
        let mut pcs = r.pcs.clone();
        pcs.push(Piece {
            cell: parent,
            variant: if is_pentagon(parent) {
                VARIANT_FULL_PENTAGON
            } else {
                VARIANT_FULL_HEX
            },
        });
        let c = cover(t, h, &pcs, r.max_level);
        tried += 1;
        if c.multi > 0 && c.max_cover >= 2 {
            detected += 1;
        }
    }
    println!("HEX-02 negative control (duplicate parent): detected in {detected}/{tried} sets");
    assert!(tried > 0);
    assert_eq!(detected, tried, "a duplicated piece must double-cover");
}

#[test]
fn pentagons_never_halved() {
    let t = topo();
    // Structural: every pentagon cell below level 0 is the centre child of exactly one parent.
    let mut checked = 0usize;
    for level in 1..=TOPO_LEVELS {
        for c in CellId::level_iter(level) {
            if is_pentagon(c) {
                let ps = &t.pars[&c];
                assert_eq!(ps.len(), 1, "pentagon {c:?} is an edge child");
                assert_eq!(t.kids[&ps[0]][0], c, "pentagon {c:?} is not a centre child");
                assert_eq!(t.nbrs[&c].len(), 5);
                checked += 1;
            }
        }
    }
    let mut partial = 0usize;
    let mut seen = 0usize;
    for r in [Closure::None, Closure::ParentsOnly, Closure::Full]
        .iter()
        .flat_map(|m| results(*m).iter())
        .chain(pentagon_results().iter())
    {
        for p in &r.pcs {
            if is_pentagon(p.cell) {
                seen += 1;
                if p.variant != VARIANT_FULL_PENTAGON {
                    partial += 1;
                }
            }
        }
    }
    println!(
        "HEX-02 pentagons: {checked} pentagon cells L1..L{TOPO_LEVELS} all centre children; {seen} pentagon pieces drawn, partial: {partial}"
    );
    assert!(seen > 0);
    assert_eq!(partial, 0);
}

// ---------------------------------------------------------------- coverage of real selections

/// Coverage of a real selection on a disc around the eye, checked exactly on atoms.
///
/// The global atom hierarchy above only reaches `TOPO_LEVELS`; selections reach deeper, so the
/// atoms are generated lazily from the production adjacency, top-down from the 120 level-0 atoms,
/// and only inside the disc. Every atom of the checked level `m` inside the disc must be covered by
/// exactly one piece (counting the pieces of every coarser level that contain it), and adjacent
/// atoms of different pieces may differ by one level at most.
mod local {
    use super::*;
    use planetary_core::HexSphere;
    use planetary_core::Topology;
    use planetary_core::lod::{DEFAULT_KAPPA, LodModel, LodParams, View};
    use planetary_core::topology::select::select_cells;
    use planetary_core::{PlanetTerrain, TerrainParams};

    fn common(a: CellId, b: CellId) -> Vec<CellId> {
        let nbb = nb(b);
        let out: Vec<CellId> = nb(a).into_iter().filter(|x| nbb.contains(x)).collect();
        assert_eq!(out.len(), 2, "{a:?} {b:?} must share exactly 2 neighbours");
        out
    }

    fn edge_child(a: CellId, b: CellId) -> CellId {
        let mut kids = Vec::new();
        children(a, &mut kids);
        kids.into_iter()
            .skip(1)
            .find(|x| {
                let mut p = Vec::new();
                parents(*x, &mut p);
                p.len() == 2 && p.contains(&a) && p.contains(&b)
            })
            .unwrap_or_else(|| panic!("no edge child of {a:?} {b:?}"))
    }

    fn atom_children([a, b, c]: Atom) -> [Atom; 4] {
        let mut kids = Vec::new();
        children(a, &mut kids);
        let af = kids[0];
        let (mab, mac, mbc) = (edge_child(a, b), edge_child(a, c), edge_child(b, c));
        [
            [af, mab, mac],
            [mab, af, mac],
            [mab, mac, af],
            [mab, mac, mbc],
        ]
    }

    /// Atoms of a piece (the atoms of its drawn sectors).
    fn piece_atoms(p: &Piece<CellId>) -> Vec<Atom> {
        let x = p.cell;
        let r = ring(x);
        let mut v = Vec::new();
        for k in variant_sectors(p.variant) {
            let n = r[usize::from(*k)];
            for q in common(x, n) {
                v.push([x, n, q]);
            }
        }
        v
    }

    pub struct Local {
        pub atoms: usize,
        pub zero: usize,
        pub multi: usize,
        pub max_jump: u8,
        pub jump_pairs: BTreeMap<u8, usize>,
    }

    fn spacing(level: u8) -> f64 {
        1.2 / f64::from(1u32 << level)
    }

    fn angle(a: CellId, eye_dir: DVec3) -> f64 {
        cell_center_dir(a, 2).dot(eye_dir).clamp(-1.0, 1.0).acos()
    }

    pub fn check(pcs: &[Piece<CellId>], eye_dir: DVec3, disc: f64, m: u8) -> Local {
        let mut direct: BTreeMap<Atom, (u32, u32)> = BTreeMap::new();
        let mut plevel = Vec::new();
        for (pid, p) in pcs.iter().enumerate() {
            plevel.push(p.cell.level());
            for a in piece_atoms(p) {
                let e = direct.entry(a).or_insert((0, 0));
                e.0 += 1;
                e.1 = u32::try_from(pid).expect("piece id");
            }
        }
        let mut level0: BTreeSet<Atom> = BTreeSet::new();
        for p in CellId::level_iter(0) {
            for n in nb(p) {
                for q in common(p, n) {
                    level0.insert([p, n, q]);
                }
            }
        }
        assert_eq!(level0.len(), 120);
        let mut fine: BTreeMap<Atom, (u32, u32)> = BTreeMap::new();
        let mut stack: Vec<(Atom, u8, u32, u32)> = level0
            .into_iter()
            .map(|a| (a, 0u8, 0u32, u32::MAX))
            .collect();
        while let Some((a, level, inh, ipid)) = stack.pop() {
            if angle(a[0], eye_dir) > disc + 2.0 * spacing(level) {
                continue;
            }
            let (dc, dp) = direct.get(&a).copied().unwrap_or((0, 0));
            let cnt = inh + dc;
            let pid = if dc > 0 { dp } else { ipid };
            if level == m {
                fine.insert(a, (cnt, pid));
            } else {
                for ch in atom_children(a) {
                    stack.push((ch, level + 1, cnt, pid));
                }
            }
        }
        // Only atoms whose own cell is inside the disc are judged (the rim is a sampling margin).
        let mut r = Local {
            atoms: 0,
            zero: 0,
            multi: 0,
            max_jump: 0,
            jump_pairs: BTreeMap::new(),
        };
        for (a, (cnt, pid)) in &fine {
            if angle(a[0], eye_dir) > disc {
                continue;
            }
            r.atoms += 1;
            if *cnt == 0 {
                r.zero += 1;
            }
            if *cnt > 1 {
                r.multi += 1;
            }
            let [p, n, q] = *a;
            for b in [[n, p, q], [p, q, n]] {
                let Some((c2, pid2)) = fine.get(&b) else {
                    continue;
                };
                if *cnt != 1 || *c2 != 1 || pid == pid2 || b < *a {
                    continue;
                }
                let d = plevel[*pid as usize].abs_diff(plevel[*pid2 as usize]);
                *r.jump_pairs.entry(d).or_insert(0) += 1;
                r.max_jump = r.max_jump.max(d);
            }
        }
        r
    }

    fn views(radius: f64, alt: f64) -> Vec<(String, DVec3)> {
        let roots: Vec<CellId> = CellId::level_iter(0).collect();
        let mut out: Vec<(String, DVec3)> = roots
            .iter()
            .enumerate()
            .map(|(i, r)| (format!("pentagon{i}"), cell_center_dir(*r, 2)))
            .collect();
        let mut edges = Vec::new();
        for (i, a) in roots.iter().enumerate() {
            for b in nb(*a) {
                if *a < b {
                    edges.push((i, *a, b));
                }
            }
        }
        assert_eq!(edges.len(), 30);
        for k in 0..18usize {
            let (i, a, b) = edges[k * 30 / 18];
            let (da, db) = (cell_center_dir(a, 2), cell_center_dir(b, 2));
            let s = DVec3::new(da.x + db.x, da.y + db.y, da.z + db.z);
            let l = s.dot(s).sqrt();
            out.push((
                format!("edge{k}_from{i}"),
                DVec3::new(s.x / l, s.y / l, s.z / l),
            ));
        }
        out.into_iter()
            .map(|(n, d)| {
                let s = radius + alt;
                (n, DVec3::new(d.x * s, d.y * s, d.z * s))
            })
            .collect()
    }

    /// Returns (views, atoms, violations, max jump, level-diff-1 pairs). With `drop_half` one half
    /// piece is removed from each selection (negative control).
    pub fn run(label: &str, drop_half: bool) -> (usize, usize, usize, u8, usize) {
        let hex = HexSphere::new(16).expect("hex");
        let terrain =
            PlanetTerrain::new(0x0007_5EED_2026_0001, TerrainParams::default(), 16).expect("t");
        let model = LodModel::for_topology(&terrain, DEFAULT_KAPPA, hex.sample_angle0());
        let params = LodParams::default();
        let (mut atoms, mut bad, mut views_n, mut max_jump, mut pairs1) = (0, 0, 0, 0u8, 0usize);
        for (name, eye) in
            views(model.radius, 1000.0)
                .into_iter()
                .take(if drop_half { 12 } else { usize::MAX })
        {
            let view = View {
                eye,
                proj_scale: 720.0 / (2.0 * (1.0 / 3.0f64.sqrt())),
                frustum: None,
            };
            let sel = select_cells(&hex, &model, &params, &view, &BTreeSet::new(), &|_| None);
            let mut pcs = sel.pieces.clone();
            let deepest = pcs.iter().map(|p| p.cell.level()).max().unwrap_or(0);
            let l = (eye.x * eye.x + eye.y * eye.y + eye.z * eye.z).sqrt();
            let dir = DVec3::new(eye.x / l, eye.y / l, eye.z / l);
            if drop_half {
                // The half piece closest to the eye (inside the checked disc).
                let Some(i) = (0..pcs.len())
                    .filter(|i| pcs[*i].variant >= 2)
                    .min_by(|a, b| angle(pcs[*a].cell, dir).total_cmp(&angle(pcs[*b].cell, dir)))
                else {
                    continue;
                };
                pcs.remove(i);
            }
            let disc = 3.0 * spacing(deepest);
            let r = check(&pcs, dir, disc, deepest + 2);
            println!(
                "HEX-02 {label} {name}: pieces {} deepest {deepest} checked L{} atoms {} zero {} multi {} max jump {} pairs {:?}",
                pcs.len(),
                deepest + 2,
                r.atoms,
                r.zero,
                r.multi,
                r.max_jump,
                r.jump_pairs
            );
            assert!(r.atoms > 0);
            atoms += r.atoms;
            bad += r.zero + r.multi;
            views_n += 1;
            max_jump = max_jump.max(r.max_jump);
            pairs1 += r.jump_pairs.get(&1).copied().unwrap_or(0);
        }
        println!(
            "HEX-02 {label} TOTAL: {views_n} views, {atoms} atoms, {bad} violations, max jump {max_jump}, level-diff-1 pairs {pairs1}"
        );
        (views_n, atoms, bad, max_jump, pairs1)
    }
}

#[test]
fn coverage_of_selected_cuts() {
    let (views, atoms, bad, max_jump, pairs1) = local::run("selected cuts", false);
    assert_eq!(views, 30);
    assert!(atoms > 0);
    assert_eq!(bad, 0, "coverage violations in real selections");
    assert!(max_jump <= 1, "level jump {max_jump} in real selections");
    assert!(pairs1 > 0, "no mixed-level borders were exercised");
}

#[test]
fn selected_cut_negative_control_dropped_half_detected() {
    let (views, _, bad, _, _) = local::run("dropped half", true);
    assert!(views > 0, "no selection drew a half piece");
    assert!(bad > 0, "dropping a half piece must leave a hole");
}
