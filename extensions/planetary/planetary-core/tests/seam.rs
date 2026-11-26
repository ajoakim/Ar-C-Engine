//! CPU seam proofs (ADR-0010): same-level borders, cube edges and corners, mixed-level junction
//! gaps vs skirt depth (with a negative control), f32 reconstruction and measured eps.

use std::collections::{BTreeMap, BTreeSet};

use engine_sdk::DVec3;
use engine_sdk::procedural::splitmix64;
use planetary_core::lod::{
    DEFAULT_KAPPA, LodModel, LodParams, View, is_partition, max_level_jump, reference_views, select,
};
use planetary_core::mesh::{border_positions_f64, build_patch, skirt_depth, vertex_position_f64};
use planetary_core::{
    Edge, Face, PatchId, PlanetTerrain, TerrainParams, lattice_dir, neighbor, neighbor_children,
    patch_corner_dirs,
};

const SEED: u64 = 0x0007_5EED_2026_0001;

fn terrain(n: u32) -> PlanetTerrain {
    PlanetTerrain::new(SEED, TerrainParams::default(), n).expect("terrain")
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

#[test]
fn same_level_borders_bit_identical() {
    let t = terrain(8);
    let n = t.cells_per_edge();
    let mut checked = 0u64;
    let mut expected = 0u64;
    for level in 0..=4u8 {
        let mut cache: BTreeMap<(PatchId, Edge), Vec<DVec3>> = BTreeMap::new();
        for p in PatchId::level_iter(level) {
            for e in Edge::ALL {
                cache.insert((p, e), border_positions_f64(&t, p, e));
            }
        }
        for p in PatchId::level_iter(level) {
            for e in Edge::ALL {
                let nb = neighbor(p, e);
                let a = &cache[&(p, e)];
                let b = &cache[&(nb.patch, nb.edge)];
                for k in 0..=n {
                    let kb = if nb.flipped { n - k } else { k };
                    assert_eq!(
                        bits(a[k as usize]),
                        bits(b[kb as usize]),
                        "{p:?} {e:?} vs {:?} {:?} at {k}",
                        nb.patch,
                        nb.edge
                    );
                    checked += 1;
                }
            }
        }
        expected += 6 * 4u64.pow(u32::from(level)) * 4 * u64::from(n + 1);
    }
    assert_eq!(checked, expected);
    eprintln!("same-level border vertex pairs checked: {checked} (all bit-identical)");
}

#[test]
fn cube_edges_and_corners_explicit() {
    let t = terrain(8);
    let n = t.cells_per_edge();
    let level = 2u8;
    // 12 cube edges: face pairs with perpendicular normals.
    let mut edges = BTreeSet::new();
    for f in Face::ALL {
        for e in Edge::ALL {
            let g = Face::from_normal(f.basis().outward(e)).expect("edge face");
            let key = if f < g { (f, g) } else { (g, f) };
            if !edges.insert(key) {
                continue;
            }
            // All level-2 patches of `f` along edge `e` against their neighbors across it.
            let mut pairs = 0;
            for p in PatchId::level_iter(level).filter(|p| p.face() == f) {
                let (x, y) = p.xy();
                let k = (1u32 << level) - 1;
                let on_edge = match e {
                    Edge::NegU => x == 0,
                    Edge::PosU => x == k,
                    Edge::NegV => y == 0,
                    Edge::PosV => y == k,
                };
                if !on_edge {
                    continue;
                }
                let nb = neighbor(p, e);
                assert_eq!(nb.patch.face(), g);
                let a = border_positions_f64(&t, p, e);
                let b = border_positions_f64(&t, nb.patch, nb.edge);
                for i in 0..=n {
                    let j = if nb.flipped { n - i } else { i };
                    assert_eq!(
                        bits(a[i as usize]),
                        bits(b[j as usize]),
                        "cube edge {f:?}/{g:?}: {p:?} vs {:?} at {i}",
                        nb.patch
                    );
                }
                pairs += 1;
            }
            assert_eq!(pairs, 4, "cube edge {f:?}/{g:?} patch pairs");
        }
    }
    assert_eq!(edges.len(), 12, "cube edges");
    eprintln!("cube edges verified: {}/12", edges.len());

    // 8 corners: group the three face-corner vertices at each cube corner.
    let mut corners: BTreeMap<[u64; 3], Vec<(PatchId, u32, u32)>> = BTreeMap::new();
    for p in PatchId::level_iter(level) {
        let dirs = patch_corner_dirs(p);
        let ij = [(0, 0), (n, 0), (0, n), (n, n)];
        for (d, (i, j)) in dirs.iter().zip(ij) {
            let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
            if ax.to_bits() == ay.to_bits() && ay.to_bits() == az.to_bits() {
                let key = [
                    u64::from(d.x < 0.0),
                    u64::from(d.y < 0.0),
                    u64::from(d.z < 0.0),
                ];
                corners.entry(key).or_default().push((p, i, j));
            }
        }
    }
    assert_eq!(corners.len(), 8, "cube corners");
    for (key, list) in &corners {
        assert_eq!(list.len(), 3, "corner {key:?} must touch 3 faces");
        let first = vertex_position_f64(&t, list[0].0, list[0].1, list[0].2);
        for &(p, i, j) in &list[1..] {
            assert_eq!(
                bits(first),
                bits(vertex_position_f64(&t, p, i, j)),
                "corner {key:?}: {p:?}"
            );
        }
    }
    eprintln!("cube corners verified: {}/8", corners.len());
}

/// One L / L+1 junction sample: gap of an odd fine vertex to the coarse edge, and the skirt
/// depths of both patches.
struct Junction {
    gap: f64,
    min_depth: f64,
}

struct Junctions {
    samples: Vec<Junction>,
    even_mismatches: u32,
    count: u32,
}

fn collect_junctions(t: &PlanetTerrain, model: &LodModel) -> Junctions {
    let params = LodParams {
        max_level: 14,
        ..LodParams::default()
    };
    let mut out = Junctions {
        samples: Vec::new(),
        even_mismatches: 0,
        count: 0,
    };
    let mut cache: BTreeMap<(PatchId, Edge), Vec<DVec3>> = BTreeMap::new();
    let mut seen: BTreeSet<(PatchId, Edge)> = BTreeSet::new();
    for (name, eye) in reference_views(model.radius, model.max_relief) {
        let view = View::new(eye, 720, 1.0 / 3.0f64.sqrt()).expect("view");
        let sel = select(model, &params, &view, &BTreeSet::new());
        assert!(is_partition(&sel.leaves), "{name}: partition");
        assert!(max_level_jump(&sel.leaves) <= 1, "{name}: jump");
        let leaves: BTreeSet<PatchId> = sel.leaves.iter().copied().collect();
        for &p in &sel.leaves {
            for e in Edge::ALL {
                let Some(kids) = neighbor_children(p, e) else {
                    continue;
                };
                if !kids.iter().all(|k| leaves.contains(k)) {
                    continue;
                }
                if !seen.insert((p, e)) {
                    continue;
                }
                let nb = neighbor(p, e);
                let mut fine: Vec<DVec3> = Vec::new();
                for (k, kid) in kids.iter().enumerate() {
                    let mut b = cache
                        .entry((*kid, nb.edge))
                        .or_insert_with(|| border_positions_f64(t, *kid, nb.edge))
                        .clone();
                    if nb.flipped {
                        b.reverse();
                    }
                    if k == 0 {
                        fine.extend(b);
                    } else {
                        fine.extend(b.into_iter().skip(1));
                    }
                }
                let coarse = cache
                    .entry((p, e))
                    .or_insert_with(|| border_positions_f64(t, p, e))
                    .clone();
                let min_depth =
                    skirt_depth(model, p.level()).min(skirt_depth(model, p.level() + 1));
                for (g, f) in fine.iter().enumerate() {
                    if g % 2 == 0 {
                        if bits(*f) != bits(coarse[g / 2]) {
                            out.even_mismatches += 1;
                        }
                    } else {
                        let gap = dist_to_segment(*f, coarse[(g - 1) / 2], coarse[g.div_ceil(2)]);
                        out.samples.push(Junction { gap, min_depth });
                    }
                }
                out.count += 1;
            }
        }
    }
    out
}

/// Number of odd-vertex violations; `forced_depth` overrides the real skirt depth.
fn violations(j: &Junctions, forced_depth: Option<f64>) -> usize {
    j.samples
        .iter()
        .filter(|s| s.gap > forced_depth.unwrap_or(s.min_depth))
        .count()
}

#[test]
fn mixed_level_gap_bounded_by_skirts() {
    let t = terrain(8);
    let model = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let j = collect_junctions(&t, &model);
    assert!(j.count > 0, "no L/L+1 junctions found");
    assert_eq!(j.even_mismatches, 0, "even fine vertices must equal coarse");
    let max_ratio = j
        .samples
        .iter()
        .map(|s| s.gap / s.min_depth)
        .fold(0.0f64, f64::max);
    eprintln!(
        "mixed-level junctions checked: {} ({} odd vertices); max gap/min skirt depth = {max_ratio:.6}",
        j.count,
        j.samples.len()
    );
    assert_eq!(violations(&j, None), 0, "gap exceeds skirt depth");
    assert!(max_ratio <= 1.0);
}

#[test]
fn negative_control_without_skirts_fails() {
    let t = terrain(8);
    let model = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let j = collect_junctions(&t, &model);
    let v = violations(&j, Some(0.0));
    eprintln!(
        "negative control (depth 0): {v} violations of {} odd vertices",
        j.samples.len()
    );
    assert!(v > 0, "check cannot fail: no violation with depth 0");
    assert!(j.samples.iter().any(|s| s.gap > 0.0));
    assert_eq!(violations(&j, None), 0);
}

#[test]
fn f32_reconstruction_within_tolerance() {
    let t = terrain(8);
    let model = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let n = t.cells_per_edge();
    let w = (n + 1) as usize;
    let mut s = 0xABCD_u64;
    let mut worst = 0.0f64;
    for level in [0u8, 5, 10, 15, 19] {
        for _ in 0..2 {
            s = splitmix64(s);
            let k = 1u64 << level;
            let x = ((s >> 8) % k) as u32;
            let y = ((s >> 36) % k) as u32;
            let face = Face::ALL[(s % 6) as usize];
            let p = PatchId::new(face, level, x, y).expect("id");
            let e = Edge::ALL[((s >> 4) % 4) as usize];
            let nb = neighbor(p, e);
            let (ma, mb) = (
                build_patch(&t, &model, p),
                build_patch(&t, &model, nb.patch),
            );
            let extent = f64::from(ma.bound_radius.max(mb.bound_radius));
            let bound = 2.0 * extent * 2f64.powi(-23);
            assert!(bound < skirt_depth(&model, level), "level {level}");
            let at = |m: &planetary_core::PatchMesh, (i, j): (u32, u32)| {
                let v = m.vertices[j as usize * w + i as usize].position;
                DVec3::new(
                    m.origin.x + f64::from(v[0]),
                    m.origin.y + f64::from(v[1]),
                    m.origin.z + f64::from(v[2]),
                )
            };
            for i in 0..=n {
                let ib = if nb.flipped { n - i } else { i };
                let d = dist(at(&ma, e.vertex(n, i)), at(&mb, nb.edge.vertex(n, ib)));
                assert!(d <= bound, "level {level}: f32 error {d} > {bound}");
                worst = worst.max(d / bound);
            }
        }
    }
    eprintln!("f32 reconstruction: max error / bound = {worst:.6} (bound < skirt depth)");
}

#[test]
fn measured_error_below_eps() {
    let t = terrain(32);
    let model = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let n = t.cells_per_edge();
    let mut overall = 0.0f64;
    let mut s = 0x5EED_u64;
    for level in [2u8, 6, 10, 14, 18, 19] {
        let mut max_ratio = 0.0f64;
        for _ in 0..4 {
            s = splitmix64(s);
            let k = 1u64 << level;
            let face = Face::ALL[(s % 6) as usize];
            let (x, y) = (((s >> 8) % k) as u32, ((s >> 36) % k) as u32);
            let id = PatchId::new(face, level, x, y).expect("id");
            let pos: Vec<DVec3> = (0..=n)
                .flat_map(|j| (0..=n).map(move |i| (i, j)))
                .map(|(i, j)| vertex_position_f64(&t, id, i, j))
                .collect();
            let g2 = (u64::from(n) << level) * 2;
            let (bx, by) = (u64::from(x) * u64::from(n), u64::from(y) * u64::from(n));
            let at = |i: u32, j: u32| pos[(j * (n + 1) + i) as usize];
            for j in 0..=n {
                for i in 0..=n {
                    let mut check = |ni: u32, nj: u32, du: u64, dv: u64| {
                        let (a, b) = (at(i, j), at(ni, nj));
                        let mid =
                            DVec3::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5);
                        let d = lattice_dir(
                            id.face(),
                            2 * (bx + u64::from(i)) + du,
                            2 * (by + u64::from(j)) + dv,
                            g2,
                        );
                        let truth = t.surface_point(d);
                        let dev = dist(truth, mid);
                        assert!(
                            dev <= model.geometric_error(level),
                            "level {level}: deviation {dev} > eps {}",
                            model.geometric_error(level)
                        );
                        max_ratio = max_ratio.max(dev / model.cell_size(level));
                    };
                    if i < n {
                        check(i + 1, j, 1, 0);
                    }
                    if j < n {
                        check(i, j + 1, 0, 1);
                    }
                }
            }
        }
        eprintln!("level {level}: max deviation/cell_size = {max_ratio:.6}");
        overall = overall.max(max_ratio);
    }
    eprintln!("overall max deviation/cell_size = {overall:.6} (kappa = {DEFAULT_KAPPA})");
}
