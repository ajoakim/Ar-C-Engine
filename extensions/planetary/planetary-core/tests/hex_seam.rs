//! CPU seam proofs for the hex meshes (ADR-0010 carried over to ADR-0011): same-level borders,
//! icosahedron edges and vertices, pentagons, mixed-level junctions including half-cell
//! diagonals (with a negative control), measured error and f32 reconstruction.
//!
//! Junction geometry (spike S4): at an L / L+1 border the coarse side is always a hex outer edge
//! (of a whole cell or of a half piece) and the fine side is the diagonal of the half-piece edge
//! child that is a child of both cells. "Edges" below means the coarse side is a whole cell,
//! "diagonals" means the coarse side is a half piece (its other border is the half diagonal).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use engine_sdk::DVec3;
use engine_sdk::procedural::splitmix64;
use planetary_core::hex::adjacency::{children, is_pentagon, neighbors, parents};
use planetary_core::hex::mapping::{cell_center_dir, cell_sample_point, ring, vertices};
use planetary_core::hex::mesh::{
    build_hex_cell, hex_skirt_depth, hex_vertex_position_f64, vertex_count_hex,
};
use planetary_core::hex::pieces::{close, pieces, variant_sectors};
use planetary_core::hex::{CellFrame, CellId};
use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
use planetary_core::topology::Piece;
use planetary_core::{PlanetTerrain, TerrainParams};

const SEED: u64 = 0x0007_5EED_2026_0001;
/// Split distance factor of the selection rule (in units of the hex circumradius).
const RULE_K: f64 = 2.0;
const MAX_LEVEL: u8 = 14;

fn terrain(n: u32) -> PlanetTerrain {
    PlanetTerrain::new(SEED, TerrainParams::default(), n).expect("terrain")
}

/// `HEX_SAMPLE_ANGLE0(N)`: mean centre spacing / sqrt(3) (circumradius) divided by `N`.
fn sample_angle0(n: u32) -> f64 {
    (8.0 * core::f64::consts::PI / (10.0 * 3.0f64.sqrt())).sqrt() / (3.0f64.sqrt() * f64::from(n))
}

fn model(t: &PlanetTerrain) -> LodModel {
    LodModel::for_topology(t, DEFAULT_KAPPA, sample_angle0(t.cells_per_edge()))
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

/// Position of vertex `j` (`0..=n`) of the outer edge `corner_k -> corner_{k+1}` of a cell.
fn outer_edge(t: &PlanetTerrain, c: CellId, n: u32, k: u8) -> Vec<DVec3> {
    (0..=n)
        .map(|j| hex_vertex_position_f64(t, c, n, k, n - j, j))
        .collect()
}

/// Positions of diagonal `d` (`corner_d -> centre -> corner_{d+3}`), `2n + 1` samples.
fn diagonal(t: &PlanetTerrain, c: CellId, n: u32, d: u8) -> Vec<DVec3> {
    (0..=2 * n)
        .map(|j| {
            if j <= n {
                hex_vertex_position_f64(t, c, n, d, n - j, 0)
            } else {
                hex_vertex_position_f64(t, c, n, d + 3, j - n, 0)
            }
        })
        .collect()
}

#[test]
fn hex_mesh_vertices_are_the_position_function() {
    // The mesh builder and `hex_vertex_position_f64` are the same function: every grid vertex
    // of a built mesh equals the f32 rounding of `position - origin`.
    let n = 8;
    let t = terrain(n);
    let m = model(&t);
    let per = (n + 1) * (n + 2) / 2;
    for c in [
        CellId::new(3, 4, 5, 9).expect("cell"),
        CellId::north_pole(2),
        CellId::new(1, 3, 0, 0).expect("pentagon"),
    ] {
        let mesh = build_hex_cell(c, &t, &m, false, &|| false).expect("mesh");
        let sectors = u32::from(CellFrame::new(c, n).sectors());
        let mut idx = 0usize;
        for k in 0..sectors {
            for s in 0..=n {
                for tt in 0..=(n - s) {
                    let p = hex_vertex_position_f64(&t, c, n, k as u8, s, tt);
                    let o = [
                        (p.x - mesh.origin.x) as f32,
                        (p.y - mesh.origin.y) as f32,
                        (p.z - mesh.origin.z) as f32,
                    ];
                    assert_eq!(mesh.vertices[idx].position, o, "{c:?} k{k} s{s} t{tt}");
                    idx += 1;
                }
            }
        }
        assert_eq!(idx as u32, sectors * per);
    }
    assert_eq!(vertex_count_hex(n), 6 * per + 6 * (n + 1) + 3 * (2 * n + 1));
}

#[test]
fn hex_same_level_borders_bit_identical() {
    let n = 8;
    let t = terrain(n);
    let mut checked = 0u64;
    let mut expected = 0u64;
    let mut pentagon_pairs = 0u64;
    let mut on_icosa_edge = 0u64;
    let mut edges: BTreeSet<Vec<u8>> = BTreeSet::new();
    let mut ico_vertices: BTreeSet<(u8, u64, u64, u64, u64)> = BTreeSet::new();
    for level in 0..=3u8 {
        let mut cache: BTreeMap<(CellId, u8), Vec<DVec3>> = BTreeMap::new();
        let mut rings: BTreeMap<CellId, Vec<CellId>> = BTreeMap::new();
        for c in CellId::level_iter(level) {
            let r = ring(c);
            for k in 0..r.len() as u8 {
                cache.insert((c, k), outer_edge(&t, c, n, k));
            }
            rings.insert(c, r);
        }
        for c in CellId::level_iter(level) {
            for (k, x) in rings[&c].iter().enumerate() {
                let m = rings[x].iter().position(|y| *y == c).expect("reciprocal") as u8;
                let a = &cache[&(c, k as u8)];
                let b = &cache[&(*x, m)];
                for j in 0..=n as usize {
                    assert_eq!(
                        bits(a[j]),
                        bits(b[n as usize - j]),
                        "{c:?} sector {k} vs {x:?} sector {m} at {j}"
                    );
                    checked += 1;
                }
                if is_pentagon(c) || is_pentagon(*x) {
                    pentagon_pairs += u64::from(n + 1);
                }
                // Count border samples that lie on an icosahedron edge / vertex.
                let n_ = n;
                for j in 0..=n_ {
                    let p = cell_sample_point(c, n_, k as u8, n_ - j, j).canonical();
                    let support: Vec<u8> = {
                        let fv = planetary_core::hex::FACES[usize::from(p.face)];
                        fv.iter()
                            .zip([p.a, p.b, p.c])
                            .filter(|(_, w)| *w > 0)
                            .map(|(v, _)| *v)
                            .collect()
                    };
                    if support.len() == 2 {
                        on_icosa_edge += 1;
                        let mut e = support;
                        e.sort_unstable();
                        edges.insert(e);
                    }
                }
            }
            expected += rings[&c].len() as u64 * u64::from(n + 1);
            if is_pentagon(c) {
                // The icosahedron vertex is the centre sample of every sector, each computed on a
                // different face: all bit-identical.
                let sectors = CellFrame::new(c, n).sectors();
                let p0 = hex_vertex_position_f64(&t, c, n, 0, 0, 0);
                for k in 1..sectors {
                    assert_eq!(bits(p0), bits(hex_vertex_position_f64(&t, c, n, k, 0, 0)));
                }
                let lp = cell_sample_point(c, n, 0, 0, 0).canonical();
                if [lp.a, lp.b, lp.c].iter().filter(|w| **w > 0).count() == 1 {
                    ico_vertices.insert((lp.face, lp.a, lp.b, lp.c, lp.d));
                }
            }
        }
    }
    assert_eq!(checked, expected);
    // Every icosahedron edge and vertex is exercised by the border pairs above (level <= 3).
    assert_eq!(edges.len(), 30, "icosahedron edges exercised");
    let distinct_vertices: BTreeSet<u8> = ico_vertices
        .iter()
        .map(|(f, a, b, c, _)| {
            let fv = planetary_core::hex::FACES[usize::from(*f)];
            let w = [*a, *b, *c];
            fv[w.iter().position(|x| *x > 0).expect("support")]
        })
        .collect();
    assert_eq!(
        distinct_vertices.len(),
        12,
        "icosahedron vertices exercised"
    );
    assert!(pentagon_pairs > 0);
    eprintln!(
        "hex same-level border vertex pairs: {checked} bit-identical; pentagon-adjacent {pentagon_pairs}; samples on icosahedron edges/vertices {on_icosa_edge} ({} edges, {} vertices)",
        edges.len(),
        distinct_vertices.len()
    );
}

// ------------------------------------------------------------------------ mixed-level junctions

struct Junction {
    gap: f64,
    min_depth: f64,
    coarse_whole: bool,
}

struct Junctions {
    samples: Vec<Junction>,
    even_mismatches: u32,
    structure_errors: u32,
    count: u32,
    whole: u32,
    half: u32,
    pentagon: u32,
    icosa_cross: u32,
    levels: BTreeSet<u8>,
    views: u32,
}

fn eyes(radius: f64) -> Vec<DVec3> {
    let v = vertices();
    let nrm = |p: [f64; 3]| {
        let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        [p[0] / l, p[1] / l, p[2] / l]
    };
    let mut dirs: Vec<[f64; 3]> = v.iter().map(|p| nrm(*p)).collect();
    let mut edges = Vec::new();
    for a in 0..12 {
        for b in (a + 1)..12 {
            let d = [v[a][0] - v[b][0], v[a][1] - v[b][1], v[a][2] - v[b][2]];
            if ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) - 4.0).abs() < 1e-9 {
                edges.push((a, b));
            }
        }
    }
    assert_eq!(edges.len(), 30);
    for i in 0..18 {
        let (a, b) = edges[i * 30 / 18];
        dirs.push(nrm([
            v[a][0] + v[b][0],
            v[a][1] + v[b][1],
            v[a][2] + v[b][2],
        ]));
    }
    let mut out = Vec::new();
    for d in dirs {
        for alt in [1000.0, 50.0] {
            out.push(DVec3::new(
                d[0] * (radius + alt),
                d[1] * (radius + alt),
                d[2] * (radius + alt),
            ));
        }
    }
    out
}

/// Split set from a distance rule (cell split when the eye is within `RULE_K` circumradii of
/// its centre), closed under the balance rule.
fn split_set(eye: DVec3, radius: f64, full: bool) -> BTreeSet<CellId> {
    let s0 = (8.0 * core::f64::consts::PI / (10.0 * 3.0f64.sqrt())).sqrt();
    let mut split = BTreeSet::new();
    let mut frontier: Vec<CellId> = CellId::level_iter(0).collect();
    let mut kids = Vec::new();
    for level in 0..MAX_LEVEL {
        let circ = radius * s0 / (3.0f64.sqrt() * f64::from(1u32 << level));
        let mut next = BTreeSet::new();
        for c in frontier {
            let d = cell_center_dir(c, 4);
            let centre = DVec3::new(d.x * radius, d.y * radius, d.z * radius);
            if dist(eye, centre) < RULE_K * circ {
                split.insert(c);
                kids.clear();
                children(c, &mut kids);
                next.extend(kids.iter().copied());
            }
        }
        frontier = next.into_iter().collect();
    }
    if full {
        // Superseded full closure (parents plus all neighbours of the parents), informational.
        let mut work: Vec<CellId> = split.iter().copied().collect();
        let mut ps = Vec::new();
        let mut nb = Vec::new();
        while let Some(x) = work.pop() {
            ps.clear();
            parents(x, &mut ps);
            for q in ps.clone() {
                nb.clear();
                neighbors(q, &mut nb);
                for r in std::iter::once(q).chain(nb.iter().copied()) {
                    if split.insert(r) {
                        work.push(r);
                    }
                }
            }
        }
    } else {
        // Production balance rule: parents-only closure (ADR-0011). It produces coarse HALF pieces
        // facing finer cells, which the superseded full rule never produced.
        close(&mut split);
    }
    split
}

fn collect_junctions(full: bool) -> Junctions {
    let n = 8u32;
    let t = terrain(n);
    let m = model(&t);
    let radius = t.params().radius;
    let mut out = Junctions {
        samples: Vec::new(),
        even_mismatches: 0,
        structure_errors: 0,
        count: 0,
        whole: 0,
        half: 0,
        pentagon: 0,
        icosa_cross: 0,
        levels: BTreeSet::new(),
        views: 0,
    };
    let mut seen: BTreeSet<(CellId, u8)> = BTreeSet::new();
    let mut ring_cache: BTreeMap<CellId, Vec<CellId>> = BTreeMap::new();
    for eye in eyes(radius) {
        out.views += 1;
        let split = split_set(eye, radius, full);
        let mut ps: Vec<Piece<CellId>> = Vec::new();
        pieces(&split, &mut ps);
        let drawn: BTreeMap<CellId, u16> = ps.iter().map(|p| (p.cell, p.variant)).collect();
        for p in &ps {
            let c = p.cell;
            let r = ring_cache.entry(c).or_insert_with(|| ring(c)).clone();
            for &k in variant_sectors(p.variant) {
                let x = r[usize::from(k)];
                if !split.contains(&x) || !seen.insert((c, k)) {
                    continue;
                }
                // The edge child X is a child of both c and x.
                let (mut kc, mut kx) = (Vec::new(), Vec::new());
                children(c, &mut kc);
                children(x, &mut kx);
                let common: Vec<CellId> = kc.iter().copied().filter(|y| kx.contains(y)).collect();
                let Some(&xc) = common.first().filter(|_| common.len() == 1) else {
                    out.structure_errors += 1;
                    continue;
                };
                let Some(&v) = drawn.get(&xc).filter(|v| **v >= 2) else {
                    out.structure_errors += 1;
                    continue;
                };
                let d = ((v - 2) / 2) as u8;
                let coarse = outer_edge(&t, c, n, k);
                let mut fine = diagonal(&t, xc, n, d);
                if bits(fine[0]) != bits(coarse[0]) {
                    fine.reverse();
                }
                if bits(fine[0]) != bits(coarse[0]) {
                    out.even_mismatches += 1;
                    continue;
                }
                let min_depth = hex_skirt_depth(&m, c.level()).min(hex_skirt_depth(&m, xc.level()));
                let whole = p.variant < 2;
                for (g, f) in fine.iter().enumerate() {
                    if g % 2 == 0 {
                        if bits(*f) != bits(coarse[g / 2]) {
                            out.even_mismatches += 1;
                        }
                    } else {
                        let gap = dist_to_segment(*f, coarse[(g - 1) / 2], coarse[g.div_ceil(2)]);
                        out.samples.push(Junction {
                            gap,
                            min_depth,
                            coarse_whole: whole,
                        });
                    }
                }
                out.count += 1;
                out.levels.insert(c.level());
                if whole {
                    out.whole += 1;
                } else {
                    out.half += 1;
                }
                if is_pentagon(c) || is_pentagon(x) {
                    out.pentagon += 1;
                }
                let crosses = (0..=n).any(|j| {
                    let lp = cell_sample_point(c, n, k, n - j, j).canonical();
                    [lp.a, lp.b, lp.c].iter().filter(|w| **w > 0).count() < 3
                });
                if crosses {
                    out.icosa_cross += 1;
                }
            }
        }
    }
    out
}

/// Production closure (parents-only): the main path.
fn junctions() -> &'static Junctions {
    static J: OnceLock<Junctions> = OnceLock::new();
    J.get_or_init(|| collect_junctions(false))
}

/// Superseded full closure (informational comparison).
fn junctions_full() -> &'static Junctions {
    static J: OnceLock<Junctions> = OnceLock::new();
    J.get_or_init(|| collect_junctions(true))
}

/// Number of odd-vertex violations; `forced_depth` overrides the real skirt depth.
fn violations(j: &Junctions, forced_depth: Option<f64>) -> usize {
    j.samples
        .iter()
        .filter(|s| s.gap > forced_depth.unwrap_or(s.min_depth))
        .count()
}

fn max_ratio(j: &Junctions, whole: bool) -> f64 {
    j.samples
        .iter()
        .filter(|s| s.coarse_whole == whole)
        .map(|s| s.gap / s.min_depth)
        .fold(0.0f64, f64::max)
}

#[test]
fn hex_mixed_level_gap_bounded_by_skirts() {
    let j = junctions();
    assert!(j.count > 0, "no L/L+1 junctions found");
    assert_eq!(
        j.structure_errors, 0,
        "junction structure (edge child / half piece)"
    );
    assert_eq!(j.even_mismatches, 0, "even fine vertices must equal coarse");
    assert!(j.whole > 0, "no whole-cell coarse junction");
    assert!(j.half > 0, "no coarse half-piece junction exercised");
    assert!(j.pentagon > 0, "no pentagon junction exercised");
    assert!(j.icosa_cross > 0, "no icosahedron-edge junction exercised");
    let (re, rd) = (max_ratio(j, true), max_ratio(j, false));
    eprintln!(
        "hex junctions: {} ({} views, coarse levels {:?}); coarse whole {} / coarse half {}; touching a pentagon {}; crossing an icosahedron edge/vertex {}; odd vertices {} (whole {} / half {})",
        j.count,
        j.views,
        j.levels,
        j.whole,
        j.half,
        j.pentagon,
        j.icosa_cross,
        j.samples.len(),
        j.samples.iter().filter(|s| s.coarse_whole).count(),
        j.samples.iter().filter(|s| !s.coarse_whole).count()
    );
    eprintln!(
        "max gap / skirt depth: edges (coarse whole) {re:.6}, diagonals (coarse half) {rd:.6}"
    );
    assert_eq!(violations(j, None), 0, "gap exceeds skirt depth");
    assert!(re <= 1.0 && rd <= 1.0);
}

/// Coarse HALF pieces facing finer cells (produced by the production parents-only closure): the
/// half piece's outer edges are bounded by the same skirts, and the depth-0 control finds them.
#[test]
fn hex_mixed_level_gap_with_half_pieces() {
    let j = junctions();
    assert_eq!(
        j.structure_errors, 0,
        "junction structure (edge child / half piece)"
    );
    assert_eq!(j.even_mismatches, 0, "even fine vertices must equal coarse");
    assert!(j.half > 0, "no coarse half-piece junction exercised");
    let (re, rd) = (max_ratio(j, true), max_ratio(j, false));
    eprintln!(
        "parents-only closure: {} junctions (coarse whole {} / half {}), odd vertices {}; max gap / skirt depth: whole {re:.6}, half {rd:.6}",
        j.count,
        j.whole,
        j.half,
        j.samples.len()
    );
    assert_eq!(violations(j, None), 0, "gap exceeds skirt depth");
    assert!(
        violations(j, Some(0.0)) == j.samples.iter().filter(|s| s.gap > 0.0).count()
            && violations(j, Some(0.0)) > 0
    );
    let half_odd: Vec<_> = j.samples.iter().filter(|s| !s.coarse_whole).collect();
    assert!(!half_odd.is_empty(), "no odd vertex on a coarse half piece");
    assert!(
        half_odd.iter().any(|s| s.gap > 0.0),
        "control: coarse half pieces must show a gap that the skirt covers"
    );
}

/// The superseded full closure on the same views: no coarse half pieces, still 0 violations.
#[test]
fn hex_superseded_full_closure_informational() {
    let j = junctions_full();
    assert_eq!(j.structure_errors, 0);
    assert_eq!(j.even_mismatches, 0);
    assert_eq!(violations(j, None), 0, "gap exceeds skirt depth");
    eprintln!(
        "superseded full closure: {} junctions (coarse whole {} / half {})",
        j.count, j.whole, j.half
    );
}

#[test]
fn hex_negative_control_without_skirts_fails() {
    let j = junctions();
    let v = violations(j, Some(0.0));
    let with_gap = j.samples.iter().filter(|s| s.gap > 0.0).count();
    eprintln!(
        "negative control (depth 0): {v} violations of {} odd vertices ({with_gap} have a gap)",
        j.samples.len()
    );
    assert!(v > 0, "check cannot fail: no violation with depth 0");
    assert_eq!(v, with_gap, "every odd vertex with a gap must be reported");
    assert_eq!(violations(j, None), 0);
}

// -------------------------------------------------------------------------- measured error / f32

fn seeded_hex_at(level: u8, s: &mut u64) -> CellId {
    loop {
        *s = splitmix64(*s);
        let k = 1u64 << level;
        let base = 1 + ((*s >> 4) % 10) as u8;
        let c = CellId::new(base, level, ((*s >> 8) % k) as u32, ((*s >> 36) % k) as u32)
            .expect("cell");
        if !is_pentagon(c) {
            return c;
        }
    }
}

#[test]
fn hex_measured_error_below_eps() {
    let n = 16u32;
    let t = terrain(n);
    let m = model(&t);
    let mut s = 0x5EED_u64;
    let mut overall = 0.0f64;
    for level in [2u8, 6, 10, 14, 16] {
        let eps = m.geometric_error(level);
        let mut worst = 0.0f64;
        for _ in 0..4 {
            let c = seeded_hex_at(level, &mut s);
            let frame = CellFrame::new(c, n);
            let dense = CellFrame::new(c, 2 * n);
            let pos = |k: u8, a: u32, b: u32| {
                planetary_core::hex::mapping::sample_dir(frame.sample(k, a, b))
            };
            let at = |k: u8, a: u32, b: u32| t.surface_point(pos(k, a, b));
            for k in 0..frame.sectors() {
                for a in 0..=n {
                    for b in 0..=(n - a) {
                        let mut check = |a2: u32, b2: u32| {
                            let (p, q) = (at(k, a, b), at(k, a2, b2));
                            let mid =
                                DVec3::new((p.x + q.x) * 0.5, (p.y + q.y) * 0.5, (p.z + q.z) * 0.5);
                            // Lattice midpoint = sample (a + a2, b + b2) of the doubled frame.
                            let truth = t.surface_point(planetary_core::hex::mapping::sample_dir(
                                dense.sample(k, a + a2, b + b2),
                            ));
                            let dev = dist(truth, mid);
                            assert!(dev <= eps, "level {level}: deviation {dev} > eps {eps}");
                            worst = worst.max(dev / eps);
                        };
                        if a + b < n {
                            check(a + 1, b);
                            check(a, b + 1);
                        }
                        if a + b < n && a >= 1 {
                            check(a - 1, b + 1);
                        }
                    }
                }
            }
        }
        eprintln!("level {level}: max deviation / eps = {worst:.6}");
        overall = overall.max(worst);
    }
    eprintln!("overall max deviation / eps = {overall:.6} (kappa = {DEFAULT_KAPPA})");
    assert!(overall <= 1.0);
}

#[test]
fn hex_f32_reconstruction_within_tolerance() {
    let n = 16u32;
    let t = terrain(n);
    let m = model(&t);
    let mut s = 0xABCD_u64;
    let mut worst = 0.0f64;
    for level in [0u8, 5, 10, 15, 16] {
        for rep in 0..2 {
            let c = if level == 0 {
                CellId::level_iter(0).nth(rep + 3).expect("root")
            } else {
                seeded_hex_at(level, &mut s)
            };
            let r = ring(c);
            s = splitmix64(s);
            let k = (s % r.len() as u64) as usize;
            let x = r[k];
            let mk = ring(x).iter().position(|y| *y == c).expect("reciprocal");
            let (ma, mb) = (
                build_hex_cell(c, &t, &m, false, &|| false).expect("mesh"),
                build_hex_cell(x, &t, &m, false, &|| false).expect("mesh"),
            );
            let extent = f64::from(ma.bound_radius.max(mb.bound_radius));
            let bound = 2.0 * extent * 2f64.powi(-23);
            assert!(bound < hex_skirt_depth(&m, level), "level {level}");
            let per = (n + 1) * (n + 2) / 2;
            let idx = |sector: usize, a: u32, b: u32| {
                sector as u32 * per + a * (n + 1) - a * a.saturating_sub(1) / 2 + b
            };
            let at = |mesh: &planetary_core::CellMesh, i: u32| {
                let v = mesh.vertices[i as usize].position;
                DVec3::new(
                    mesh.origin.x + f64::from(v[0]),
                    mesh.origin.y + f64::from(v[1]),
                    mesh.origin.z + f64::from(v[2]),
                )
            };
            for j in 0..=n {
                let pa = at(&ma, idx(k, n - j, j));
                let pb = at(&mb, idx(mk, j, n - j));
                let d = dist(pa, pb);
                assert!(d <= bound, "level {level}: f32 error {d} > {bound}");
                worst = worst.max(d / bound);
                let truth = hex_vertex_position_f64(&t, c, n, k as u8, n - j, j);
                assert!(dist(pa, truth) <= bound, "level {level}: vs f64 truth");
            }
        }
    }
    eprintln!("hex f32 reconstruction: max error / bound = {worst:.6} (bound < skirt depth)");
}
