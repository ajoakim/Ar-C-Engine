#![allow(clippy::needless_range_loop, clippy::neg_cmp_op_on_partial_ord)]
//! Geomorph targets (07.1-22): brute-force oracles for cube and hex, bit-exact copies, fallback
//! counts, root/debug behaviour and continuity of the child triangles at morph 1. (Wall-clock
//! timing is not allowed in planetary crates; the build-time cost is measured with a temporary
//! harness and recorded in the plan summary.)
//!
//! The oracle is independent of the locator: it ray-casts the child vertex direction against EVERY
//! triangle of the parent-level mesh (Moller-Trumbore, its own barycentric and edge-distance
//! code) and takes the containing triangle.

use engine_sdk::DVec3;
use engine_sdk::procedural::splitmix64;
use planetary_core::hex::adjacency::{is_pentagon, parents};
use planetary_core::hex::mesh::{
    build_hex_cell, build_hex_cell_identity_morph, hex_morph_targets_f64,
    hex_morph_targets_with_parents_f64,
};
use planetary_core::hex::{CellFrame, CellId, sample_dir};
use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
use planetary_core::mesh::{build_patch, grid_indices, vertex_position_f64};
use planetary_core::morph::{blend, identity_morph};
use planetary_core::topology::cube::cube_morph_targets_f64;
use planetary_core::{
    CubeSphere, Face, PatchId, PlanetTerrain, TerrainParams, Topology, vertex_dir,
};

const SEED: u64 = 0x0007_5EED_2026_0001;
/// Absolute position tolerance of the oracle comparison (metres).
const ORACLE_M: f64 = 1e-6;
/// A hit may be this far (metres) outside a triangle edge and still count as inside.
const INSIDE_M: f64 = 2e-7;

fn terrain(n: u32) -> PlanetTerrain {
    PlanetTerrain::new(SEED, TerrainParams::default(), n).expect("terrain")
}

fn hex_angle0(n: u32) -> f64 {
    (8.0 * core::f64::consts::PI / (10.0 * 3.0f64.sqrt())).sqrt() / (3.0f64.sqrt() * f64::from(n))
}

fn hex_model(t: &PlanetTerrain) -> LodModel {
    LodModel::for_topology(t, DEFAULT_KAPPA, hex_angle0(t.cells_per_edge()))
}

fn sub(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}
fn dot(a: DVec3, b: DVec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}
fn cross(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}
fn len(a: DVec3) -> f64 {
    dot(a, a).sqrt()
}
fn dist(a: DVec3, b: DVec3) -> f64 {
    len(sub(a, b))
}
fn scale(a: DVec3, s: f64) -> DVec3 {
    DVec3::new(a.x * s, a.y * s, a.z * s)
}

/// Moller-Trumbore: `(point, weights, min signed edge distance in metres)` of the ray `t * d`
/// (`t > 0`) against triangle `(a, b, c)`.
fn ray_tri(a: DVec3, b: DVec3, c: DVec3, d: DVec3) -> Option<(DVec3, [f64; 3], f64)> {
    let e1 = sub(b, a);
    let e2 = sub(c, a);
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-30 {
        return None;
    }
    let inv = 1.0 / det;
    let tv = scale(a, -1.0);
    let u = dot(tv, p) * inv;
    let q = cross(tv, e1);
    let v = dot(d, q) * inv;
    let t = dot(e2, q) * inv;
    if !(t > 0.0) {
        return None;
    }
    let w = [1.0 - u - v, u, v];
    let area2 = len(cross(e1, e2));
    let opp = [len(sub(c, b)), len(e2), len(e1)];
    let md = (0..3)
        .map(|k| w[k] * area2 / opp[k])
        .fold(f64::INFINITY, f64::min);
    Some((scale(d, t), w, md))
}

/// Result of an oracle query: position and the interpolated uv.
type Target = (DVec3, [f64; 2]);

fn best_tri(tris: impl Iterator<Item = [(DVec3, [f32; 2]); 3]>, d: DVec3) -> Option<(f64, Target)> {
    let mut best: Option<(f64, Target)> = None;
    for t in tris {
        if let Some((p, w, md)) = ray_tri(t[0].0, t[1].0, t[2].0, d)
            && best.as_ref().is_none_or(|b| md > b.0)
        {
            let uv = |k: usize| {
                w[0] * f64::from(t[0].1[k])
                    + w[1] * f64::from(t[1].1[k])
                    + w[2] * f64::from(t[2].1[k])
            };
            best = Some((md, (p, [uv(0), uv(1)])));
        }
    }
    best.filter(|b| b.0 >= -INSIDE_M)
}

// ---------------------------------------------------------------------------------------------
// cube

struct CubeParent {
    n: u32,
    verts: Vec<(DVec3, [f32; 2])>,
}

fn cube_parent(t: &PlanetTerrain, model: &LodModel, p: PatchId) -> CubeParent {
    let n = t.cells_per_edge();
    let mesh = build_patch(t, model, p);
    let mut verts = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            verts.push((
                vertex_position_f64(t, p, i, j),
                mesh.vertices[(j * (n + 1) + i) as usize].uv,
            ));
        }
    }
    CubeParent { n, verts }
}

impl CubeParent {
    fn tris(&self) -> impl Iterator<Item = [(DVec3, [f32; 2]); 3]> + '_ {
        let w = self.n + 1;
        let at = move |i: u32, j: u32| self.verts[(j * w + i) as usize];
        (0..self.n).flat_map(move |j| {
            (0..self.n).flat_map(move |i| {
                [
                    [at(i, j), at(i + 1, j), at(i + 1, j + 1)],
                    [at(i, j), at(i + 1, j + 1), at(i, j + 1)],
                ]
            })
        })
    }
}

fn seeded_cube_cells(count: usize, seed: u64) -> Vec<PatchId> {
    let mut out = Vec::new();
    let mut s = seed;
    while out.len() < count {
        s = splitmix64(s);
        let level = 1 + (out.len() % 7) as u8;
        let face = Face::ALL[((s >> 3) % 6) as usize];
        let k = 1u64 << level;
        let x = ((s >> 8) % k) as u32;
        let y = ((s >> 36) % k) as u32;
        out.push(PatchId::new(face, level, x, y).expect("id"));
    }
    out
}

#[test]
fn cube_targets_match_the_brute_force_oracle() {
    let mut worst = 0.0f64;
    let mut worst_uv = 0.0f64;
    let (mut cells, mut vertices, mut copied) = (0u32, 0u64, 0u64);
    let mut extrapolated = 0u64;
    for n in [4u32, 8, 16] {
        let t = terrain(n);
        let model = LodModel::from_terrain(&t, DEFAULT_KAPPA);
        for c in seeded_cube_cells(if n == 16 { 8 } else { 14 }, 0xC0BE ^ u64::from(n)) {
            let parent = cube_parent(&t, &model, c.parent().expect("parent"));
            let tg = cube_morph_targets_f64(&t, &model, c).expect("targets");
            let mesh = build_patch(&t, &model, c);
            assert_eq!(tg.position.len(), mesh.vertices.len());
            let s = tg.stats;
            assert_eq!(s.identity, 0, "{c:?}");
            // A quad-centre direction lying on the grid diagonal can sit marginally (below
            // 2e-7 m, f64 noise) outside the chosen triangle; counted, still oracle-exact.
            extrapolated += u64::from(s.extrapolated);
            assert_eq!(
                s.parent_samples, 0,
                "{c:?}: no parent vertex outside the footprint"
            );
            for j in 0..=n {
                for i in 0..=n {
                    let k = (j * (n + 1) + i) as usize;
                    let d = vertex_dir(c, n, i, j);
                    let (md, (p, uv)) = best_tri(parent.tris(), d).expect("oracle hit");
                    assert!(md >= -INSIDE_M);
                    let e = dist(tg.position[k], p);
                    worst = worst.max(e);
                    assert!(e <= ORACLE_M, "{c:?} ({i},{j}): {e} m");
                    for q in 0..2 {
                        let eu = (f64::from(tg.uv[k][q]) - uv[q]).abs();
                        worst_uv = worst_uv.max(eu);
                        assert!(eu <= 2e-6, "{c:?} ({i},{j}) uv: {eu}");
                    }
                    vertices += 1;
                }
            }
            // Skirts hang by the skirt depth along -dir from the border target.
            let w = (n + 1) as usize;
            let mut idx = w * w;
            for e in planetary_core::Edge::ALL {
                for tt in 0..=n {
                    let (i, j) = e.vertex(n, tt);
                    let k = (j as usize) * w + i as usize;
                    let d = vertex_dir(c, n, i, j);
                    let want = DVec3::new(
                        tg.position[k].x - d.x * mesh.skirt_depth,
                        tg.position[k].y - d.y * mesh.skirt_depth,
                        tg.position[k].z - d.z * mesh.skirt_depth,
                    );
                    assert_eq!(tg.position[idx], want);
                    assert_eq!(tg.uv[idx], tg.uv[k]);
                    idx += 1;
                }
            }
            copied += u64::from(s.copied);
            cells += 1;
        }
    }
    println!(
        "cube oracle: {cells} cells, {vertices} vertices, worst position error {worst:.3e} m, worst uv error {worst_uv:.3e}, copied {copied}, marginal extrapolations {extrapolated}"
    );
}

#[test]
fn cube_even_vertices_copy_the_parent_bit_for_bit() {
    let n = 8u32;
    let t = terrain(n);
    let model = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let mut checked = 0u32;
    for c in seeded_cube_cells(16, 0xE7E4) {
        let p = c.parent().expect("parent");
        let pm = build_patch(&t, &model, p);
        let tg = cube_morph_targets_f64(&t, &model, c).expect("targets");
        let (x, y) = c.xy();
        for j in 0..=n {
            for i in 0..=n {
                let (ni, nj) = ((x & 1) * n + i, (y & 1) * n + j);
                if ni % 2 == 0 && nj % 2 == 0 {
                    let (pi, pj) = (ni / 2, nj / 2);
                    let want = vertex_position_f64(&t, p, pi, pj);
                    let k = (j * (n + 1) + i) as usize;
                    assert_eq!(tg.position[k], want, "{c:?} ({i},{j})");
                    assert_eq!(
                        tg.uv[k],
                        pm.vertices[(pj * (n + 1) + pi) as usize].uv,
                        "{c:?} uv"
                    );
                    checked += 1;
                }
            }
        }
    }
    println!("cube bit-exact parent copies checked: {checked}");
    assert!(checked >= 400);
}

#[test]
fn cube_roots_debug_and_vertices_unchanged() {
    let n = 8u32;
    let t = terrain(n);
    let model = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let cube = CubeSphere::new(n);
    for f in Face::ALL {
        let r = PatchId::root(f);
        let m = cube
            .build_mesh(r, &t, &model, false, &|| false)
            .expect("mesh");
        assert_eq!(m.morph, identity_morph(&m.vertices));
        let pm = build_patch(&t, &model, r);
        assert_eq!(m.vertices, pm.vertices);
    }
    for c in seeded_cube_cells(10, 0xDEB6) {
        let m = cube
            .build_mesh(c, &t, &model, false, &|| false)
            .expect("mesh");
        let pm = build_patch(&t, &model, c);
        assert_eq!(m.vertices, pm.vertices, "vertices unchanged for {c:?}");
        assert_eq!(m.morph.len(), m.vertices.len());
        let tg = cube_morph_targets_f64(&t, &model, c).expect("targets");
        assert_eq!(m.morph, tg.to_vertices(m.origin));
        assert!(m.morph.iter().any(|v| v.position != m.vertices[0].position));
        let dbg = cube
            .build_mesh(c, &t, &model, true, &|| false)
            .expect("mesh");
        for (v, mv) in dbg.vertices.iter().zip(&dbg.morph) {
            assert_eq!(mv.uv[1], v.uv[1], "debug colour must not blend");
        }
        assert_eq!(dbg.morph[0].position, m.morph[0].position);
    }
}

/// Deviation of the child triangles at morph 1 from the parent surface, measured radially.
///
/// Plan claim: every child triangle nests in one parent triangle, so the deviation is only
/// rounding. Reality: the equal-angle map is not linear, so the direction of a parent quad centre
/// is not in the plane of the grid diagonal; the centre child vertex lies on whichever parent
/// triangle contains its direction, and the child triangles touching it straddle the diagonal.
/// The sliver is second order: the test measures it per level and bounds the deviation, relative to
/// the parent edge, by `0.5 * (parent cell angle)^2` (measured worst factor about 0.12).
#[test]
fn cube_child_triangles_at_morph_one_follow_the_parent_surface() {
    let mut rows: Vec<(u32, u8, f64, f64, f64)> = Vec::new();
    for n in [4u32, 8] {
        let t = terrain(n);
        let model = LodModel::from_terrain(&t, DEFAULT_KAPPA);
        let idx = grid_indices(n);
        for level in 1..=7u8 {
            let (mut worst_abs, mut worst_rel) = (0.0f64, 0.0f64);
            for ci in 0..3u64 {
                let mut s =
                    splitmix64(0xC027 ^ u64::from(n) ^ (ci << 8) ^ (u64::from(level) << 16));
                let face = Face::ALL[(splitmix64(s) % 6) as usize];
                let k = 1u64 << level;
                s = splitmix64(s);
                let c =
                    PatchId::new(face, level, (s % k) as u32, ((s >> 20) % k) as u32).expect("id");
                let parent = cube_parent(&t, &model, c.parent().expect("parent"));
                let tg = cube_morph_targets_f64(&t, &model, c).expect("targets");
                let edge = dist(parent.verts[0].0, parent.verts[1].0);
                for tri in idx[..(6 * n * n) as usize].chunks(3) {
                    let v = [
                        tg.position[tri[0] as usize],
                        tg.position[tri[1] as usize],
                        tg.position[tri[2] as usize],
                    ];
                    for _ in 0..8 {
                        s = splitmix64(s);
                        let a = (s >> 11) as f64 / (1u64 << 53) as f64;
                        s = splitmix64(s);
                        let b = (s >> 11) as f64 / (1u64 << 53) as f64;
                        let (a, b) = if a + b > 1.0 {
                            (1.0 - a, 1.0 - b)
                        } else {
                            (a, b)
                        };
                        let p = DVec3::new(
                            v[0].x + a * (v[1].x - v[0].x) + b * (v[2].x - v[0].x),
                            v[0].y + a * (v[1].y - v[0].y) + b * (v[2].y - v[0].y),
                            v[0].z + a * (v[1].z - v[0].z) + b * (v[2].z - v[0].z),
                        );
                        let d = scale(p, 1.0 / len(p));
                        let (_, (q, _)) = best_tri(parent.tris(), d).expect("parent hit");
                        let dev = dist(p, q);
                        worst_abs = worst_abs.max(dev);
                        worst_rel = worst_rel.max(dev / edge);
                    }
                }
            }
            let angle =
                core::f64::consts::FRAC_PI_2 / (f64::from(n) * f64::from(1u32 << (level - 1)));
            rows.push((n, level, worst_abs, worst_rel, angle));
        }
    }
    let mut worst_ratio = 0.0f64;
    for (n, level, a, r, angle) in &rows {
        println!(
            "cube continuity n={n} level={level}: worst {a:.3e} m = {r:.3e} of a parent edge, parent cell angle {angle:.3e} rad (relative / angle^2 = {:.3})",
            r / (angle * angle)
        );
        worst_ratio = worst_ratio.max(r / (angle * angle));
    }
    println!("cube continuity worst (relative deviation / parent cell angle^2) = {worst_ratio:.4}");
    assert!(
        worst_ratio < 0.5,
        "relative deviation exceeds 0.5 * angle^2: ratio {worst_ratio}"
    );
}

#[test]
fn blend_matches_targets_at_the_ends() {
    let n = 4u32;
    let t = terrain(n);
    let model = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let cube = CubeSphere::new(n);
    let c = seeded_cube_cells(1, 5)[0];
    let m = cube
        .build_mesh(c, &t, &model, false, &|| false)
        .expect("mesh");
    for (v, mv) in m.vertices.iter().zip(&m.morph) {
        assert_eq!(blend(v.position, mv.position, 0.0), v.position);
        assert_eq!(blend(v.position, mv.position, 1.0), mv.position);
    }
}

// ---------------------------------------------------------------------------------------------
// hex

struct HexParent {
    n: u32,
    sectors: u32,
    /// Per sector, row-major `(s, t)` with `t <= n - s`, same order as the mesh.
    verts: Vec<(DVec3, [f32; 2])>,
}

fn sector_len(n: u32) -> u32 {
    (n + 1) * (n + 2) / 2
}

fn hex_parent(t: &PlanetTerrain, model: &LodModel, p: CellId) -> HexParent {
    let n = t.cells_per_edge();
    let mesh = build_hex_cell(p, t, model, false, &|| false).expect("parent mesh");
    let sectors = if is_pentagon(p) { 5 } else { 6 };
    let mut verts = Vec::new();
    let mut g = 0usize;
    for k in 0..sectors {
        for s in 0..=n {
            for tt in 0..=(n - s) {
                verts.push((
                    planetary_core::hex::mesh::hex_vertex_position_f64(t, p, n, k as u8, s, tt),
                    mesh.vertices[g].uv,
                ));
                g += 1;
            }
        }
    }
    HexParent { n, sectors, verts }
}

impl HexParent {
    fn at(&self, k: u32, s: u32, t: u32) -> (DVec3, [f32; 2]) {
        let n = self.n;
        let idx = k * sector_len(n) + s * (n + 1) - s * s.saturating_sub(1) / 2 + t;
        self.verts[idx as usize]
    }

    fn tris(&self) -> impl Iterator<Item = [(DVec3, [f32; 2]); 3]> + '_ {
        let n = self.n;
        (0..self.sectors).flat_map(move |k| {
            (0..n).flat_map(move |s| {
                (0..(n - s)).flat_map(move |t| {
                    let mut v =
                        vec![[self.at(k, s, t), self.at(k, s + 1, t), self.at(k, s, t + 1)]];
                    if t + 1 < n - s {
                        v.push([
                            self.at(k, s + 1, t),
                            self.at(k, s + 1, t + 1),
                            self.at(k, s, t + 1),
                        ]);
                    }
                    v
                })
            })
        })
    }
}

fn seeded_hex_cells(count: usize, seed: u64, pentagon_children: bool) -> Vec<CellId> {
    let mut out = Vec::new();
    let mut s = seed;
    while out.len() < count {
        s = splitmix64(s);
        let level = 2 + (out.len() % 5) as u8;
        let k = 1u64 << level;
        let base = 1 + ((s >> 4) % 10) as u8;
        let i = ((s >> 8) % k) as u32;
        let j = ((s >> 36) % k) as u32;
        let c = CellId::new(base, level, i, j).expect("cell");
        if !pentagon_children && is_pentagon(c) {
            continue;
        }
        out.push(c);
    }
    out
}

/// All vertex directions of a hex cell in mesh grid order.
fn hex_dirs(c: CellId, n: u32) -> Vec<DVec3> {
    let f = CellFrame::new(c, n);
    let mut out = Vec::new();
    for k in 0..f.sectors() {
        for s in 0..=n {
            for t in 0..=(n - s) {
                out.push(sample_dir(f.sample(k, s, t)));
            }
        }
    }
    out
}

struct HexOracle {
    /// Position, uv and the index (0 primary, 1 second) of the parent containing the direction;
    /// `None` when neither does.
    hit: Option<(Target, usize)>,
    /// Whether each parent contains the direction.
    in_parent: [bool; 2],
}

fn hex_oracle(grids: &[HexParent], d: DVec3) -> HexOracle {
    let mut res = HexOracle {
        hit: None,
        in_parent: [false; 2],
    };
    for (pi, g) in grids.iter().enumerate() {
        if let Some((_, tg)) = best_tri(g.tris(), d) {
            res.in_parent[pi] = true;
            if res.hit.is_none() {
                res.hit = Some((tg, pi));
            }
        }
    }
    res
}

fn parents_of(c: CellId) -> Vec<CellId> {
    let mut v = Vec::new();
    parents(c, &mut v);
    v
}

#[test]
fn hex_targets_match_the_brute_force_oracle() {
    let mut worst = 0.0f64;
    let mut worst_uv = 0.0f64;
    let (mut cells, mut vertices) = (0u32, 0u64);
    let (mut centre_cells, mut edge_cells, mut pent_cells) = (0u32, 0u32, 0u32);
    let (mut identity_edge, mut second, mut extra_walks, mut cone_tests) = (0u64, 0u64, 0u64, 0u64);
    let (mut centre_identity, mut centre_samples, mut edge_samples) = (0u64, 0u64, 0u64);
    for n in [2u32, 4, 8, 16] {
        let t = terrain(n);
        let model = hex_model(&t);
        let count = match n {
            16 => 5,
            8 => 12,
            _ => 20,
        };
        let mut cs = seeded_hex_cells(count, 0xA11CE ^ u64::from(n), false);
        // Children of pentagon roots: the centre child (a pentagon) and its edge children.
        for r in CellId::level_iter(0).filter(|c| is_pentagon(*c)).take(2) {
            let mut ch = Vec::new();
            planetary_core::hex::adjacency::children(r, &mut ch);
            cs.extend(ch.into_iter().take(if n == 16 { 2 } else { 4 }));
        }
        for c in cs {
            let ps = parents_of(c);
            assert!(!ps.is_empty() && ps.len() <= 2);
            let grids: Vec<HexParent> = ps.iter().map(|p| hex_parent(&t, &model, *p)).collect();
            let tg = hex_morph_targets_f64(c, &t, &model).expect("targets");
            let own = build_hex_cell_identity_morph(c, &t, &model, false, &|| false).expect("mesh");
            assert_eq!(tg.position.len(), own.vertices.len());
            let dirs = hex_dirs(c, n);
            let mut identity = 0u64;
            for (g, d) in dirs.iter().enumerate() {
                let o = hex_oracle(&grids, *d);
                match o.hit {
                    Some(((p, uv), pi)) => {
                        let e = dist(tg.position[g], p);
                        worst = worst.max(e);
                        assert!(e <= ORACLE_M, "{c:?} n={n} v{g}: {e} m (parent {pi})");
                        for q in 0..2 {
                            let eu = (f64::from(tg.uv[g][q]) - uv[q]).abs();
                            worst_uv = worst_uv.max(eu);
                            assert!(eu <= 2e-6, "{c:?} v{g} uv {eu}");
                        }
                    }
                    None => {
                        identity += 1;
                        // Outside both parents: morph == position.
                        let own_p = DVec3::new(
                            own.origin.x + f64::from(own.vertices[g].position[0]),
                            own.origin.y + f64::from(own.vertices[g].position[1]),
                            own.origin.z + f64::from(own.vertices[g].position[2]),
                        );
                        assert!(dist(tg.position[g], own_p) < 0.01, "{c:?} v{g} identity");
                    }
                }
                vertices += 1;
            }
            let s = tg.stats;
            assert_eq!(u64::from(s.identity), identity, "{c:?}: identity count");
            second += u64::from(s.second_parent);
            extra_walks += u64::from(s.extra_walks);
            cone_tests += s.cone_tests;
            if ps.len() == 1 {
                centre_cells += 1;
                centre_identity += identity;
                centre_samples += u64::from(s.parent_samples);
                assert_eq!(identity, 0, "centre child {c:?} must have no fallback");
            } else {
                edge_cells += 1;
                identity_edge += identity;
                edge_samples += u64::from(s.parent_samples);
            }
            if is_pentagon(c) {
                pent_cells += 1;
            }
            cells += 1;
        }
    }
    println!(
        "hex oracle: {cells} cells ({centre_cells} centre, {edge_cells} edge, {pent_cells} pentagon), {vertices} vertices, worst position error {worst:.3e} m, worst uv error {worst_uv:.3e}"
    );
    println!(
        "hex fallback: centre identity {centre_identity}, edge identity {identity_edge}, second-parent vertices {second}, extra sector walks {extra_walks} of {vertices}, triangle tests {cone_tests} ({:.1}/vertex), parent samples centre {centre_samples} edge {edge_samples}",
        cone_tests as f64 / vertices as f64
    );
    assert!(
        (extra_walks as f64) < 0.35 * vertices as f64,
        "extra walks {extra_walks} of {vertices}"
    );
}

#[test]
fn hex_parent_lattice_points_copy_bit_for_bit() {
    let n = 4u32;
    let t = terrain(n);
    let model = hex_model(&t);
    let mut checked = 0u64;
    for c in seeded_hex_cells(16, 0xC09E, true) {
        let ps = parents_of(c);
        let tg = hex_morph_targets_f64(c, &t, &model).expect("targets");
        let cf = CellFrame::new(c, n);
        let dirs = hex_dirs(c, n);
        let mut g = 0usize;
        for k in 0..cf.sectors() {
            for s in 0..=n {
                for tt in 0..=(n - s) {
                    let canon = cf.sample(k, s, tt).canonical();
                    'p: for p in &ps {
                        let pf = CellFrame::new(*p, n);
                        for pk in 0..pf.sectors() {
                            for ps_ in 0..=n {
                                for pt in 0..=(n - ps_) {
                                    if pf.sample(pk, ps_, pt).canonical() == canon {
                                        let want =
                                            planetary_core::hex::mesh::hex_vertex_position_f64(
                                                &t, *p, n, pk, ps_, pt,
                                            );
                                        assert_eq!(tg.position[g], want, "{c:?} g{g}");
                                        assert_eq!(
                                            dirs[g].x.to_bits(),
                                            sample_dir(pf.sample(pk, ps_, pt)).x.to_bits()
                                        );
                                        checked += 1;
                                        break 'p;
                                    }
                                }
                            }
                        }
                    }
                    g += 1;
                }
            }
        }
    }
    println!("hex bit-exact parent copies checked: {checked}");
    assert!(checked > 100);
}

#[test]
fn hex_half_cell_sides_lie_in_one_parent_each() {
    let n = 8u32;
    let t = terrain(n);
    let model = hex_model(&t);
    let mut cells = 0u32;
    for c in seeded_hex_cells(40, 0x4A1F, false) {
        let ps = parents_of(c);
        if ps.len() != 2 {
            continue;
        }
        let grids: Vec<HexParent> = ps.iter().map(|p| hex_parent(&t, &model, *p)).collect();
        let cf = CellFrame::new(c, n);
        let mut agreeing = 0;
        for diag in 0..3u32 {
            // side 0: sectors diag..diag+2; side 1: diag+3..diag+5 (mod 6).
            let mut sides: [Option<usize>; 2] = [None, None];
            let mut ok = true;
            for side in 0..2u32 {
                for off in 0..3u32 {
                    let k = (diag + 3 * side + off) % 6;
                    for s in 0..=n {
                        for tt in 0..=(n - s) {
                            // strictly inside the half: not on the diagonal sectors' axes
                            let on_diag = (off == 0 && tt == 0) || (off == 2 && s == 0);
                            if on_diag {
                                continue;
                            }
                            let d = sample_dir(cf.sample(k as u8, s, tt));
                            let o = hex_oracle(&grids, d);
                            // the parent that contains it and not the other
                            let which = match o.in_parent {
                                [true, false] => Some(0),
                                [false, true] => Some(1),
                                _ => None,
                            };
                            match (which, sides[side as usize]) {
                                (Some(w), None) => sides[side as usize] = Some(w),
                                (Some(w), Some(x)) if w == x => {}
                                (None, _) => {}
                                _ => ok = false,
                            }
                        }
                    }
                }
            }
            if ok && sides[0].is_some() && sides[1].is_some() && sides[0] != sides[1] {
                agreeing += 1;
            }
        }
        assert_eq!(
            agreeing, 1,
            "exactly one diagonal splits {c:?} into its two parents"
        );
        cells += 1;
    }
    println!("hex half-cell sides checked on {cells} edge children");
    assert!(cells >= 10);
}

/// Negative control for the oracle: searching only the wrong parent leaves the other half of an
/// edge child unlocated (identity), which the oracle comparison must flag; the correct search
/// (either order) passes it.
#[test]
fn hex_wrong_parent_is_detected_by_the_oracle() {
    let n = 4u32;
    let t = terrain(n);
    let model = hex_model(&t);
    let mut checked = 0u32;
    for c in seeded_hex_cells(30, 0xBAD, false) {
        let ps = parents_of(c);
        if ps.len() != 2 {
            continue;
        }
        let grids: Vec<HexParent> = ps.iter().map(|p| hex_parent(&t, &model, *p)).collect();
        let dirs = hex_dirs(c, n);
        let mismatches = |tg: &planetary_core::morph::MorphTargets| {
            dirs.iter()
                .enumerate()
                .filter(|(g, d)| {
                    hex_oracle(&grids, **d)
                        .hit
                        .is_some_and(|((p, _), _)| dist(tg.position[*g], p) > 1e-3)
                })
                .count()
        };
        let good = hex_morph_targets_f64(c, &t, &model).expect("targets");
        let swapped =
            hex_morph_targets_with_parents_f64(c, &t, &model, &[ps[1], ps[0]]).expect("targets");
        assert_eq!(mismatches(&good), 0);
        assert_eq!(mismatches(&swapped), 0, "parent order must not matter");
        for only in [ps[0], ps[1]] {
            let bad = hex_morph_targets_with_parents_f64(c, &t, &model, &[only]).expect("targets");
            assert!(
                mismatches(&bad) > 0,
                "{c:?}: a single parent leaves vertices unlocated"
            );
        }
        checked += 1;
    }
    println!("hex wrong-parent negative control: {checked} edge children");
    assert!(checked >= 8);
}

#[test]
fn hex_roots_identity_vertices_unchanged_and_debug() {
    let n = 4u32;
    let t = terrain(n);
    let model = hex_model(&t);
    for r in CellId::level_iter(0) {
        let m = build_hex_cell(r, &t, &model, false, &|| false).expect("mesh");
        assert_eq!(m.morph, identity_morph(&m.vertices));
    }
    for c in seeded_hex_cells(8, 0xDB6, true) {
        let m = build_hex_cell(c, &t, &model, false, &|| false).expect("mesh");
        let o = build_hex_cell_identity_morph(c, &t, &model, false, &|| false).expect("mesh");
        assert_eq!(m.vertices, o.vertices, "vertices unchanged for {c:?}");
        assert_eq!(m.morph.len(), m.vertices.len());
        let tg = hex_morph_targets_f64(c, &t, &model).expect("targets");
        assert_eq!(m.morph, tg.to_vertices(m.origin));
        let dbg = build_hex_cell(c, &t, &model, true, &|| false).expect("mesh");
        for (v, mv) in dbg.vertices.iter().zip(&dbg.morph) {
            assert_eq!(mv.uv[1], v.uv[1], "debug colour must not blend");
        }
    }
    // Cancellation propagates.
    let c = seeded_hex_cells(1, 3, false)[0];
    assert!(build_hex_cell(c, &t, &model, false, &|| true).is_none());
}

/// Child triangles at morph 1 against the parent surface.
#[test]
fn hex_child_triangles_at_morph_one_follow_the_parent_surface() {
    let n = 4u32;
    let t = terrain(n);
    let model = hex_model(&t);
    let variants = planetary_core::hex::mesh::hex_index_variants(n);
    // (centre child, parent inside one icosahedron face, worst metres, worst relative, bound)
    let mut rows: Vec<(bool, bool, f64, f64, f64)> = Vec::new();
    for c in seeded_hex_cells(28, 0x51E7, true) {
        let ps = parents_of(c);
        let grids: Vec<HexParent> = ps.iter().map(|p| hex_parent(&t, &model, *p)).collect();
        let tg = hex_morph_targets_f64(c, &t, &model).expect("targets");
        let idx = &variants[usize::from(is_pentagon(c))].indices;
        let sector_tris = (if is_pentagon(c) { 5 } else { 6 }) * 3 * n * n;
        // The child lattice nests exactly in the parent lattice only when child and parents
        // are sampled on one icosahedron face.
        let faces_of = |cell: CellId| -> Vec<u8> {
            let f = CellFrame::new(cell, n);
            let mut v = Vec::new();
            for k in 0..f.sectors() {
                for (s, tt) in [(0, 0), (n, 0), (0, n), (n, 1), (1, n)] {
                    v.push(f.sample(k, s, tt).face);
                }
            }
            v.sort_unstable();
            v.dedup();
            v
        };
        let mut all = faces_of(c);
        for p in &ps {
            all.extend(faces_of(*p));
        }
        all.sort_unstable();
        all.dedup();
        let inside_face = all.len() == 1;
        let mut s = splitmix64(c.raw());
        let edge = dist(grids[0].verts[0].0, grids[0].at(0, n, 0).0);
        let (mut worst_abs, mut worst_rel) = (0.0f64, 0.0f64);
        for tri in idx[..sector_tris as usize].chunks(3) {
            let v = [
                tg.position[tri[0] as usize],
                tg.position[tri[1] as usize],
                tg.position[tri[2] as usize],
            ];
            for _ in 0..8 {
                s = splitmix64(s);
                let a = (s >> 11) as f64 / (1u64 << 53) as f64;
                s = splitmix64(s);
                let b = (s >> 11) as f64 / (1u64 << 53) as f64;
                let (a, b) = if a + b > 1.0 {
                    (1.0 - a, 1.0 - b)
                } else {
                    (a, b)
                };
                let p = DVec3::new(
                    v[0].x + a * (v[1].x - v[0].x) + b * (v[2].x - v[0].x),
                    v[0].y + a * (v[1].y - v[0].y) + b * (v[2].y - v[0].y),
                    v[0].z + a * (v[1].z - v[0].z) + b * (v[2].z - v[0].z),
                );
                let d = scale(p, 1.0 / len(p));
                let o = hex_oracle(&grids, d);
                if let Some(((q, _), _)) = o.hit {
                    let dev = dist(p, q);
                    worst_abs = worst_abs.max(dev);
                    worst_rel = worst_rel.max(dev / edge);
                }
            }
        }
        let l = c.level() - 1;
        let bound = model.residual(l) + model.sagitta(l);
        rows.push((ps.len() == 1, inside_face, worst_abs, worst_rel, bound));
    }
    let (mut a_cells, mut b_cells) = (0u32, 0u32);
    let (mut a_worst, mut b_worst_ratio) = (0.0f64, 0.0f64);
    for (centre, inside, abs, rel, bound) in &rows {
        println!(
            "hex continuity ({}, {}): worst {abs:.3e} m = {rel:.3e} of a parent sector edge, model bound {bound:.3e} m",
            if *centre { "centre" } else { "edge" },
            if *inside {
                "inside one face"
            } else {
                "touches an icosahedron edge or vertex"
            }
        );
        if *inside {
            a_cells += 1;
            a_worst = a_worst.max(*rel);
        } else {
            b_cells += 1;
            b_worst_ratio = b_worst_ratio.max(abs / bound);
        }
    }
    println!(
        "hex continuity groups: inside-face {a_cells} cells (worst {a_worst:.3e} of an edge), face-crossing {b_cells} cells (worst deviation / (residual + sagitta) = {b_worst_ratio:.3e})"
    );
    assert!(a_cells >= 3 && b_cells >= 3, "both groups sampled");
    assert!(a_worst <= 1e-6, "nested cells deviate {a_worst}");
    assert!(
        b_worst_ratio <= 1.0,
        "face-crossing cells exceed the selection error model"
    );
}
