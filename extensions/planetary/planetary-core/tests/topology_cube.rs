//! `CubeSphere` as a `Topology`: trait behaviours and oracles against the pre-trait cube code.

use std::collections::BTreeSet;

use engine_sdk::DVec3;
use planetary_core::lod::{
    DEFAULT_KAPPA, HeightRange, LodModel, LodParams, SelectStats, View, balance, leaves_of,
    reference_views,
};
use planetary_core::topology::select::{close_balance, select_cells};
use planetary_core::{
    CubeSphere, Edge, Face, PatchId, PlanetTerrain, TerrainParams, Topology, build_patch,
    grid_indices, neighbor,
};

/// Cube-sphere selection through the generic `select_cells` (the `lod::select` wrapper is gone).
struct Selection {
    split: BTreeSet<PatchId>,
    leaves: Vec<PatchId>,
    stats: SelectStats,
}

fn select(
    m: &LodModel,
    p: &LodParams,
    v: &View,
    prev: &BTreeSet<PatchId>,
    resident: &dyn Fn(PatchId) -> Option<HeightRange>,
) -> Selection {
    let s = select_cells(&CubeSphere::new(m.cells_per_edge), m, p, v, prev, resident);
    Selection {
        split: s.split,
        leaves: s.pieces.iter().map(|q| q.cell).collect(),
        stats: s.stats,
    }
}

const R: f64 = 6_371_000.0;
const RELIEF: f64 = 3800.0;
const SEED: u64 = 0x0007_5EED_2026_0001;

fn model() -> LodModel {
    LodModel {
        radius: R,
        height: HeightRange {
            min: -RELIEF,
            max: RELIEF,
        },
        residual: [0.0; 25],
        cells_per_edge: 32,
        sample_angle0: core::f64::consts::FRAC_PI_2 / 32.0,
        kappa: 0.25,
    }
}

fn view_at(eye: DVec3) -> View {
    View {
        eye,
        proj_scale: 720.0 / (2.0 * (1.0 / 3.0f64.sqrt())),
        frustum: None,
    }
}

fn fnv(h: &mut u64, v: u64) {
    for b in v.to_le_bytes() {
        *h ^= u64::from(b);
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

fn digest(split: &BTreeSet<PatchId>, leaves: &[PatchId], s: &SelectStats) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for c in split {
        fnv(&mut h, c.raw());
    }
    fnv(&mut h, leaves.len() as u64);
    for l in leaves {
        fnv(&mut h, l.raw());
    }
    for v in [
        u64::from(s.nodes_visited),
        u64::from(s.culled_horizon),
        u64::from(s.culled_frustum),
        s.leaves as u64,
        u64::from(s.forced_splits),
        u64::from(s.trimmed_splits),
        u64::from(s.node_budget_hit),
        u64::from(s.leaf_budget_hit),
    ] {
        fnv(&mut h, v);
    }
    h
}

/// Digests of the reference views (cold), a tight-budget case and a 100-step descent (with
/// hysteresis state).
fn select_digests() -> Vec<u64> {
    let m = model();
    let p = LodParams::default();
    let mut out = Vec::new();
    for (_, eye) in reference_views(m.radius, m.height.max) {
        let s = select(&m, &p, &view_at(eye), &BTreeSet::new(), &|_| None);
        out.push(digest(&s.split, &s.leaves, &s.stats));
    }
    // Tight budgets exercise the trim path.
    let tight = LodParams {
        max_leaves: 60,
        ..LodParams::default()
    };
    let s = select(
        &m,
        &tight,
        &view_at(DVec3::new(R + RELIEF + 100.0, 0.0, 0.0)),
        &BTreeSet::new(),
        &|_| None,
    );
    out.push(digest(&s.split, &s.leaves, &s.stats));
    let mut prev = BTreeSet::new();
    let (far, near) = (2.0 * R, R + 3.0);
    let ratio = (near / far).powf(1.0 / 99.0);
    let mut d = far;
    for _ in 0..100 {
        let s = select(&m, &p, &view_at(DVec3::new(d, 0.0, 0.0)), &prev, &|_| None);
        out.push(digest(&s.split, &s.leaves, &s.stats));
        prev = s.split;
        d *= ratio;
    }
    out
}

/// FNV of the per-step digests, captured from the cube code before the Topology refactor.
const GOLDEN_HASH: u64 = 0xee2d_a365_46c6_e081;
const GOLDEN_COUNT: usize = 131;

#[test]
fn cube_select_matches_pre_refactor_goldens() {
    let d = select_digests();
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for v in &d {
        fnv(&mut h, *v);
    }
    assert_eq!(d.len(), GOLDEN_COUNT);
    assert_eq!(h, GOLDEN_HASH);
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn random_cell(s: &mut u64, max_level: u8) -> PatchId {
    let level = (splitmix(s) % (u64::from(max_level) + 1)) as u8;
    let face = Face::ALL[(splitmix(s) % 6) as usize];
    let k = 1u64 << level;
    let x = (splitmix(s) % k) as u32;
    let y = (splitmix(s) % k) as u32;
    PatchId::new(face, level, x, y).expect("cell")
}

#[test]
fn cube_roots_levels_and_raw_match_patch_id() {
    let t = CubeSphere::new(32);
    assert_eq!(t.roots().len(), 6);
    let mut s = 3u64;
    for r in t.roots() {
        assert_eq!(t.level(*r), 0);
    }
    for _ in 0..200 {
        let c = random_cell(&mut s, 12);
        assert_eq!(t.level(c), c.level());
        assert_eq!(t.raw(c), c.raw());
        assert_eq!(t.from_raw(c.raw()), Some(c));
        let mut ch = Vec::new();
        t.children(c, &mut ch);
        assert_eq!(ch.len(), 4);
        let mut parents = Vec::new();
        t.parents(ch[2], &mut parents);
        assert_eq!(parents, vec![c]);
        assert_eq!(t.primary_parent(ch[2]), Some(c));
    }
    assert_eq!(t.from_raw(u64::MAX), None);
}

#[test]
fn cube_required_splits_match_balance_rule() {
    let t = CubeSphere::new(32);
    let mut s = 11u64;
    for _ in 0..300 {
        let c = random_cell(&mut s, 7);
        let mut got = Vec::new();
        t.required_splits(c, &mut got);
        let mut want = Vec::new();
        if let (Some(parent), Some(children)) = (c.parent(), c.children()) {
            want.push(parent);
            for p in children {
                for e in Edge::ALL {
                    if let Some(g) = neighbor(p, e).patch.ancestor_at(c.level() - 1) {
                        want.push(g);
                    }
                }
            }
        }
        assert_eq!(got, want, "{c:?}");
        assert!(got.iter().all(|r| r.level() < c.level()));
        // Splitting `c` alone and running `balance` yields exactly the closure of these cells.
        let mut a: BTreeSet<PatchId> = BTreeSet::new();
        a.insert(c);
        let mut b = a.clone();
        let forced_a = balance(&mut a);
        let forced_b = close_balance(&t, &mut b);
        assert_eq!(a, b, "{c:?}");
        assert_eq!(forced_a, forced_b, "{c:?}");
        for r in &got {
            assert!(b.contains(r));
        }
    }
}

#[test]
fn close_balance_matches_balance_oracle() {
    let t = CubeSphere::new(32);
    let mut s = 0xBA1A_0CE5u64;
    for _ in 0..200 {
        let mut a: BTreeSet<PatchId> = BTreeSet::new();
        let n = 1 + splitmix(&mut s) % 6;
        for _ in 0..n {
            let mut cur = Some(random_cell(&mut s, 7));
            while let Some(c) = cur {
                a.insert(c);
                cur = c.parent();
            }
        }
        let mut b = a.clone();
        let fa = balance(&mut a);
        let fb = close_balance(&t, &mut b);
        assert_eq!(a, b);
        assert_eq!(fa, fb);
        // Idempotent: a fixpoint inserts nothing.
        assert_eq!(close_balance(&t, &mut b), 0);
    }
}

#[test]
fn cube_pieces_are_leaves() {
    let t = CubeSphere::new(32);
    let mut split = BTreeSet::new();
    let mut s = 5u64;
    for _ in 0..4 {
        let mut cur = Some(random_cell(&mut s, 6));
        while let Some(c) = cur {
            split.insert(c);
            cur = c.parent();
        }
    }
    balance(&mut split);
    let mut pieces = Vec::new();
    t.pieces(&split, &mut pieces);
    let leaves = leaves_of(&split);
    assert_eq!(pieces.len(), leaves.len());
    for (p, l) in pieces.iter().zip(&leaves) {
        assert_eq!(p.cell, *l);
        assert_eq!(p.variant, 0);
    }
    let v = t.index_variants();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].vertex_class, 0);
    assert_eq!(v[0].indices, grid_indices(32));
}

#[test]
fn cube_model_angle_matches_from_terrain() {
    for n in [16u32, 32] {
        let terrain = PlanetTerrain::new(SEED, TerrainParams::default(), n).expect("terrain");
        let cube = CubeSphere::new(n);
        let a = LodModel::from_terrain(&terrain, DEFAULT_KAPPA);
        let b = LodModel::for_topology(&terrain, DEFAULT_KAPPA, cube.sample_angle0());
        assert_eq!(a, b);
        for l in 0..20u8 {
            assert_eq!(a.cell_size(l).to_bits(), b.cell_size(l).to_bits());
            assert_eq!(
                a.geometric_error(l).to_bits(),
                b.geometric_error(l).to_bits()
            );
            // Same value the original `FRAC_PI_2 / (N * 2^L)` formula gave.
            let old = core::f64::consts::FRAC_PI_2 / (f64::from(n) * f64::from(1u32 << l));
            assert_eq!(a.cell_size(l).to_bits(), (a.radius * old).to_bits());
        }
    }
}

#[test]
fn cube_mesh_matches_build_patch() {
    let n = 16;
    let terrain = PlanetTerrain::new(SEED, TerrainParams::default(), n).expect("terrain");
    let model = LodModel::from_terrain(&terrain, DEFAULT_KAPPA);
    let cube = CubeSphere::new(n);
    let mut s = 21u64;
    for _ in 0..4 {
        let id = random_cell(&mut s, 6);
        let want = build_patch(&terrain, &model, id);
        let got = cube
            .build_mesh(id, &terrain, &model, false, &|| false)
            .expect("mesh");
        assert_eq!(got.raw, id.raw());
        assert_eq!(got.vertex_class, 0);
        assert_eq!(got.origin, want.origin);
        assert_eq!(got.vertices, want.vertices);
        assert_eq!(got.bound_radius, want.bound_radius);
        assert_eq!(got.skirt_depth, want.skirt_depth);
        assert_eq!(
            (got.height_min, got.height_max),
            (want.height_min, want.height_max)
        );
        let dbg = cube
            .build_mesh(id, &terrain, &model, true, &|| false)
            .expect("mesh");
        let row = ((u32::from(id.level()) * 7 + (id.raw() >> 7) as u32) % 16) as f32;
        for (a, b) in dbg.vertices.iter().zip(&want.vertices) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.normal, b.normal);
            assert_eq!(a.uv[0], b.uv[0]);
            assert_eq!(a.uv[1], (row + 0.5) / 16.0);
        }
        assert!(
            cube.build_mesh(id, &terrain, &model, false, &|| true)
                .is_none()
        );
    }
}
