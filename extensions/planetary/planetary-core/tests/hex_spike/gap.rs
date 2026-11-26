//! S4: skirt gap bound at L/L+1 borders of the half-cell hex design.
//!
//! Geometry (all integer barycentrics on the sample lattice `D = 3 * N * 2^L`, `N = 8`).
//! A level-L lattice edge `P-Q` is shared by two lattice triangles `T1 = (P, Q, m1)` and
//! `T2 = (P, Q, m2)`. The hex edge between P and Q runs from the centroid `c1` of T1 through the
//! edge midpoint `M` to the centroid `c2` of T2; `N + 1` samples with step `(p + q) - 2 m` on each
//! half. The same segment is the corner-centre-corner DIAGONAL of the level-(L+1) edge child
//! `X = mid(P, Q)`, with `2N + 1` samples (the even ones coincide with the coarse ones). So at an
//! L/L+1 border the coarse side is always a hex outer edge (of a whole cell or of a half piece)
//! and the fine side is always a half-cell diagonal; a fine hex outer edge never faces a coarse
//! piece (it would need a fine hex edge on a coarse hex edge, which aperture 4 does not produce).
//!
//! Gap of an odd fine vertex = distance to the coarse chord between its two even neighbours.
//! Skirt depth = `SKIRT_MARGIN * eps(L)` with `eps(L) = kappa * R * a_L`, `a_L` the sample spacing
//! angle (hex circumradius / N) of the COARSE level L.

use super::coverage::{Closure, Rng, TOPO_LEVELS, close, pieces, topo};
use super::lattice::*;
use super::projection::{cell_dir, dir, vertices};
use engine_sdk::DVec3;
use planetary_core::lod::DEFAULT_KAPPA;
use planetary_core::{PlanetTerrain, SKIRT_MARGIN, TerrainParams};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

const N: i64 = 8;
const SEED: u64 = 0x0007_5EED_2026_0001;
/// Split distance factor of the selection rule (in units of the hex circumradius).
const RULE_K: f64 = 2.0;

fn terrain() -> &'static PlanetTerrain {
    static T: OnceLock<PlanetTerrain> = OnceLock::new();
    T.get_or_init(|| PlanetTerrain::new(SEED, TerrainParams::default(), N as u32).expect("terrain"))
}

fn s0() -> f64 {
    (8.0 * std::f64::consts::PI / (10.0 * 3.0f64.sqrt())).sqrt()
}

fn pow2(level: u32) -> f64 {
    (1u64 << level) as f64
}

/// Hex circumradius angle at `level` (radians on the unit sphere).
fn circ_angle(level: u8) -> f64 {
    s0() / (pow2(u32::from(level)) * 3.0f64.sqrt())
}

/// Sample spacing angle `a_L` (circumradius / N).
fn sample_angle(level: u8) -> f64 {
    circ_angle(level) / N as f64
}

fn eps(level: u8) -> f64 {
    DEFAULT_KAPPA * terrain().params().radius * sample_angle(level)
}

fn skirt(level: u8) -> f64 {
    SKIRT_MARGIN * eps(level)
}

fn bits(p: DVec3) -> [u64; 3] {
    [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
}

fn dist(a: DVec3, b: DVec3) -> f64 {
    let (x, y, z) = (a.x - b.x, a.y - b.y, a.z - b.z);
    (x * x + y * y + z * z).sqrt()
}

fn dist_to_segment(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let ab = DVec3::new(b.x - a.x, b.y - a.y, b.z - a.z);
    let ap = DVec3::new(p.x - a.x, p.y - a.y, p.z - a.z);
    let len2 = ab.x * ab.x + ab.y * ab.y + ab.z * ab.z;
    let t = if len2 > 0.0 {
        ((ap.x * ab.x + ap.y * ab.y + ap.z * ab.z) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    dist(
        p,
        DVec3::new(a.x + t * ab.x, a.y + t * ab.y, a.z + t * ab.z),
    )
}

/// Surface position of the sample lattice point `(face, w) / d`.
fn pos(face: u8, w: [i64; 3], d: i64) -> DVec3 {
    assert!(w.iter().all(|x| *x >= 0), "negative weight {w:?}");
    assert_eq!(w[0] + w[1] + w[2], d, "weights must sum to D");
    let v = dir(face, w[0] as u64, w[1] as u64, w[2] as u64, d as u64);
    terrain().surface_point(DVec3::new(v[0], v[1], v[2]))
}

/// A lattice triangle `(p, q, m)` in one face frame, weights in units of `n = 2^level`.
#[derive(Clone, Copy, Debug)]
struct Tri {
    face: u8,
    p: [i64; 3],
    q: [i64; 3],
    m: [i64; 3],
    n: i64,
}

fn tri_of(a: Cell, b: Cell, c: Cell) -> Tri {
    let fps = [point_of(a), point_of(b), point_of(c)];
    let faces_ok: Vec<u8> = (0..20u8)
        .filter(|f| {
            fps.iter().all(|fp| {
                fp.key()
                    .iter()
                    .filter(|(v, _)| *v != 255)
                    .all(|(v, _)| faces()[usize::from(*f)].contains(v))
            })
        })
        .collect();
    assert_eq!(faces_ok.len(), 1, "triangle {a:?} {b:?} {c:?} face");
    let face = faces_ok[0];
    let fv = faces()[usize::from(face)];
    let w_of = |fp: &FacePoint| -> [i64; 3] {
        let mut w = [0i64; 3];
        for (v, x) in fp.key().iter().filter(|(v, _)| *v != 255) {
            let k = fv.iter().position(|u| u == v).expect("vertex in face");
            w[k] = *x;
        }
        w
    };
    Tri {
        face,
        p: w_of(&fps[0]),
        q: w_of(&fps[1]),
        m: w_of(&fps[2]),
        n: fps[0].n,
    }
}

fn add(a: [i64; 3], b: [i64; 3]) -> [i64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [i64; 3], k: i64) -> [i64; 3] {
    a.map(|x| x * k)
}

/// Samples from the triangle centroid `c` to the edge midpoint `M` (inclusive), refined by `r`
/// (units `r * D_L`): `r * N / 2 + 1` points.
fn half(t: &Tri, r: i64) -> Vec<(u8, [i64; 3])> {
    let c = scale(add(add(t.p, t.q), t.m), N * r);
    let step = add(add(t.p, t.q), scale(t.m, -2));
    (0..=(r * N / 2))
        .map(|k| (t.face, add(c, scale(step, k))))
        .collect()
}

/// The hex edge `c1 -> M -> c2` as `r * N + 1` samples; also returns M as computed in both frames.
type Sample = (u8, [i64; 3]);

fn hex_edge(t1: &Tri, t2: &Tri, r: i64) -> (Vec<Sample>, Sample, Sample) {
    let a = half(t1, r);
    let mut b = half(t2, r);
    let m1 = *a.last().expect("half");
    let m2 = *b.last().expect("half");
    b.reverse();
    let mut out = a;
    out.extend(b.into_iter().skip(1));
    (out, m1, m2)
}

#[derive(Clone, Debug)]
struct JStat {
    level: u8,
    whole: bool,
    pentagon: bool,
    crosses_face: bool,
    even_mismatch: usize,
    odd: usize,
    max_ratio: f64,
    sum_ratio: f64,
    max_gap: f64,
    gaps_over_zero: usize,
    over_skirt: usize,
}

fn eval(t1: &Tri, t2: &Tri, level: u8) -> (usize, Vec<f64>) {
    let d = 3 * N * t1.n;
    let (coarse_w, cm1, cm2) = hex_edge(t1, t2, 1);
    let (fine_w, fm1, fm2) = hex_edge(t1, t2, 2);
    assert_eq!(coarse_w.len() as i64, N + 1);
    assert_eq!(fine_w.len() as i64, 2 * N + 1);
    let coarse: Vec<DVec3> = coarse_w.iter().map(|(f, w)| pos(*f, *w, d)).collect();
    let fine: Vec<DVec3> = fine_w.iter().map(|(f, w)| pos(*f, *w, 2 * d)).collect();
    let mut mismatch = 0usize;
    // The edge midpoint is computed from both adjacent faces (it lies on an icosahedron edge when
    // the two triangles are in different faces): both frames must give identical bits.
    if bits(pos(cm1.0, cm1.1, d)) != bits(pos(cm2.0, cm2.1, d)) {
        mismatch += 1;
    }
    if bits(pos(fm1.0, fm1.1, 2 * d)) != bits(pos(fm2.0, fm2.1, 2 * d)) {
        mismatch += 1;
    }
    for (k, c) in coarse.iter().enumerate() {
        if bits(*c) != bits(fine[2 * k]) {
            mismatch += 1;
        }
    }
    let _ = level;
    let gaps = (0..coarse.len() - 1)
        .map(|k| dist_to_segment(fine[2 * k + 1], coarse[k], coarse[k + 1]))
        .collect();
    (mismatch, gaps)
}

fn stat(
    t1: &Tri,
    t2: &Tri,
    level: u8,
    whole: bool,
    pentagon: bool,
    force_depth: Option<f64>,
) -> JStat {
    let (even_mismatch, gaps) = eval(t1, t2, level);
    let depth = force_depth.unwrap_or_else(|| skirt(level));
    let mut s = JStat {
        level,
        whole,
        pentagon,
        crosses_face: t1.face != t2.face,
        even_mismatch,
        odd: gaps.len(),
        max_ratio: 0.0,
        sum_ratio: 0.0,
        max_gap: 0.0,
        gaps_over_zero: 0,
        over_skirt: 0,
    };
    for g in gaps {
        let ratio = g / skirt(level);
        s.max_ratio = s.max_ratio.max(ratio);
        s.sum_ratio += ratio;
        s.max_gap = s.max_gap.max(g);
        if g > 0.0 {
            s.gaps_over_zero += 1;
        }
        if g > depth {
            s.over_skirt += 1;
        }
    }
    s
}

// ---------------------------------------------------------------- selection on the real hierarchy

fn eyes() -> Vec<(String, DVec3)> {
    let v = vertices();
    let t = terrain();
    let mut dirs: Vec<(String, [f64; 3])> = Vec::new();
    let nrm = |p: [f64; 3]| {
        let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        p.map(|x| x / l)
    };
    for (k, p) in v.iter().enumerate() {
        dirs.push((format!("vertex{k}"), nrm(*p)));
    }
    for [a, b] in edges() {
        let (p, q) = (v[usize::from(a)], v[usize::from(b)]);
        dirs.push((
            format!("edge{a}-{b}"),
            nrm([p[0] + q[0], p[1] + q[1], p[2] + q[2]]),
        ));
    }
    let mut rng = Rng(0xE4E5);
    let mut unit = || (rng.next() >> 11) as f64 / (1u64 << 53) as f64;
    for k in 0..8 {
        loop {
            let p = [unit() * 2.0 - 1.0, unit() * 2.0 - 1.0, unit() * 2.0 - 1.0];
            let l2 = p[0] * p[0] + p[1] * p[1] + p[2] * p[2];
            if l2 > 1e-6 && l2 <= 1.0 {
                dirs.push((format!("random{k}"), nrm(p)));
                break;
            }
        }
    }
    let mut out = Vec::new();
    for (name, d) in dirs {
        let dv = DVec3::new(d[0], d[1], d[2]);
        let r = {
            let sp = t.surface_point(dv);
            (sp.x * sp.x + sp.y * sp.y + sp.z * sp.z).sqrt()
        };
        let alts: &[f64] = if name.starts_with("random") {
            &[1000.0]
        } else {
            &[1000.0, 50.0]
        };
        for a in alts {
            out.push((
                format!("{name}@{a}m"),
                DVec3::new(d[0] * (r + a), d[1] * (r + a), d[2] * (r + a)),
            ));
        }
    }
    out
}

struct Real {
    sets: usize,
    junction_total: usize,
    unique: Vec<JStat>,
    skipped_dup: usize,
}

fn real(mode: Closure) -> &'static Real {
    static FULL: OnceLock<Real> = OnceLock::new();
    static PARENTS: OnceLock<Real> = OnceLock::new();
    let cell = if mode == Closure::Full {
        &FULL
    } else {
        &PARENTS
    };
    cell.get_or_init(|| {
        let tp = topo();
        let radius = terrain().params().radius;
        let max_level = TOPO_LEVELS; // pieces up to level 5 => coarse levels 0..=4
        let mut centres: BTreeMap<Cell, [f64; 3]> = BTreeMap::new();
        for level in 0..max_level {
            for c in all_cells(level) {
                centres.insert(c, cell_dir(c));
            }
        }
        let mut seen: BTreeSet<(Cell, Cell)> = BTreeSet::new();
        let mut unique = Vec::new();
        let mut junction_total = 0usize;
        let mut skipped_dup = 0usize;
        let eyes = eyes();
        for (_, eye) in &eyes {
            let mut split: BTreeSet<Cell> = BTreeSet::new();
            for (c, d) in &centres {
                let p = DVec3::new(d[0] * radius, d[1] * radius, d[2] * radius);
                if dist(*eye, p) < RULE_K * radius * circ_angle(c.level) {
                    split.insert(*c);
                }
            }
            close(tp, &mut split, mode);
            let pcs = pieces(tp, &split);
            for (p, secs) in &pcs {
                for q in secs {
                    if !split.contains(q) {
                        continue;
                    }
                    junction_total += 1;
                    if !seen.insert((*p, *q)) {
                        skipped_dup += 1;
                        continue;
                    }
                    let whole = secs.len() == tp.nbrs[p].len();
                    let pentagon = vertex_of(*p).is_some() || vertex_of(*q).is_some();
                    let ms = tp.common(*p, *q);
                    let t1 = tri_of(*p, *q, ms[0]);
                    let t2 = tri_of(*p, *q, ms[1]);
                    unique.push(stat(&t1, &t2, p.level, whole, pentagon, None));
                }
            }
        }
        Real {
            sets: eyes.len(),
            junction_total,
            unique,
            skipped_dup,
        }
    })
}

fn max_of(it: impl Iterator<Item = f64>) -> f64 {
    it.fold(0.0, f64::max)
}

fn report(label: &str, js: &[&JStat]) {
    let odd: usize = js.iter().map(|j| j.odd).sum();
    let mx = max_of(js.iter().map(|j| j.max_ratio));
    let mean = js.iter().map(|j| j.sum_ratio).sum::<f64>() / odd.max(1) as f64;
    let over: usize = js.iter().map(|j| j.over_skirt).sum();
    println!(
        "S4 [{label}] junctions={} odd_vertices={odd} max gap/skirt={mx:.4} mean={mean:.4} over_skirt={over}",
        js.len()
    );
}

fn report_real(mode: Closure) {
    let r = real(mode);
    println!(
        "S4 selection [{mode:?}]: {} eyes (12 vertices, 30 edge midpoints at 1 km and 50 m, 8 random at 1 km), closed ({mode:?} closure), rule K={RULE_K}, N={N}, kappa={DEFAULT_KAPPA}, margin={SKIRT_MARGIN}",
        r.sets
    );
    println!(
        "S4 [{mode:?}] junction occurrences={} unique (P,Q)={} duplicates skipped={}",
        r.junction_total,
        r.unique.len(),
        r.skipped_dup
    );
    let all: Vec<&JStat> = r.unique.iter().collect();
    report(&format!("{mode:?} all"), &all);
    for (name, f) in [
        ("coarse side = whole-cell hex edge", true),
        ("coarse side = half-piece hex edge", false),
    ] {
        let v: Vec<&JStat> = r.unique.iter().filter(|j| j.whole == f).collect();
        report(&format!("{mode:?} {name}"), &v);
    }
    for (name, f) in [("touching pentagon", true), ("hex only", false)] {
        let v: Vec<&JStat> = r.unique.iter().filter(|j| j.pentagon == f).collect();
        report(&format!("{mode:?} {name}"), &v);
    }
    for (name, f) in [
        ("crossing icosahedron edge", true),
        ("inside one face", false),
    ] {
        let v: Vec<&JStat> = r.unique.iter().filter(|j| j.crosses_face == f).collect();
        report(&format!("{mode:?} {name}"), &v);
    }
    for level in 0..TOPO_LEVELS {
        let v: Vec<&JStat> = r.unique.iter().filter(|j| j.level == level).collect();
        if !v.is_empty() {
            report(&format!("{mode:?} coarse level {level}"), &v);
        }
    }
}

// ---------------------------------------------------------------- deep levels (no tables)

/// Random interior junctions at coarse level `level` (3..=19), both triangles in one face.
fn deep(level: u8, count: usize, seed: u64) -> Vec<JStat> {
    let n = 1i64 << level;
    let mut rng = Rng(seed ^ (u64::from(level) << 40));
    let units: [[i64; 3]; 6] = [
        [1, -1, 0],
        [-1, 1, 0],
        [1, 0, -1],
        [-1, 0, 1],
        [0, 1, -1],
        [0, -1, 1],
    ];
    let mut out = Vec::new();
    for _ in 0..count {
        let face = rng.below(20) as u8;
        let (a, b, c) = loop {
            let a = 2 + (rng.next() % (n as u64 - 3)) as i64;
            let b = 2 + (rng.next() % (n as u64 - 3)) as i64;
            let c = n - a - b;
            if c >= 2 {
                break (a, b, c);
            }
        };
        let p = [a, b, c];
        let d = units[rng.below(6)];
        let q = add(p, d);
        // the two unit vectors at 60 degrees from d
        let ms: Vec<[i64; 3]> = units
            .iter()
            .filter(|u| {
                let e = [u[0] - d[0], u[1] - d[1], u[2] - d[2]];
                e[0] * e[0] + e[1] * e[1] + e[2] * e[2] == 2
            })
            .map(|u| add(p, *u))
            .collect();
        assert_eq!(ms.len(), 2);
        let t1 = Tri {
            face,
            p,
            q,
            m: ms[0],
            n,
        };
        let t2 = Tri {
            face,
            p,
            q,
            m: ms[1],
            n,
        };
        out.push(stat(&t1, &t2, level, true, false, None));
    }
    out
}

fn deep_all() -> &'static Vec<(u8, Vec<JStat>)> {
    static D: OnceLock<Vec<(u8, Vec<JStat>)>> = OnceLock::new();
    D.get_or_init(|| (3..=19u8).map(|l| (l, deep(l, 60, 0xDEE9))).collect())
}

// ---------------------------------------------------------------- tests

#[test]
fn s4_even_vertices_bit_identical() {
    for mode in [Closure::Full, Closure::ParentsOnly] {
        report_real(mode);
        let r = real(mode);
        assert!(!r.unique.is_empty(), "no L/L+1 junctions found");
        let mism: usize = r.unique.iter().map(|j| j.even_mismatch).sum();
        let faces_cross = r.unique.iter().filter(|j| j.crosses_face).count();
        let pent = r.unique.iter().filter(|j| j.pentagon).count();
        println!(
            "S4 even-vertex check [{mode:?}]: {} junctions ({faces_cross} cross an icosahedron edge, {pent} touch a pentagon), mismatches={mism}",
            r.unique.len()
        );
        assert_eq!(mism, 0, "even fine vertices must equal coarse bit-exactly");
        assert!(
            faces_cross > 0 && pent > 0,
            "seam / pentagon junctions not exercised"
        );
    }
    let dmism: usize = deep_all()
        .iter()
        .flat_map(|(_, v)| v.iter())
        .map(|j| j.even_mismatch)
        .sum();
    let dn: usize = deep_all().iter().map(|(_, v)| v.len()).sum();
    println!("S4 even-vertex check deep levels 3..=19: {dn} junctions, mismatches={dmism}");
    assert_eq!(dmism, 0);
}

#[test]
fn s4_gap_bounded_by_skirts() {
    for mode in [Closure::Full, Closure::ParentsOnly] {
        report_real(mode);
        let r = real(mode);
        let mx = max_of(r.unique.iter().map(|j| j.max_ratio));
        let over: usize = r.unique.iter().map(|j| j.over_skirt).sum();
        let whole = r.unique.iter().filter(|j| j.whole).count();
        let half = r.unique.len() - whole;
        let whole_mx = max_of(r.unique.iter().filter(|j| j.whole).map(|j| j.max_ratio));
        let half_mx = max_of(r.unique.iter().filter(|j| !j.whole).map(|j| j.max_ratio));
        println!(
            "S4 gap/skirt [{mode:?}] (levels 0..{}): max={mx:.4}; coarse whole-cell hex edges ({whole}): {whole_mx:.4}; coarse half-piece hex edges ({half}): {half_mx:.4}; odd vertices over skirt: {over}",
            TOPO_LEVELS - 1
        );
        assert_eq!(over, 0, "gap exceeds skirt depth");
        assert!(mx <= 1.0);
    }
    // With the full balance rule a half piece never faces finer cells (all neighbours of the
    // split parent are split), so half-piece coarse borders only occur without the neighbour rule.
    let full_half = real(Closure::Full)
        .unique
        .iter()
        .filter(|j| !j.whole)
        .count();
    let po_half = real(Closure::ParentsOnly)
        .unique
        .iter()
        .filter(|j| !j.whole)
        .count();
    println!(
        "S4 coarse half-piece borders: Full closure {full_half}, parents-only closure {po_half}"
    );
    assert_eq!(full_half, 0);
    assert!(
        po_half > 0,
        "half-piece borders must be exercised somewhere"
    );
}

#[test]
fn s4_negative_control_zero_skirt() {
    let r = real(Closure::Full);
    let odd: usize = r.unique.iter().map(|j| j.odd).sum();
    let nonzero: usize = r.unique.iter().map(|j| j.gaps_over_zero).sum();
    // Recompute violations with depth 0 on a subset of junctions (the same (P,Q) set).
    let tp = topo();
    let mut zero_viol = 0usize;
    let mut checked_odd = 0usize;
    let mut checked_nonzero = 0usize;
    let mut pairs: Vec<(Cell, Cell)> = Vec::new();
    for level in 0..3u8 {
        for p in all_cells(level) {
            for q in &tp.nbrs[&p] {
                if p < *q {
                    pairs.push((p, *q));
                }
            }
        }
    }
    for (p, q) in pairs {
        let ms = tp.common(p, q);
        let s = stat(
            &tri_of(p, q, ms[0]),
            &tri_of(p, q, ms[1]),
            p.level,
            true,
            false,
            Some(0.0),
        );
        zero_viol += s.over_skirt;
        checked_odd += s.odd;
        checked_nonzero += s.gaps_over_zero;
    }
    println!(
        "S4 negative control (depth 0): every lattice edge of levels 0..=2 as a junction: {zero_viol} violations of {checked_odd} odd vertices ({checked_nonzero} with non-zero gap); real set has {nonzero}/{odd} odd vertices with non-zero gap"
    );
    assert!(
        zero_viol > 0,
        "check cannot fail: no violation with depth 0"
    );
    assert_eq!(
        zero_viol, checked_nonzero,
        "every non-zero gap must be reported"
    );
    assert_eq!(
        r.unique.iter().map(|j| j.over_skirt).sum::<usize>(),
        0,
        "real depth must report none"
    );
}

#[test]
fn s4_deep_levels_gap_report() {
    println!("S4 deep levels (interior random junctions, N={N}), skirt = 1.5 * 0.25 * R * a_L:");
    let mut worst = 0.0f64;
    let mut over_total = 0usize;
    for (level, v) in deep_all() {
        let odd: usize = v.iter().map(|j| j.odd).sum();
        let mx = max_of(v.iter().map(|j| j.max_ratio));
        let mean = v.iter().map(|j| j.sum_ratio).sum::<f64>() / odd as f64;
        let over: usize = v.iter().map(|j| j.over_skirt).sum();
        let max_gap = max_of(v.iter().map(|j| j.max_gap));
        println!(
            "S4 deep L{level:>2}: junctions={} odd={odd} sample_spacing={:.2} m skirt={:.3} m max_gap={:.4} m max ratio={mx:.4} mean ratio={mean:.4} over_skirt={over}",
            v.len(),
            sample_angle(*level) * terrain().params().radius,
            skirt(*level),
            max_gap
        );
        worst = worst.max(mx);
        over_total += over;
    }
    println!("S4 deep levels: worst ratio {worst:.4}, odd vertices over skirt {over_total}");
    assert_eq!(over_total, 0, "gap exceeds skirt depth at deep levels");
}
