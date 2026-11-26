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

// ------------------------------------------------------------------------ morph target seams

mod morph_seams {
    use super::*;
    use planetary_core::hex::mesh::{hex_morph_targets_f64, hex_morph_targets_with_parents_f64};
    use planetary_core::morph::MorphTargets;

    /// Same-level border agreement tolerance (1e-9 m, the plan value).
    const SAME_LEVEL_M: f64 = 1e-9;
    /// Distance of a fine border target from the coarse border polyline (a few f64 ulps).
    const ON_COARSE_BORDER_M: f64 = 2e-9;

    fn per(n: u32) -> u32 {
        (n + 1) * (n + 2) / 2
    }

    /// Grid index of `(sector, s, t)` (the mesh vertex layout).
    fn gi(n: u32, k: u32, s: u32, t: u32) -> usize {
        (k * per(n) + s * (n + 1) - s * s.saturating_sub(1) / 2 + t) as usize
    }

    fn outer(n: u32, k: u32, j: u32) -> usize {
        gi(n, k, n - j, j)
    }

    /// Grid `(sector, s, t)` of diagonal `d`, vertex `j` (`0..=2n`).
    fn diag_grid(n: u32, d: u32, j: u32) -> (u32, u32, u32) {
        if j <= n {
            (d, n - j, 0)
        } else {
            (d + 3, j - n, 0)
        }
    }

    fn outer_skirt(n: u32, sectors: u32, k: u32, j: u32) -> usize {
        (sectors * per(n) + k * (n + 1) + j) as usize
    }

    fn diag_skirt(n: u32, sectors: u32, d: u32, j: u32) -> usize {
        (sectors * (per(n) + n + 1) + d * (2 * n + 1) + j) as usize
    }

    fn targets(t: &PlanetTerrain, m: &LodModel, c: CellId) -> MorphTargets {
        hex_morph_targets_f64(c, t, m).expect("targets")
    }

    /// Largest position and uv difference between the outer-edge targets of `a` (sector `ka`) and
    /// `b` (sector `kb`), which run in opposite directions.
    fn border_diff(n: u32, a: &MorphTargets, ka: u32, b: &MorphTargets, kb: u32) -> (f64, f64) {
        let (mut dp, mut du) = (0.0f64, 0.0f64);
        for j in 0..=n {
            let (ga, gb) = (outer(n, ka, j), outer(n, kb, n - j));
            dp = dp.max(dist(a.position[ga], b.position[gb]));
            for q in 0..2 {
                du = du.max((f64::from(a.uv[ga][q]) - f64::from(b.uv[gb][q])).abs());
            }
        }
        (dp, du)
    }

    #[test]
    fn same_level_border_targets_agree() {
        let n = 4;
        let t = terrain(n);
        let m = model(&t);
        let (mut worst_p, mut worst_u, mut pairs, mut pent) = (0.0f64, 0.0f64, 0u64, 0u64);
        let mut worst_cell = None;
        for level in 1..=3u8 {
            let mut all: BTreeMap<CellId, MorphTargets> = BTreeMap::new();
            let mut rings: BTreeMap<CellId, Vec<CellId>> = BTreeMap::new();
            for c in CellId::level_iter(level) {
                all.insert(c, targets(&t, &m, c));
                rings.insert(c, ring(c));
            }
            for c in CellId::level_iter(level) {
                for (k, x) in rings[&c].iter().enumerate() {
                    let mm = rings[x].iter().position(|y| *y == c).expect("reciprocal");
                    let (dp, du) = border_diff(n, &all[&c], k as u32, &all[x], mm as u32);
                    if dp > worst_p {
                        worst_cell = Some((c, *x, dp));
                    }
                    worst_p = worst_p.max(dp);
                    worst_u = worst_u.max(du);
                    pairs += 1;
                    if is_pentagon(c) || is_pentagon(*x) {
                        pent += 1;
                    }
                }
            }
        }
        eprintln!(
            "hex morph same-level border pairs: {pairs} (pentagon-adjacent {pent}), worst position difference {worst_p:.3e} m, worst uv difference {worst_u:.3e}, worst pair {worst_cell:?}"
        );
        assert!(
            worst_p <= SAME_LEVEL_M,
            "border targets differ by {worst_p} m"
        );
        assert!(worst_u <= 1e-9, "border uvs differ by {worst_u}");
    }

    /// Diagonal `d` of the edge child `c` whose targets all lie on the shared border of its two
    /// parents, with the worst distance to that border (None when no diagonal does).
    fn shared_diagonal(
        t: &PlanetTerrain,
        c: CellId,
        n: u32,
        tg: &MorphTargets,
    ) -> Option<(u32, f64, Vec<CellId>)> {
        let mut ps = Vec::new();
        parents(c, &mut ps);
        if ps.len() != 2 {
            return None;
        }
        let (p, q) = (ps[0], ps[1]);
        let rp = ring(p);
        let k = rp.iter().position(|x| *x == q)? as u8;
        let poly = outer_edge(t, p, n, k);
        let mut found: Vec<(u32, f64)> = Vec::new();
        for d in 0..3u32 {
            let mut worst = 0.0f64;
            for j in 0..=2 * n {
                let (kk, s, tt) = diag_grid(n, d, j);
                let pos = tg.position[gi(n, kk, s, tt)];
                let dd = (0..poly.len() - 1)
                    .map(|i| dist_to_segment(pos, poly[i], poly[i + 1]))
                    .fold(f64::INFINITY, f64::min);
                worst = worst.max(dd);
            }
            // 1 m: far below the distance of the other two diagonals (kilometres).
            if worst < 1.0 {
                found.push((d, worst));
            }
        }
        assert!(
            found.len() <= 1,
            "{c:?}: more than one diagonal on the border"
        );
        found.first().map(|f| (f.0, f.1, ps))
    }

    #[test]
    fn edge_child_diagonal_targets_lie_on_the_shared_parent_border() {
        let n = 4;
        let t = terrain(n);
        let m = model(&t);
        let (mut worst, mut cells, mut missing) = (0.0f64, 0u32, 0u32);
        let (mut worst_ratio, mut pent_parent) = (0.0f64, 0u32);
        for level in 2..=4u8 {
            let mut s = splitmix64(0xD1A6 ^ u64::from(level));
            let count = if level == 4 { 60 } else { 40 };
            for _ in 0..count {
                let c = seeded_hex_at(level, &mut s);
                let mut ps = Vec::new();
                parents(c, &mut ps);
                if ps.len() != 2 {
                    continue;
                }
                let tg = targets(&t, &m, c);
                match shared_diagonal(&t, c, n, &tg) {
                    Some((d, w, ps)) => {
                        assert!(
                            w <= ON_COARSE_BORDER_M,
                            "{c:?}: diagonal {d} target {w} m off the shared border"
                        );
                        worst = worst.max(w);
                        // Displacement of the half-cell border against the skirt depth.
                        let depth = hex_skirt_depth(&m, level);
                        let mesh = build_hex_cell(c, &t, &m, false, &|| false).expect("mesh");
                        let _ = mesh;
                        for j in 0..=2 * n {
                            let (kk, ss, tt) = diag_grid(n, d, j);
                            let pos = hex_vertex_position_f64(&t, c, n, kk as u8, ss, tt);
                            let disp = dist(tg.position[gi(n, kk, ss, tt)], pos);
                            assert!(
                                disp < depth,
                                "{c:?}: half-border displacement {disp} >= {depth}"
                            );
                            worst_ratio = worst_ratio.max(disp / depth);
                        }
                        if ps.iter().any(|p| is_pentagon(*p)) {
                            pent_parent += 1;
                        }
                        cells += 1;
                    }
                    None => missing += 1,
                }
            }
        }
        eprintln!(
            "hex morph half-cell diagonals: {cells} edge children, worst distance to the shared parent border {worst:.3e} m, worst half-border displacement / skirt depth {worst_ratio:.4}, pentagon parents {pent_parent}, no matching diagonal {missing}"
        );
        assert!(cells >= 40);
        assert_eq!(
            missing, 0,
            "every edge child has exactly one diagonal on the border"
        );
    }

    #[test]
    fn outer_border_displacement_is_below_the_skirt_depth() {
        let n = 4;
        let t = terrain(n);
        let m = model(&t);
        let (mut worst_ratio, mut cells) = (0.0f64, 0u32);
        for level in 1..=4u8 {
            let mut s = splitmix64(0xB0D4 ^ u64::from(level));
            for _ in 0..30 {
                let c = seeded_hex_at(level, &mut s);
                let tg = targets(&t, &m, c);
                let depth = hex_skirt_depth(&m, level);
                let sectors = u32::from(CellFrame::new(c, n).sectors());
                for k in 0..sectors {
                    for j in 0..=n {
                        let pos = hex_vertex_position_f64(&t, c, n, k as u8, n - j, j);
                        let disp = dist(tg.position[outer(n, k, j)], pos);
                        assert!(disp < depth, "{c:?}: border displacement {disp} >= {depth}");
                        worst_ratio = worst_ratio.max(disp / depth);
                    }
                }
                cells += 1;
            }
        }
        eprintln!(
            "hex morph outer-border displacement / skirt depth: worst {worst_ratio:.4} over {cells} cells"
        );
    }

    /// Skirt targets equal the border target minus `dir * depth`; false when any differs.
    fn skirts_consistent(
        t: &PlanetTerrain,
        c: CellId,
        n: u32,
        depth: f64,
        tg: &MorphTargets,
    ) -> bool {
        let sectors = u32::from(CellFrame::new(c, n).sectors());
        let dir = |k: u32, s: u32, tt: u32| {
            planetary_core::hex::sample_dir(cell_sample_point(c, n, k as u8, s, tt))
        };
        let _ = t;
        let want = |g: usize, d: DVec3| {
            let b = tg.position[g];
            DVec3::new(b.x - d.x * depth, b.y - d.y * depth, b.z - d.z * depth)
        };
        for k in 0..sectors {
            for j in 0..=n {
                let d = dir(k, n - j, j);
                if tg.position[outer_skirt(n, sectors, k, j)] != want(outer(n, k, j), d) {
                    return false;
                }
            }
        }
        if sectors == 6 {
            for dg in 0..3u32 {
                for j in 0..=2 * n {
                    let (k, s, tt) = diag_grid(n, dg, j);
                    let d = dir(k, s, tt);
                    if tg.position[diag_skirt(n, sectors, dg, j)] != want(gi(n, k, s, tt), d) {
                        return false;
                    }
                }
            }
        }
        true
    }

    #[test]
    fn swapping_the_primary_parent_changes_nothing() {
        // Shared vertices lie in both parents and take the canonical chord; vertices in one
        // parent are found only there: the search order must not matter.
        let n = 4;
        let t = terrain(n);
        let m = model(&t);
        let mut checked = 0u32;
        let mut s = splitmix64(0x5A7);
        for _ in 0..40 {
            let c = seeded_hex_at(3, &mut s);
            let mut ps = Vec::new();
            parents(c, &mut ps);
            if ps.len() != 2 {
                continue;
            }
            let a = targets(&t, &m, c);
            let rev = [ps[1], ps[0]];
            let b = hex_morph_targets_with_parents_f64(c, &t, &m, &rev).expect("targets");
            let worst = a
                .position
                .iter()
                .zip(&b.position)
                .map(|(x, y)| dist(*x, *y))
                .fold(0.0f64, f64::max);
            assert!(
                worst <= 1e-6,
                "{c:?}: swapped parent order moves a target by {worst} m"
            );
            checked += 1;
        }
        eprintln!("hex morph swapped parent order: {checked} edge children identical");
        assert!(checked >= 10);
    }

    #[test]
    fn negative_controls_are_detected() {
        let n = 4;
        let t = terrain(n);
        let m = model(&t);
        let c = CellId::new(3, 3, 2, 5).expect("cell");
        let r = ring(c);
        let x = r[2];
        let mm = ring(x).iter().position(|y| *y == c).expect("reciprocal") as u32;
        let a = targets(&t, &m, c);
        let b = targets(&t, &m, x);
        // Control 0: the unperturbed pair passes.
        let (dp, _) = border_diff(n, &a, 2, &b, mm);
        assert!(dp <= SAME_LEVEL_M, "baseline {dp}");
        let depth = hex_skirt_depth(&m, c.level());
        assert!(skirts_consistent(&t, c, n, depth, &a));

        // Control 1: perturb one border morph_position by 1 mm.
        let mut bad = b.clone();
        bad.position[outer(n, mm, 2)].x += 1e-3;
        let (dp, _) = border_diff(n, &a, 2, &bad, mm);
        assert!(
            dp > SAME_LEVEL_M,
            "a 1 mm perturbation must be detected, got {dp}"
        );

        // Control 2: shift one skirt target by +1 m.
        let sectors = u32::from(CellFrame::new(c, n).sectors());
        let mut bad = a.clone();
        bad.position[outer_skirt(n, sectors, 1, 2)].z += 1.0;
        assert!(
            !skirts_consistent(&t, c, n, depth, &bad),
            "a 1 m skirt shift must be detected"
        );
        let mut bad = a.clone();
        bad.position[diag_skirt(n, sectors, 1, 3)].z += 1.0;
        assert!(
            !skirts_consistent(&t, c, n, depth, &bad),
            "a 1 m diagonal skirt shift must be detected"
        );

        // Control 3: the half-cell diagonal check fails when a diagonal target is perturbed.
        let mut s = splitmix64(0xC7);
        let edge_child = loop {
            let e = seeded_hex_at(3, &mut s);
            let mut ps = Vec::new();
            parents(e, &mut ps);
            if ps.len() == 2 {
                break e;
            }
        };
        let tg = targets(&t, &m, edge_child);
        let (d, w, _) = shared_diagonal(&t, edge_child, n, &tg).expect("diagonal");
        assert!(w <= ON_COARSE_BORDER_M);
        let mut bad = tg.clone();
        let (k, ss, tt) = diag_grid(n, d, 3);
        bad.position[gi(n, k, ss, tt)].y += 1e-3;
        let found = shared_diagonal(&t, edge_child, n, &bad);
        // A 1 mm shift is below the 1 m detection radius of the finder but above the tolerance.
        let (_, w2, _) = found.expect("still within 1 m");
        assert!(
            w2 > ON_COARSE_BORDER_M,
            "a 1 mm diagonal shift must exceed the tolerance, got {w2}"
        );
    }
}
