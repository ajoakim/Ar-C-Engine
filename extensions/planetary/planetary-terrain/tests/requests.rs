//! Priority-ordered request planning (07.1-19): score order, deterministic tie-break,
//! ancestors-first, caps, and the screen-space-error score itself.

use std::collections::BTreeSet;

use planetary_core::lod::{DEFAULT_KAPPA, HeightRange, LodModel, View, patch_footprint};
use planetary_core::{CubeSphere, Face, PatchId, PlanetTerrain, TerrainParams, Topology};
use planetary_terrain::{TAN_HALF_FOV_Y, plan_requests, plan_requests_scored, request_score};

fn topo() -> CubeSphere {
    CubeSphere::new(8)
}

fn id(face: Face, level: u8, x: u32, y: u32) -> PatchId {
    PatchId::new(face, level, x, y).expect("patch id")
}

/// Everything above `level` resident, `level` and below not.
fn resident_above(level: u8) -> impl Fn(PatchId) -> bool {
    move |p: PatchId| p.level() < level
}

#[test]
fn higher_score_first_then_level_raw_tie_break() {
    let a = id(Face::ALL[0], 2, 0, 0);
    let b = id(Face::ALL[1], 2, 1, 1);
    let c = id(Face::ALL[2], 2, 2, 2);
    let d = id(Face::ALL[3], 2, 3, 3);
    let res = resident_above(2);
    let none = |_: PatchId| false;
    let out = plan_requests_scored(
        &topo(),
        &[(a, 1.0), (b, 5.0), (c, 5.0), (d, 3.0)],
        &res,
        &none,
        usize::MAX,
    );
    let mut tied = [b, c];
    tied.sort_by_key(|p| (p.level(), p.raw()));
    assert_eq!(out, vec![tied[0], tied[1], d, a]);
    // Identical inputs give identical output, whatever the input order of equal scores.
    let again = plan_requests_scored(
        &topo(),
        &[(d, 3.0), (c, 5.0), (b, 5.0), (a, 1.0)],
        &res,
        &none,
        usize::MAX,
    );
    assert_eq!(out, again);
}

#[test]
fn ancestor_inherits_the_best_score_of_the_leaves_it_serves() {
    let t = topo();
    let urgent_leaf = id(Face::ALL[0], 4, 5, 6);
    let other_leaf = id(Face::ALL[1], 4, 2, 2);
    // Only the roots are missing: both chains first need their level-0 root.
    let res = |_: PatchId| false;
    let none = |_: PatchId| false;
    let out = plan_requests_scored(
        &t,
        &[(other_leaf, 2.0), (urgent_leaf, 9.0)],
        &res,
        &none,
        usize::MAX,
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], urgent_leaf.ancestor_at(0).expect("root"));
    assert_eq!(out[1], other_leaf.ancestor_at(0).expect("root"));
}

#[test]
fn a_child_is_never_planned_before_its_parent_is_resident() {
    let t = topo();
    let leaves: Vec<(PatchId, f64)> = PatchId::level_iter(3)
        .take(40)
        .enumerate()
        .map(|(i, p)| (p, 1000.0 - i as f64))
        .collect();
    // Nothing resident: only roots may be requested, whatever the scores.
    let out = plan_requests_scored(&t, &leaves, &|_| false, &|_| false, usize::MAX);
    assert!(!out.is_empty());
    assert!(out.iter().all(|p| p.level() == 0));
    // Parents resident down to level 2: level-3 leaves are planned, never level-4 or deeper.
    let res = resident_above(3);
    let out = plan_requests_scored(&t, &leaves, &res, &|_| false, usize::MAX);
    assert!(
        out.iter()
            .all(|p| p.level() == 3 && res(p.parent().unwrap()))
    );
}

#[test]
fn cap_takes_the_highest_scores_and_is_a_prefix() {
    let t = topo();
    let leaves: Vec<(PatchId, f64)> = PatchId::level_iter(3)
        .take(30)
        .enumerate()
        .map(|(i, p)| (p, (i % 7) as f64 + (i as f64) * 1e-3))
        .collect();
    let res = resident_above(3);
    let all = plan_requests_scored(&t, &leaves, &res, &|_| false, usize::MAX);
    let capped = plan_requests_scored(&t, &leaves, &res, &|_| false, 5);
    assert_eq!(capped.len(), 5);
    assert_eq!(capped[..], all[..5]);
    let uniq: BTreeSet<_> = all.iter().collect();
    assert_eq!(uniq.len(), all.len(), "duplicates");
    assert!(plan_requests_scored(&t, &leaves, &res, &|_| false, 0).is_empty());
}

#[test]
fn pending_and_resident_cells_are_skipped_and_non_finite_scores_count_as_zero() {
    let t = topo();
    let a = id(Face::ALL[0], 2, 0, 0);
    let b = id(Face::ALL[1], 2, 1, 1);
    let c = id(Face::ALL[2], 2, 2, 2);
    let res = resident_above(2);
    let pend = move |p: PatchId| p == a;
    let out = plan_requests_scored(
        &t,
        &[(a, 100.0), (b, f64::NAN), (c, 0.5)],
        &res,
        &pend,
        usize::MAX,
    );
    assert_eq!(out, vec![c, b]);
}

#[test]
fn unscored_wrapper_orders_by_level_then_raw() {
    let t = topo();
    let leaves = vec![
        id(Face::ALL[3], 3, 1, 1),
        id(Face::ALL[0], 3, 0, 0),
        id(Face::ALL[2], 3, 4, 4),
    ];
    let out = plan_requests(&t, &leaves, &resident_above(3), &|_| false, 10);
    let mut sorted = leaves.clone();
    sorted.sort_by_key(|p| (p.level(), p.raw()));
    assert_eq!(out, sorted);
}

fn model() -> LodModel {
    let terrain = PlanetTerrain::new(1, TerrainParams::default(), 8).expect("terrain");
    LodModel::from_terrain(&terrain, DEFAULT_KAPPA)
}

#[test]
fn score_is_higher_for_nearer_cells_and_for_larger_geometric_error() {
    let m = model();
    let r = m.radius;
    let range = HeightRange { min: 0.0, max: 0.0 };
    let leaf = id(Face::ALL[0], 8, 100, 100);
    let fp = patch_footprint(leaf);
    let dir = planetary_core::patch_center_dir(leaf);
    let near = View::new(dir * (r + 2.0e3), 720, TAN_HALF_FOV_Y).expect("view");
    let far = View::new(dir * (r + 2.0e6), 720, TAN_HALF_FOV_Y).expect("view");
    let near_score = request_score(&m, &near, &fp, range, leaf.level());
    let far_score = request_score(&m, &far, &fp, range, leaf.level());
    assert!(near_score > far_score, "{near_score} vs {far_score}");
    // Same footprint and distance, coarser level: larger geometric error, larger score.
    let coarse = request_score(&m, &near, &fp, range, leaf.level() - 2);
    assert!(coarse > near_score, "{coarse} vs {near_score}");
    // Pure.
    assert_eq!(
        near_score.to_bits(),
        request_score(&m, &near, &fp, range, leaf.level()).to_bits()
    );
    // The distance is clamped at 1 m: an eye on the surface gives a finite score.
    let on = View::new(dir * r, 720, TAN_HALF_FOV_Y).expect("view");
    assert!(request_score(&m, &on, &fp, range, leaf.level()).is_finite());
}

#[test]
fn nearest_cell_to_the_eye_is_requested_first() {
    let t = topo();
    let m = model();
    let range = m.height;
    let level = 6;
    let near = id(Face::ALL[0], level, 20, 20);
    let far = id(Face::ALL[3], level, 20, 20);
    let eye = planetary_core::patch_center_dir(near) * (m.radius + 5.0e3);
    let view = View::new(eye, 720, TAN_HALF_FOV_Y).expect("view");
    let scored: Vec<(PatchId, f64)> = [far, near]
        .iter()
        .map(|&p| {
            (
                p,
                request_score(&m, &view, &t.footprint(p), range, p.level()),
            )
        })
        .collect();
    let out = plan_requests_scored(&t, &scored, &resident_above(level), &|_| false, 10);
    assert_eq!(out, vec![near, far]);
}

#[test]
fn roots_are_planned_before_any_higher_scored_deeper_cell() {
    let t = topo();
    let root = id(Face::ALL[5], 0, 0, 0);
    let deep = id(Face::ALL[0], 3, 1, 1);
    // Roots of the deep chain's face are resident; the other face's root is not.
    let res = move |p: PatchId| p.face() == Face::ALL[0] && p.level() < 3;
    let out = plan_requests_scored(&t, &[(deep, 1.0e6), (root, 0.001)], &res, &|_| false, 10);
    assert_eq!(out, vec![root, deep]);
}

/// The root-first chain scan `plan_requests_scored` used before the shared-ancestor memo.
fn plan_reference<T: Topology>(
    topo: &T,
    wanted: &[(T::Cell, f64)],
    is_resident: &dyn Fn(T::Cell) -> bool,
    is_pending: &dyn Fn(T::Cell) -> bool,
    max: usize,
) -> Vec<T::Cell> {
    let mut best: std::collections::BTreeMap<(u8, u64), f64> = std::collections::BTreeMap::new();
    let mut chain: Vec<T::Cell> = Vec::new();
    for &(piece, score) in wanted {
        let score = if score.is_finite() { score } else { 0.0 };
        chain.clear();
        let mut cur = Some(piece);
        while let Some(c) = cur {
            chain.push(c);
            cur = topo.primary_parent(c);
        }
        for &a in chain.iter().rev() {
            if is_resident(a) {
                continue;
            }
            if !is_pending(a) {
                let e = best.entry((topo.level(a), topo.raw(a))).or_insert(score);
                if score > *e {
                    *e = score;
                }
            }
            break;
        }
    }
    let mut order: Vec<((u8, u64), f64)> = best.into_iter().collect();
    order.sort_by(|a, b| {
        (a.0.0 != 0)
            .cmp(&(b.0.0 != 0))
            .then_with(|| b.1.total_cmp(&a.1))
    });
    order
        .into_iter()
        .take(max)
        .filter_map(|((_, raw), _)| topo.from_raw(raw))
        .collect()
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn planning_equals_reference<T: Topology>(topo: &T, seed: u64) {
    let mut st = seed;
    // A pool of cells over levels 0..=7 reached by descending from the roots.
    let mut pool: Vec<T::Cell> = Vec::new();
    let mut frontier: Vec<T::Cell> = topo.roots().to_vec();
    for _ in 0..7 {
        pool.extend(frontier.iter().copied());
        let mut next = Vec::new();
        let mut kids = Vec::new();
        for &c in &frontier {
            kids.clear();
            topo.children(c, &mut kids);
            if let Some(k) = kids.get(usize::try_from(splitmix(&mut st) % 3).expect("i")) {
                next.push(*k);
            }
            next.extend(kids.iter().copied().take(2));
        }
        next.sort();
        next.dedup();
        frontier = next;
    }
    pool.extend(frontier.iter().copied());
    for round in 0..80 {
        let resident_pct = splitmix(&mut st) % 100;
        let pending_pct = splitmix(&mut st) % 40;
        let mut resident = BTreeSet::new();
        let mut pending = BTreeSet::new();
        for &c in &pool {
            if splitmix(&mut st) % 100 < resident_pct {
                resident.insert(c);
            }
            if splitmix(&mut st) % 100 < pending_pct {
                pending.insert(c);
            }
        }
        let mut wanted = Vec::new();
        for _ in 0..(1 + splitmix(&mut st) % 120) {
            let c = pool[usize::try_from(splitmix(&mut st) % pool.len() as u64).expect("i")];
            let score = match splitmix(&mut st) % 9 {
                0 => f64::NAN,
                1 => f64::INFINITY,
                _ => (splitmix(&mut st) % 50) as f64 * 0.5,
            };
            wanted.push((c, score));
        }
        let max = usize::try_from(1 + splitmix(&mut st) % 40).expect("max");
        let res = |c: T::Cell| resident.contains(&c);
        let pen = |c: T::Cell| pending.contains(&c);
        assert_eq!(
            plan_requests_scored(topo, &wanted, &res, &pen, max),
            plan_reference(topo, &wanted, &res, &pen, max),
            "round {round}"
        );
    }
}

#[test]
fn shared_ancestor_planning_equals_the_chain_scan_hex_and_cube() {
    planning_equals_reference(&CubeSphere::new(8), 1);
    planning_equals_reference(&planetary_core::HexSphere::new(8).expect("hex"), 2);
}
