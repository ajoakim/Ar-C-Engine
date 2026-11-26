//! Error model, bounds, 2:1 balance and partition validators.

use std::collections::BTreeSet;

use engine_sdk::DVec3;
use planetary_core::lod::{
    LodModel, balance, is_balanced, is_partition, leaves_of, max_level_jump,
};
use planetary_core::{Face, PatchId, patch_corner_dirs};

const R: f64 = 6_371_000.0;

fn model() -> LodModel {
    LodModel {
        radius: R,
        max_relief: 3800.0,
        cells_per_edge: 32,
        kappa: 0.25,
    }
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn insert_closed(split: &mut BTreeSet<PatchId>, p: PatchId) {
    let mut cur = Some(p);
    while let Some(c) = cur {
        split.insert(c);
        cur = c.parent();
    }
}

#[test]
fn geometric_error_decreases_and_matches_formula() {
    let m = model();
    let cell0 = R * core::f64::consts::FRAC_PI_2 / 32.0;
    assert!((m.cell_size(0) - cell0).abs() < 1e-6);
    for l in 0..19u8 {
        assert!(m.geometric_error(l + 1) < m.geometric_error(l));
    }
    for l in 0..=19u8 {
        let want = (0.25 * m.cell_size(l)).max(m.sagitta(l));
        assert_eq!(m.geometric_error(l), want);
    }
}

#[test]
fn node_bound_contains_corner_vertices() {
    let m = model();
    let mut s = 7u64;
    for _ in 0..200 {
        let level = (splitmix(&mut s) % 20) as u8;
        let face = Face::ALL[(splitmix(&mut s) % 6) as usize];
        let n = 1u64 << level;
        let x = (splitmix(&mut s) % n) as u32;
        let y = (splitmix(&mut s) % n) as u32;
        let id = PatchId::new(face, level, x, y).unwrap();
        let b = m.node_bound(id);
        for d in patch_corner_dirs(id) {
            for h in [-m.max_relief, m.max_relief] {
                let p = DVec3::new(d.x * (R + h), d.y * (R + h), d.z * (R + h));
                assert!((p - b.center).length() <= b.radius);
            }
        }
    }
}

fn corner_chain() -> BTreeSet<PatchId> {
    let mut split = BTreeSet::new();
    let mut p = PatchId::root(Face::PosX);
    split.insert(p);
    for _ in 0..8 {
        p = p.children().unwrap()[0];
        split.insert(p);
    }
    split
}

#[test]
fn corner_chain_balances_across_faces() {
    let mut split = corner_chain();
    let before = split.clone();
    assert!(max_level_jump(&leaves_of(&split)) >= 2);
    let forced = balance(&mut split);
    assert!(forced > 0);
    let leaves = leaves_of(&split);
    assert!(is_partition(&leaves));
    assert!(is_balanced(&leaves));
    assert!(before.is_subset(&split));
    assert!(split.difference(&before).any(|p| p.face() != Face::PosX));
}

#[test]
fn balance_idempotent() {
    let mut split = corner_chain();
    balance(&mut split);
    let snap = split.clone();
    assert_eq!(balance(&mut split), 0);
    assert_eq!(split, snap);
}

#[test]
fn random_split_sets_balance() {
    let mut s = 42u64;
    for _ in 0..50 {
        let mut split = BTreeSet::new();
        let count = 1 + splitmix(&mut s) % 12;
        for _ in 0..count {
            let level = (splitmix(&mut s) % 9) as u8;
            let face = Face::ALL[(splitmix(&mut s) % 6) as usize];
            let n = 1u64 << level;
            let x = (splitmix(&mut s) % n) as u32;
            let y = (splitmix(&mut s) % n) as u32;
            insert_closed(&mut split, PatchId::new(face, level, x, y).unwrap());
        }
        assert!(is_partition(&leaves_of(&split)));
        balance(&mut split);
        let leaves = leaves_of(&split);
        assert!(is_partition(&leaves));
        assert!(is_balanced(&leaves));
    }
}

#[test]
fn is_partition_rejects_bad_sets() {
    let roots: Vec<PatchId> = Face::ALL.iter().map(|f| PatchId::root(*f)).collect();
    assert!(is_partition(&roots));
    assert!(!is_partition(&[]));
    assert!(!is_partition(&roots[1..]));
    let mut with_child = roots.clone();
    with_child.push(roots[0].children().unwrap()[0]);
    assert!(!is_partition(&with_child));
    let mut split = BTreeSet::new();
    split.insert(roots[0]);
    let mut leaves = leaves_of(&split);
    assert!(is_partition(&leaves));
    leaves.pop();
    assert!(!is_partition(&leaves));
}

#[test]
fn max_level_jump_detects_negative_control() {
    // Split PosX twice on the child touching its +U edge; neighbors stay coarse.
    let mut split = BTreeSet::new();
    let root = PatchId::root(Face::PosX);
    split.insert(root);
    let c = root.children().unwrap()[1]; // x=1,y=0
    split.insert(c);
    let gc = c.children().unwrap()[1]; // x=3,y=0 -> touches the +U cube edge
    split.insert(gc);
    let leaves = leaves_of(&split);
    assert!(is_partition(&leaves));
    // gc's children are level 3; across the cube edge sits a level-0 root: jump 3.
    assert!(max_level_jump(&leaves) >= 2);
    assert!(!is_balanced(&leaves));
}

#[test]
fn reference_views_balanced() {
    use planetary_core::lod::{LodParams, View, reference_views, select};
    let m = model();
    let p = LodParams::default();
    let views = reference_views(R, m.max_relief);
    assert_eq!(views.len(), 8 + 8 + 12 + 2);
    for (name, eye) in views {
        let v = View::new(eye, 720, 1.0 / 3.0f64.sqrt()).unwrap();
        let sel = select(&m, &p, &v, &BTreeSet::new());
        assert!(is_partition(&sel.leaves), "{name}");
        assert!(max_level_jump(&sel.leaves) <= 1, "{name}");
        assert!(
            sel.leaves.len() <= p.max_leaves || sel.stats.tau_steps == p.max_tau_steps,
            "{name}"
        );
    }
}
