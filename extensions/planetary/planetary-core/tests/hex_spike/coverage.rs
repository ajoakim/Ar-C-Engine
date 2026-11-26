//! S3: half-cell piece coverage and balance on the SPHERE (pentagons and face seams included).
//!
//! Method (purely combinatorial, integers + ordered maps). A level-L hex cell has 6 sectors (5 for
//! a pentagon); sector `(X, n)` is the one whose bisector points to lattice neighbour `n`. Cutting
//! every level-L lattice triangle `T = (p, n, q)` along its medians gives 6 ATOMS per triangle,
//! identified by the ordered triple `(p, n, q)` (vertex, edge end, third vertex). A sector `(X, n)`
//! is exactly the two atoms `(X, n, q1)`, `(X, n, q2)` (q1, q2 = the two common neighbours of X and
//! n). Atoms nest dyadically: a coarse atom `(A, B, C)` is the union of the 4 fine atoms
//! `(Af, mAB, mAC)`, `(mAB, Af, mAC)`, `(mAB, mAC, Af)`, `(mAB, mAC, mBC)` (checked here both as a
//! bookkeeping partition and geometrically). Coverage of a drawn piece set is then counted exactly
//! on the atoms of the finest level (no sample ties are possible: atoms partition the sphere).
//!
//! Half cells: an edge child X with parents P1, P2 draws, for a split parent P1, the 3 sectors on
//! P1's side of its corner-centre-corner diagonal: the sector toward the centre child `P1f` of P1
//! and the two sectors adjacent to it around X (= common neighbours of X and `P1f`).

use super::lattice::*;
use super::projection::vertices;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

type Atom = [Cell; 3];

const TOPO_LEVELS: u8 = 5;

// ---------------------------------------------------------------- topology tables

pub struct Topo {
    nbrs: BTreeMap<Cell, Vec<Cell>>,
    kids: BTreeMap<Cell, Vec<Cell>>,
    pars: BTreeMap<Cell, Vec<Cell>>,
}

fn topo() -> &'static Topo {
    static T: OnceLock<Topo> = OnceLock::new();
    T.get_or_init(|| {
        let mut nbrs = BTreeMap::new();
        let mut kids = BTreeMap::new();
        let mut pars = BTreeMap::new();
        for level in 0..=TOPO_LEVELS {
            for c in all_cells(level) {
                nbrs.insert(c, neighbors(c));
                if level > 0 {
                    pars.insert(c, parents(c));
                }
                if level < TOPO_LEVELS {
                    kids.insert(c, children(c));
                }
            }
        }
        Topo { nbrs, kids, pars }
    })
}

impl Topo {
    fn common(&self, a: Cell, b: Cell) -> Vec<Cell> {
        let nb = &self.nbrs[&b];
        let out: Vec<Cell> = self.nbrs[&a]
            .iter()
            .copied()
            .filter(|x| nb.contains(x))
            .collect();
        assert_eq!(out.len(), 2, "{a:?} {b:?} must share exactly 2 neighbours");
        out
    }

    fn edge_child(&self, a: Cell, b: Cell) -> Cell {
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
        for p in all_cells(level) {
            for n in &self.nbrs[&p] {
                for q in self.common(p, *n) {
                    out.insert([p, *n, q]);
                }
            }
        }
        out
    }
}

pub struct Hier {
    atoms: Vec<Vec<Atom>>,
    index: Vec<BTreeMap<Atom, u32>>,
    kids: Vec<Vec<[u32; 4]>>,
    /// per level: the two atoms sharing a border with each atom (across the hex edge and across a
    /// sector boundary of the same cell)
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
            let m: BTreeMap<Atom, u32> =
                v.iter().enumerate().map(|(i, a)| (*a, i as u32)).collect();
            assert_eq!(m.len(), v.len(), "duplicate atoms");
            m
        };
        index.push(idx_of(&atoms[0]));
        for level in 0..TOPO_LEVELS {
            let mut next: Vec<Atom> = Vec::new();
            let mut k: Vec<[u32; 4]> = Vec::new();
            for a in &atoms[usize::from(level)] {
                let ch = t.atom_children(*a);
                let base = next.len() as u32;
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

// ---------------------------------------------------------------- geometry check of the nesting

fn lin(c: Cell) -> [f64; 3] {
    let fp = point_of(c);
    let v = vertices();
    let fv = fp.verts();
    let mut p = [0.0f64; 3];
    for k in 0..3 {
        let wf = fp.w[k] as f64;
        for (pc, vc) in p.iter_mut().zip(v[usize::from(fv[k])]) {
            *pc += wf * vc;
        }
    }
    let n = fp.n as f64;
    p.map(|x| x / n)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dotp(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn crossp(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Geometric triangle (vertex, edge midpoint, triangle centroid) of an atom in the face plane.
fn atom_tri(a: Atom) -> [[f64; 3]; 3] {
    let (p, n, q) = (lin(a[0]), lin(a[1]), lin(a[2]));
    let m = [
        (p[0] + n[0]) / 2.0,
        (p[1] + n[1]) / 2.0,
        (p[2] + n[2]) / 2.0,
    ];
    let c = [
        (p[0] + n[0] + q[0]) / 3.0,
        (p[1] + n[1] + q[1]) / 3.0,
        (p[2] + n[2] + q[2]) / 3.0,
    ];
    [p, m, c]
}

fn area(t: [[f64; 3]; 3]) -> f64 {
    let x = crossp(sub(t[1], t[0]), sub(t[2], t[0]));
    dotp(x, x).sqrt() / 2.0
}

fn inside(t: [[f64; 3]; 3], x: [f64; 3]) -> bool {
    let u = sub(t[1], t[0]);
    let v = sub(t[2], t[0]);
    let w = sub(x, t[0]);
    let (d00, d01, d11) = (dotp(u, u), dotp(u, v), dotp(v, v));
    let (d20, d21) = (dotp(w, u), dotp(w, v));
    let den = d00 * d11 - d01 * d01;
    let bv = (d11 * d20 - d01 * d21) / den;
    let bw = (d00 * d21 - d01 * d20) / den;
    let bu = 1.0 - bv - bw;
    bu >= -1e-9 && bv >= -1e-9 && bw >= -1e-9
}

// ---------------------------------------------------------------- pieces and coverage

fn pieces(t: &Topo, split: &BTreeSet<Cell>) -> BTreeMap<Cell, BTreeSet<Cell>> {
    let mut out: BTreeMap<Cell, BTreeSet<Cell>> = BTreeMap::new();
    for r in all_cells(0) {
        if !split.contains(&r) {
            out.entry(r).or_default().extend(t.nbrs[&r].iter().copied());
        }
    }
    for p in split {
        let ks = &t.kids[p];
        let pf = ks[0];
        for (idx, x) in ks.iter().enumerate() {
            if split.contains(x) {
                continue;
            }
            let e = out.entry(*x).or_default();
            if idx == 0 {
                e.extend(t.nbrs[x].iter().copied());
            } else {
                e.insert(pf);
                e.extend(t.common(*x, pf));
            }
        }
    }
    out
}

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

fn raw_pieces(t: &Topo, h: &Hier, pcs: &BTreeMap<Cell, BTreeSet<Cell>>) -> Vec<RawPiece> {
    pcs.iter()
        .map(|(x, secs)| {
            let l = usize::from(x.level);
            let mut v = Vec::new();
            for n in secs {
                for q in t.common(*x, *n) {
                    v.push(h.index[l][&[*x, *n, q]]);
                }
            }
            (x.level, v)
        })
        .collect()
}

fn cover(t: &Topo, h: &Hier, pcs: &BTreeMap<Cell, BTreeSet<Cell>>, m: u8) -> Cover {
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
            dpid[l][*i as usize] = pid as u32;
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

// ---------------------------------------------------------------- random split sets

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
pub enum Closure {
    /// no closure at all (negative control)
    None,
    /// only "all parents of a split cell are split" (negative control for balance)
    ParentsOnly,
    /// parents plus all neighbours of the parents (the hex balance rule)
    Full,
}

/// Returns the number of cells inserted by the closure.
fn close(t: &Topo, split: &mut BTreeSet<Cell>, mode: Closure) -> usize {
    if mode == Closure::None {
        return 0;
    }
    let before = split.len();
    let mut work: Vec<Cell> = split.iter().copied().collect();
    while let Some(x) = work.pop() {
        if x.level == 0 {
            continue;
        }
        for p in &t.pars[&x] {
            if split.insert(*p) {
                work.push(*p);
            }
            if mode == Closure::Full {
                for nb in &t.nbrs[p] {
                    if split.insert(*nb) {
                        work.push(*nb);
                    }
                }
            }
        }
    }
    split.len() - before
}

struct SetResult {
    max_level: u8,
    deepest: u8,
    picks: usize,
    closure_inserts: usize,
    split: usize,
    pieces: usize,
    half_pieces: usize,
    pentagon_partial: usize,
    cover: Cover,
    pcs: BTreeMap<Cell, BTreeSet<Cell>>,
}

fn random_set(seed: u64, max_level: u8, mode: Closure) -> SetResult {
    let t = topo();
    let h = hier();
    let mut rng = Rng(seed);
    let mut split: BTreeSet<Cell> = BTreeSet::new();
    let roots = all_cells(0);
    if seed.is_multiple_of(2) {
        split.extend(roots.iter().copied());
    } else {
        split.insert(roots[rng.below(12)]);
    }
    let mut closure_inserts = close(t, &mut split, mode);
    let picks = 6 + rng.below(30);
    for _ in 0..picks {
        let pcs = pieces(t, &split);
        let cand: Vec<Cell> = pcs
            .keys()
            .copied()
            .filter(|c| c.level < max_level)
            .collect();
        if cand.is_empty() {
            break;
        }
        let pool: Vec<Cell> = if rng.below(2) == 0 {
            let deepest = cand.iter().map(|c| c.level).max().unwrap_or(0);
            cand.iter()
                .copied()
                .filter(|c| c.level == deepest)
                .collect()
        } else {
            cand
        };
        let x = pool[rng.below(pool.len())];
        split.insert(x);
        closure_inserts += close(t, &mut split, mode);
    }
    let pcs = pieces(t, &split);
    let deepest = pcs.keys().map(|c| c.level).max().unwrap_or(0);
    let half_pieces = pcs
        .iter()
        .filter(|(x, s)| s.len() < t.nbrs[x].len())
        .count();
    let pentagon_partial = pcs
        .iter()
        .filter(|(x, s)| vertex_of(**x).is_some() && s.len() != 5)
        .count();
    let cover = cover(t, h, &pcs, max_level);
    SetResult {
        max_level,
        deepest,
        picks,
        closure_inserts,
        split: split.len(),
        pieces: pcs.len(),
        half_pieces,
        pentagon_partial,
        cover,
        pcs,
    }
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
    let picks: usize = rs.iter().map(|r| r.picks).sum();
    let ins: usize = rs.iter().map(|r| r.closure_inserts).sum();
    let splits: usize = rs.iter().map(|r| r.split).sum();
    let pieces: usize = rs.iter().map(|r| r.pieces).sum();
    let halves: usize = rs.iter().map(|r| r.half_pieces).sum();
    let mut jumps: BTreeMap<u8, usize> = BTreeMap::new();
    for r in rs {
        for (d, n) in &r.cover.jump_pairs {
            *jumps.entry(*d).or_insert(0) += n;
        }
    }
    println!(
        "S3 [{label}] sets={} samples(atoms)={samples} uncovered={zero} multi_covered={multi} bad_sets={bad_sets} max_cover={max_cover}",
        rs.len()
    );
    println!(
        "S3 [{label}] max_jump={max_jump} sets_with_jump>=2={jump2_sets} adjacent-piece pairs by level diff={jumps:?}"
    );
    println!(
        "S3 [{label}] sets reaching max level={reached}/{} pieces={pieces} half_pieces={halves} splits={splits} random_picks={picks} closure_inserts={ins} forced-split ratio={:.2} (closure inserts per random pick)",
        rs.len(),
        ins as f64 / picks as f64
    );
}

// ---------------------------------------------------------------- tests

#[test]
fn s3_coverage_exactly_once() {
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
    // Geometry: the 4 fine atoms tile the coarse atom (all vertices inside, areas add up).
    let mut checked = 0usize;
    let mut worst_area_err = 0.0f64;
    for level in 0..=3u8 {
        for (i, a) in h.atoms[usize::from(level)].iter().enumerate() {
            let coarse = atom_tri(*a);
            let mut sum = 0.0;
            for k in h.kids[usize::from(level)][i] {
                let fine = atom_tri(h.atoms[usize::from(level) + 1][k as usize]);
                for v in fine {
                    assert!(inside(coarse, v), "fine atom leaves coarse atom {a:?}");
                }
                sum += area(fine);
            }
            let err = (sum - area(coarse)).abs() / area(coarse);
            worst_area_err = worst_area_err.max(err);
            assert!(err < 1e-9, "area mismatch {err} at {a:?}");
            checked += 1;
        }
    }
    println!(
        "S3 atom nesting: partition ok L0..L{TOPO_LEVELS}; geometric tiling checked on {checked} coarse atoms (worst rel. area error {worst_area_err:.2e})"
    );

    let rs = results(Closure::Full);
    summarize("closed+balanced", rs);
    for (n, r) in rs.iter().enumerate() {
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
    assert!(rs.iter().any(|r| r.half_pieces > 0), "no half pieces drawn");
}

#[test]
fn s3_balance_jump_is_one() {
    let rs = results(Closure::Full);
    let max_jump = rs.iter().map(|r| r.cover.max_jump).max().unwrap_or(0);
    let pairs1: usize = rs
        .iter()
        .map(|r| r.cover.jump_pairs.get(&1).copied().unwrap_or(0))
        .sum();
    println!("S3 balance (full closure): max jump {max_jump}, level-diff-1 adjacencies {pairs1}");
    assert!(pairs1 > 0, "no mixed-level borders were exercised");
    assert_eq!(max_jump, 1, "max level jump across drawn-piece borders");
}

#[test]
fn s3_negative_control_without_closure() {
    let none = results(Closure::None);
    summarize("no closure", none);
    let parents_only = results(Closure::ParentsOnly);
    summarize("parents-only closure", parents_only);
    let cov_bad = none
        .iter()
        .filter(|r| r.cover.zero + r.cover.multi > 0)
        .count();
    let jump_bad = none.iter().filter(|r| r.cover.max_jump >= 2).count();
    println!(
        "S3 negative control (no closure): sets with coverage != 1: {cov_bad}, sets with jump >= 2: {jump_bad}"
    );
    assert!(
        cov_bad + jump_bad > 0,
        "negative control: the checker found no violation without closure"
    );
    let po_cov_bad = parents_only
        .iter()
        .filter(|r| r.cover.zero + r.cover.multi > 0)
        .count();
    let po_jump_bad = parents_only
        .iter()
        .filter(|r| r.cover.max_jump >= 2)
        .count();
    println!(
        "S3 FINDING (parents-only closure, i.e. WITHOUT the neighbour rule): sets with coverage != 1: {po_cov_bad}, sets with jump >= 2: {po_jump_bad} of {}",
        parents_only.len()
    );

    // Stress: many more parents-only sets, to look for any jump of 2 without the neighbour rule.
    let mut stress_sets = 0usize;
    let mut stress_jump2 = 0usize;
    let mut stress_cov = 0usize;
    let mut stress_max = 0u8;
    for s in 0..300u64 {
        let r = random_set(0xBAD0_0000 + s, 4, Closure::ParentsOnly);
        stress_sets += 1;
        stress_max = stress_max.max(r.cover.max_jump);
        if r.cover.max_jump >= 2 {
            stress_jump2 += 1;
        }
        if r.cover.zero + r.cover.multi > 0 {
            stress_cov += 1;
        }
    }
    println!(
        "S3 FINDING stress (parents-only, maxL 4): {stress_sets} sets, coverage violations in {stress_cov}, jump>=2 in {stress_jump2}, max jump {stress_max}"
    );

    // Power of the jump detector: a synthetic exact cover that is NOT balanced. Replace one whole
    // drawn cell X by its level+2 descendant atoms (one piece per atom). Coverage stays exactly 1,
    // so any detected jump comes from the level difference alone and must be 2.
    let t = topo();
    let h = hier();
    let base = &results(Closure::Full)[0];
    let m = base.max_level;
    let (victim, _) = base
        .pcs
        .iter()
        .find(|(x, s)| x.level + 2 <= m && s.len() == t.nbrs[x].len())
        .expect("a whole drawn cell with room for two more levels");
    let mut raw = raw_pieces(t, h, &base.pcs);
    let vi = base.pcs.keys().position(|k| k == victim).unwrap();
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
        "S3 jump-detector control: whole cell {victim:?} replaced by its L+2 atoms: uncovered={} multi={} max_jump={} pairs={:?}",
        synth.zero, synth.multi, synth.max_jump, synth.jump_pairs
    );
    assert_eq!((synth.zero, synth.multi), (0, 0));
    assert_eq!(
        synth.max_jump, 2,
        "negative control: the jump detector must see a level jump of 2"
    );
}

#[test]
fn s3_pentagons_never_halved() {
    let t = topo();
    // Structural: every cell on an icosahedron vertex is the centre child of exactly one parent.
    let mut checked = 0usize;
    for level in 1..=TOPO_LEVELS {
        for c in all_cells(level) {
            if vertex_of(c).is_some() {
                let ps = &t.pars[&c];
                assert_eq!(ps.len(), 1, "pentagon {c:?} is an edge child");
                assert_eq!(t.kids[&ps[0]][0], c, "pentagon {c:?} is not a centre child");
                assert!(vertex_of(ps[0]).is_some());
                assert_eq!(t.nbrs[&c].len(), 5);
                checked += 1;
            }
        }
    }
    // Every edge child has 6 neighbours (never a pentagon).
    for level in 0..TOPO_LEVELS {
        for p in all_cells(level) {
            for x in t.kids[&p].iter().skip(1) {
                assert_eq!(t.nbrs[x].len(), 6, "edge child {x:?} must be a hex");
            }
        }
    }
    let partial: usize = [Closure::None, Closure::ParentsOnly, Closure::Full]
        .iter()
        .flat_map(|m| results(*m).iter())
        .map(|r| r.pentagon_partial)
        .sum();
    println!(
        "S3 pentagons: {checked} pentagon cells L1..L{TOPO_LEVELS} all centre children; partial pentagon pieces across all sets: {partial}"
    );
    assert_eq!(partial, 0);
}
