//! `Topology` contract tests run for both implementations, plus generic selection on the hex
//! topology at the real default parameters (max level 16, 8192 visits, 900 pieces).

use std::collections::BTreeSet;

use engine_sdk::DVec3;
use planetary_core::hex::CellId;
use planetary_core::hex::mesh::{vertex_count_hex, vertex_count_pentagon};
use planetary_core::lod::{DEFAULT_KAPPA, LodModel, LodParams, View};
use planetary_core::topology::select::{close_balance, select_cells};
use planetary_core::{
    CubeSphere, HexSphere, PlanetTerrain, TerrainParams, Topology, decode_cell_payload,
    encode_cell_payload,
};

const SEED: u64 = 0x0007_5EED_2026_0001;
const R: f64 = 6_371_000.0;

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn dot(a: DVec3, b: DVec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

/// All cells to `exhaustive` levels, plus `samples` random descents to `max_level`.
fn cells_of<T: Topology>(t: &T, exhaustive: u8, max_level: u8, samples: u32) -> Vec<T::Cell> {
    let mut all: Vec<T::Cell> = t.roots().to_vec();
    let mut frontier = all.clone();
    let mut kids = Vec::new();
    for _ in 0..exhaustive {
        let mut next = Vec::new();
        for &c in &frontier {
            kids.clear();
            t.children(c, &mut kids);
            next.extend_from_slice(&kids);
        }
        all.extend_from_slice(&next);
        frontier = next;
    }
    let mut s = 0xC0DE_u64;
    for _ in 0..samples {
        let roots = t.roots();
        let mut c = roots[(splitmix(&mut s) % roots.len() as u64) as usize];
        while t.level(c) < max_level {
            kids.clear();
            t.children(c, &mut kids);
            if kids.is_empty() {
                break;
            }
            c = kids[(splitmix(&mut s) % kids.len() as u64) as usize];
            all.push(c);
        }
    }
    all
}

fn check_contract<T: Topology>(t: &T, max_level: u8, samples: u32) {
    let cells = cells_of(t, 5, max_level, samples);
    let (mut kids, mut pars, mut nbs, mut nbs2, mut req) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    assert!(!t.roots().is_empty());
    for &r in t.roots() {
        assert_eq!(t.level(r), 0);
        pars.clear();
        t.parents(r, &mut pars);
        assert!(pars.is_empty());
        assert_eq!(t.primary_parent(r), None);
    }
    for &c in &cells {
        assert_eq!(t.from_raw(t.raw(c)), Some(c), "{c:?}");
        kids.clear();
        t.children(c, &mut kids);
        for &k in &kids {
            assert_eq!(t.level(k), t.level(c) + 1);
            pars.clear();
            t.parents(k, &mut pars);
            assert!(pars.contains(&c), "{c:?} not a parent of its child {k:?}");
        }
        pars.clear();
        t.parents(c, &mut pars);
        if t.level(c) > 0 {
            assert!(!pars.is_empty());
            let min = pars.iter().copied().min_by_key(|p| t.raw(*p));
            assert_eq!(t.primary_parent(c), min, "{c:?}");
            assert!(pars.windows(2).all(|w| t.raw(w[0]) < t.raw(w[1])));
            assert!(pars.iter().all(|p| t.level(*p) + 1 == t.level(c)));
        }
        nbs.clear();
        t.neighbors(c, &mut nbs);
        assert!(!nbs.is_empty());
        for &n in &nbs {
            assert_eq!(t.level(n), t.level(c));
            nbs2.clear();
            t.neighbors(n, &mut nbs2);
            assert!(
                nbs2.contains(&c),
                "neighbours of {c:?} not reciprocal: {n:?}"
            );
        }
        req.clear();
        t.required_splits(c, &mut req);
        assert!(req.iter().all(|r| t.level(*r) < t.level(c)), "{c:?}");
        if t.level(c) > 0 {
            assert!(!req.is_empty());
            for p in &pars {
                assert!(req.contains(p));
            }
        }
        let fp = t.footprint(c);
        assert_eq!(
            t.locate(fp.center, t.level(c)),
            c,
            "locate(centre) != cell {c:?}"
        );
    }
    // Closure terminates and is a fixpoint on arbitrary cells.
    let mut split: BTreeSet<T::Cell> = cells.iter().copied().step_by(37).collect();
    close_balance(t, &mut split);
    assert_eq!(close_balance(t, &mut split), 0);
    assert_eq!(t.from_raw(u64::MAX), None);
}

/// Mesh checks on sampled cells: vertices inside the footprint cone; payload v3 round trip.
fn check_meshes<T: Topology>(t: &T, n: u32, counts: &dyn Fn(u8) -> Option<u32>) {
    let terrain = PlanetTerrain::new(SEED, TerrainParams::default(), n).expect("terrain");
    let model = LodModel::for_topology(&terrain, DEFAULT_KAPPA, t.sample_angle0());
    let cells = cells_of(t, 1, 9, 6);
    let mut checked = 0;
    for c in cells.into_iter().step_by(3) {
        let m = t
            .build_mesh(c, &terrain, &model, false, &|| false)
            .expect("mesh");
        assert_eq!(m.raw, t.raw(c));
        let fp = t.footprint(c);
        for v in &m.vertices {
            let p = DVec3::new(
                m.origin.x + f64::from(v.position[0]),
                m.origin.y + f64::from(v.position[1]),
                m.origin.z + f64::from(v.position[2]),
            );
            let len = dot(p, p).sqrt();
            let d = DVec3::new(p.x / len, p.y / len, p.z / len);
            assert!(
                dot(d, fp.center) >= fp.cos_delta - 1e-7,
                "vertex outside the footprint of {c:?}: {} < {}",
                dot(d, fp.center),
                fp.cos_delta
            );
        }
        let bytes = encode_cell_payload(&m);
        let back = decode_cell_payload(&bytes, m.raw, counts).expect("decode");
        assert_eq!(m.morph.len(), m.vertices.len());
        assert_eq!(back, m);
        assert!(decode_cell_payload(&bytes, m.raw ^ 1, counts).is_err());
        assert!(decode_cell_payload(&bytes[..bytes.len() - 1], m.raw, counts).is_err());
        checked += 1;
    }
    assert!(checked > 5);
}

#[test]
fn trait_contract_holds_for_both() {
    let cube = CubeSphere::new(16);
    check_contract(&cube, 12, 40);
    let hex = HexSphere::new(16).expect("hex");
    assert_eq!(hex.roots().len(), 12);
    check_contract(&hex, 12, 40);
    assert_eq!(cube.name(), "cube-sphere");
    assert_eq!(hex.name(), "hex");
}

#[test]
fn meshes_and_payload_v3_hold_for_both() {
    let cube = CubeSphere::new(16);
    check_meshes(&cube, 16, &|c| {
        (c == 0).then(|| planetary_core::vertex_count(16) as u32)
    });
    let hex = HexSphere::new(16).expect("hex");
    check_meshes(&hex, 16, &|c| match c {
        0 => Some(vertex_count_hex(16)),
        1 => Some(vertex_count_pentagon(16)),
        _ => None,
    });
}

#[test]
fn hex_new_rejects_bad_sector_side() {
    for n in [0u32, 1, 3, 15, 65, 66, 128, u32::MAX] {
        assert!(HexSphere::new(n).is_none(), "{n}");
    }
    for n in [2u32, 4, 16, 64] {
        assert!(HexSphere::new(n).is_some(), "{n}");
    }
}

fn hex_model(hex: &HexSphere) -> LodModel {
    let terrain =
        PlanetTerrain::new(SEED, TerrainParams::default(), hex.sector_side()).expect("terrain");
    LodModel::for_topology(&terrain, DEFAULT_KAPPA, hex.sample_angle0())
}

fn view_at(eye: DVec3) -> View {
    View {
        eye,
        proj_scale: 720.0 / (2.0 * (1.0 / 3.0f64.sqrt())),
        frustum: None,
    }
}

fn at(dir: DVec3, alt: f64) -> DVec3 {
    let l = dot(dir, dir).sqrt();
    let s = (R + alt) / l;
    DVec3::new(dir.x * s, dir.y * s, dir.z * s)
}

#[test]
fn hex_variants_match_pieces() {
    let hex = HexSphere::new(16).expect("hex");
    let vars = hex.index_variants();
    assert_eq!(vars.len(), 8);
    let max_index = |i: usize| vars[i].indices.iter().copied().max().expect("indices") + 1;
    // Together the variants of a class use exactly the vertices of that class.
    for (class, count) in [(0u8, vertex_count_hex(16)), (1, vertex_count_pentagon(16))] {
        let used = vars
            .iter()
            .filter(|v| v.vertex_class == class)
            .flat_map(|v| v.indices.iter().copied())
            .max()
            .expect("indices")
            + 1;
        assert_eq!(used, count, "class {class}");
    }
    assert_eq!(vars[0].vertex_class, 0);
    assert_eq!(vars[1].vertex_class, 1);
    assert!(max_index(0) <= vertex_count_hex(16));
    assert!(max_index(1) <= vertex_count_pentagon(16));
    for (i, v) in vars.iter().enumerate().skip(2) {
        assert_eq!(v.vertex_class, 0);
        assert!(max_index(i) <= vertex_count_hex(16), "{}", v.name);
    }
    // Pieces of real selections only use existing variants of the right class.
    let (model, params) = (hex_model(&hex), LodParams::default());
    let mut seen = BTreeSet::new();
    for (i, &r) in hex.roots().iter().enumerate() {
        let eye = at(hex.footprint(r).center, 1000.0);
        let sel = select_cells(
            &hex,
            &model,
            &params,
            &view_at(eye),
            &BTreeSet::new(),
            &|_: CellId| None,
        );
        for p in &sel.pieces {
            let v = &vars[usize::from(p.variant)];
            let pentagon = planetary_core::hex::adjacency::is_pentagon(p.cell);
            assert_eq!(v.vertex_class == 1, pentagon, "view {i}: {p:?}");
            seen.insert(p.variant);
        }
    }
    assert!(seen.len() >= 2, "variants seen: {seen:?}");
}

#[test]
fn hex_selection_within_budgets() {
    let hex = HexSphere::new(16).expect("hex");
    let model = hex_model(&hex);
    let params = LodParams::default();
    assert_eq!(
        (
            params.max_level,
            params.max_nodes_visited,
            params.max_leaves
        ),
        (16, 8192, 900)
    );
    // Look-at directions: a generic point, a pentagon and an icosahedron edge midpoint.
    let roots = hex.roots();
    let (a, b) = (
        hex.footprint(roots[0]).center,
        hex.footprint(roots[1]).center,
    );
    let dirs = [
        DVec3::new(0.3, 0.5, 0.8),
        a,
        DVec3::new(a.x + b.x, a.y + b.y, a.z + b.z),
    ];
    let mut req = Vec::new();
    let mut reached = Vec::new();
    for (di, dir) in dirs.iter().enumerate() {
        for (label, alt) in [
            ("3 m", 3.0),
            ("1 km", 1000.0),
            ("100 km", 100_000.0),
            ("2R", R),
        ] {
            let sel = select_cells(
                &hex,
                &model,
                &params,
                &view_at(at(*dir, alt)),
                &BTreeSet::new(),
                &|_: CellId| None,
            );
            let deepest = sel
                .pieces
                .iter()
                .map(|p| hex.level(p.cell))
                .max()
                .unwrap_or(0);
            println!(
                "hex dir{di} {label}: nodes {} pieces {} split {} deepest {} forced {} trimmed {} node_budget_hit {} leaf_budget_hit {}",
                sel.stats.nodes_visited,
                sel.pieces.len(),
                sel.split.len(),
                deepest,
                sel.stats.forced_splits,
                sel.stats.trimmed_splits,
                sel.stats.node_budget_hit,
                sel.stats.leaf_budget_hit,
            );
            assert!(
                sel.pieces.len() <= 900,
                "{label}: {} pieces",
                sel.pieces.len()
            );
            assert!(sel.stats.nodes_visited <= 8192, "{label}");
            // Closed: every required split of a split cell is split.
            for &c in &sel.split {
                req.clear();
                hex.required_splits(c, &mut req);
                assert!(req.iter().all(|r| sel.split.contains(r)), "{label}: {c:?}");
            }
            if alt == 3.0 {
                let look = hex.locate(*dir, 16);
                let at_look = sel.pieces.iter().any(|p| p.cell == look);
                println!("hex dir{di} {label}: level-16 piece at the look-at point: {at_look}");
                reached.push((di, deepest, at_look));
            }
            // Re-closing is a no-op.
            let mut again = sel.split.clone();
            assert_eq!(close_balance(&hex, &mut again), 0);
        }
    }
    assert_eq!(reached.len(), dirs.len());
}

/// 07.1-16 must-have: from 3 m the look-at point is refined to level 16 within the real budgets
/// (met with the parents-only closure, ADR-0011; the full closure stopped at level 12).
#[test]
fn hex_selection_reaches_max_level() {
    let hex = HexSphere::new(16).expect("hex");
    let model = hex_model(&hex);
    let params = LodParams::default();
    let roots = hex.roots();
    let (a, b) = (
        hex.footprint(roots[0]).center,
        hex.footprint(roots[1]).center,
    );
    let dirs = [
        DVec3::new(0.3, 0.5, 0.8),
        a,
        DVec3::new(a.x + b.x, a.y + b.y, a.z + b.z),
    ];
    for (di, dir) in dirs.iter().enumerate() {
        let sel = select_cells(
            &hex,
            &model,
            &params,
            &view_at(at(*dir, 3.0)),
            &BTreeSet::new(),
            &|_: CellId| None,
        );
        let look = hex.locate(*dir, 16);
        assert!(
            sel.pieces.iter().any(|p| p.cell == look),
            "dir{di}: no level-16 piece at the look-at point"
        );
    }
}
