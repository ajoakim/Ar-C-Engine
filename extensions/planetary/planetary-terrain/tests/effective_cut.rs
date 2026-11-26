//! Effective drawn cut under partial residency, negative control and request planning.
//!
//! Everything runs through the topology-generic API with `CubeSphere`; the Phase 7 cube-only cut
//! is kept below as an oracle and must agree exactly.

use std::collections::BTreeSet;

use engine_sdk::procedural::splitmix64;
use planetary_core::lod::{
    DEFAULT_KAPPA, LodModel, LodParams, View, balance, is_partition, leaves_of, max_level_jump,
    reference_views,
};
use planetary_core::topology::select::select_cells;
use planetary_core::{
    CubeSphere, Face, MAX_LEVEL, PatchId, Piece, PlanetTerrain, TerrainParams, Topology,
};

const SEED: u64 = 0x0007_5EED_2026_0001;

type Cut = planetary_terrain::EffectiveCut<PatchId>;

fn topo() -> CubeSphere {
    CubeSphere::new(8)
}

fn effective_cut(w: &BTreeSet<PatchId>, r: &dyn Fn(PatchId) -> bool) -> Cut {
    planetary_terrain::effective_cut(&topo(), w, r)
}

fn naive_cut(w: &BTreeSet<PatchId>, r: &dyn Fn(PatchId) -> bool) -> Cut {
    planetary_terrain::naive_cut(&topo(), w, r)
}

fn leaves(cut: &Cut) -> Vec<PatchId> {
    cut.pieces.iter().map(|p| p.cell).collect()
}

fn topo_pieces(w: &BTreeSet<PatchId>) -> Vec<Piece<PatchId>> {
    let mut out = Vec::new();
    topo().pieces(w, &mut out);
    out
}

mod oracle {
    //! The Phase 7 cube-only cut, kept verbatim as the parity oracle.
    use std::collections::{BTreeMap, BTreeSet};

    use planetary_core::{Edge, Face, PatchId, neighbor};

    pub struct Old {
        pub split: BTreeSet<PatchId>,
        pub leaves: Vec<PatchId>,
        pub coarsened: u32,
    }

    fn build_split(w: &BTreeSet<PatchId>, res: &dyn Fn(PatchId) -> bool) -> BTreeSet<PatchId> {
        let mut split = BTreeSet::new();
        let mut stack: Vec<PatchId> = Face::ALL
            .iter()
            .rev()
            .map(|f| PatchId::root(*f))
            .filter(|r| res(*r))
            .collect();
        while let Some(p) = stack.pop() {
            if !w.contains(&p) {
                continue;
            }
            let Some(ch) = p.children() else { continue };
            if ch.iter().all(|c| res(*c)) {
                split.insert(p);
                stack.extend(ch);
            }
        }
        split
    }

    fn drawn_leaves(split: &BTreeSet<PatchId>, res: &dyn Fn(PatchId) -> bool) -> Vec<PatchId> {
        let mut out = Vec::new();
        for f in Face::ALL {
            let r = PatchId::root(f);
            if !split.contains(&r) && res(r) {
                out.push(r);
            }
        }
        for s in split {
            if let Some(ch) = s.children() {
                for c in ch {
                    if !split.contains(&c) {
                        out.push(c);
                    }
                }
            }
        }
        out.sort();
        out
    }

    fn violating_parents(leaves: &[PatchId]) -> BTreeSet<PatchId> {
        let level_of: BTreeMap<PatchId, u8> = leaves.iter().map(|l| (*l, l.level())).collect();
        let mut parents = BTreeSet::new();
        for &p in leaves {
            let level = p.level();
            if level < 2 {
                continue;
            }
            for e in Edge::ALL {
                let mut cur = Some(neighbor(p, e).patch);
                while let Some(q) = cur {
                    if let Some(&ql) = level_of.get(&q) {
                        if ql + 2 <= level
                            && let Some(par) = p.parent()
                        {
                            parents.insert(par);
                        }
                        break;
                    }
                    cur = q.parent();
                }
            }
        }
        parents
    }

    pub fn old_cut(w: &BTreeSet<PatchId>, res: &dyn Fn(PatchId) -> bool) -> Old {
        let mut split = build_split(w, res);
        let mut coarsened = 0u32;
        loop {
            let leaves = drawn_leaves(&split, res);
            let bad = violating_parents(&leaves);
            if bad.is_empty() {
                return Old {
                    split,
                    leaves,
                    coarsened,
                };
            }
            let before = split.len();
            split.retain(|s| !bad.iter().any(|b| s == b || b.is_ancestor_of(*s)));
            coarsened += u32::try_from(before - split.len()).unwrap_or(u32::MAX);
        }
    }
}

fn assert_oracle(wanted: &BTreeSet<PatchId>, res: &dyn Fn(PatchId) -> bool) {
    let new = effective_cut(wanted, res);
    let old = oracle::old_cut(wanted, res);
    assert_eq!(new.split, old.split, "split differs from the Phase 7 cut");
    assert_eq!(
        leaves(&new),
        old.leaves,
        "leaves differ from the Phase 7 cut"
    );
    assert_eq!(new.coarsened, old.coarsened, "coarsened differs");
    assert!(new.pieces.iter().all(|p| p.variant == 0));
}

/// Wanted split sets for every reference view.
fn wanted_sets() -> Vec<BTreeSet<PatchId>> {
    let params = TerrainParams::default();
    let radius = params.radius;
    let relief = params.max_relief();
    let terrain = PlanetTerrain::new(SEED, params, 8).expect("terrain");
    let model = LodModel::from_terrain(&terrain, DEFAULT_KAPPA);
    let lod = LodParams {
        max_level: 14,
        ..Default::default()
    };
    reference_views(radius, relief)
        .into_iter()
        .map(|(_, eye)| {
            let view = View::new(eye, 720, planetary_terrain::TAN_HALF_FOV_Y).expect("view");
            select_cells(
                &CubeSphere::new(model.cells_per_edge),
                &model,
                &lod,
                &view,
                &BTreeSet::new(),
                &|_| None,
            )
            .split
        })
        .collect()
}

fn closure_of(split: &BTreeSet<PatchId>) -> BTreeSet<PatchId> {
    let mut out = BTreeSet::new();
    for leaf in leaves_of(split) {
        let mut cur = Some(leaf);
        while let Some(c) = cur {
            out.insert(c);
            cur = c.parent();
        }
    }
    out
}

/// With all six roots resident, every split cell has every required split split.
fn assert_balanced(cut: &Cut) {
    let t = topo();
    let mut req = Vec::new();
    for s in &cut.split {
        req.clear();
        t.required_splits(*s, &mut req);
        assert!(
            req.iter().all(|r| cut.split.contains(r)),
            "required split missing for {s:?}"
        );
    }
}

fn check_cut(wanted: &BTreeSet<PatchId>, resident: &dyn Fn(PatchId) -> bool) {
    let cut = effective_cut(wanted, resident);
    assert_oracle(wanted, resident);
    let cl = leaves(&cut);
    assert!(is_partition(&cl), "not a partition");
    assert!(max_level_jump(&cl) <= 1, "unbalanced");
    let closure = closure_of(wanted);
    for l in &cl {
        assert!(resident(*l), "non-resident leaf {l:?}");
        assert!(closure.contains(l), "leaf finer than wanted {l:?}");
    }
    assert_eq!(cut, effective_cut(wanted, resident), "deterministic");
}

#[test]
fn all_resident_equals_wanted() {
    for wanted in wanted_sets() {
        let cut = effective_cut(&wanted, &|_| true);
        assert_eq!(
            cut,
            Cut {
                split: wanted.clone(),
                pieces: topo_pieces(&wanted),
                coarsened: 0
            }
        );
        assert_eq!(leaves(&cut), leaves_of(&wanted));
        assert_oracle(&wanted, &|_| true);
    }
}

#[test]
fn levels_up_to_k_resident_is_partition_balanced_resident() {
    let sets = wanted_sets();
    for k in 0..=8u8 {
        for wanted in &sets {
            check_cut(wanted, &|p| p.level() <= k);
        }
    }
}

fn closed_random(seed: u64) -> impl Fn(PatchId) -> bool {
    fn res(seed: u64, p: PatchId) -> bool {
        match p.parent() {
            None => true,
            Some(par) => res(seed, par) && splitmix64(seed ^ p.raw()) % 10 < 7,
        }
    }
    move |p| res(seed, p)
}

#[test]
fn seeded_random_closed_residency() {
    let sets = wanted_sets();
    for seed in 0..100u64 {
        let r = closed_random(seed);
        for wanted in &sets {
            check_cut(wanted, &r);
        }
    }
}

#[test]
fn non_closed_random_residency() {
    let sets = wanted_sets();
    for seed in 0..100u64 {
        let r = move |p: PatchId| p.parent().is_none() || splitmix64(seed ^ p.raw()) % 10 < 6;
        for wanted in &sets {
            check_cut(wanted, &r);
        }
    }
}

#[test]
fn generic_cut_is_balanced() {
    let sets = wanted_sets();
    for seed in 0..20u64 {
        let r = closed_random(seed);
        for wanted in &sets {
            assert_balanced(&effective_cut(wanted, &r));
        }
    }
}

#[test]
fn naive_cut_breaks_balance_effective_cut_fixes_it() {
    // Deep chain toward the cube corner (max, max) of PosX, balanced across faces.
    let depth = 10u8;
    let mut chain = BTreeSet::new();
    let mut resident: BTreeSet<PatchId> = PatchId::level_iter(0)
        .chain(PatchId::level_iter(1))
        .collect();
    for l in 0..depth {
        let k = (1u32 << l) - 1;
        let p = PatchId::new(Face::PosX, l, k, k).unwrap();
        chain.insert(p);
        resident.extend(p.children().unwrap());
    }
    let mut wanted = chain.clone();
    balance(&mut wanted);
    let res = |p: PatchId| resident.contains(&p);

    let naive = naive_cut(&wanted, &res);
    assert!(max_level_jump(&leaves(&naive)) >= 2, "negative control");
    let eff = effective_cut(&wanted, &res);
    assert_oracle(&wanted, &res);
    let el = leaves(&eff);
    assert!(max_level_jump(&el) <= 1);
    assert!(eff.coarsened > 0);
    assert!(is_partition(&el));
    assert!(el.iter().all(|l| res(*l)));
}

#[test]
fn nothing_resident_draws_nothing() {
    let wanted = wanted_sets().remove(0);
    let cut = effective_cut(&wanted, &|_| false);
    assert!(cut.pieces.is_empty());
    assert!(cut.split.is_empty());
}

#[test]
fn missing_root_leaves_five_faces() {
    let missing = PatchId::root(Face::NegZ);
    for wanted in wanted_sets() {
        let res = |p: PatchId| p != missing && p.face() != Face::NegZ;
        let cut = effective_cut(&wanted, &res);
        assert_oracle(&wanted, &res);
        let cl = leaves(&cut);
        let area: u128 = cl
            .iter()
            .map(|p| 1u128 << (2 * u32::from(MAX_LEVEL - p.level())))
            .sum();
        assert_eq!(area, 5u128 << (2 * u32::from(MAX_LEVEL)));
        assert!(cl.iter().all(|p| p.face() != Face::NegZ));
        assert!(max_level_jump(&cl) <= 1);
    }
}

#[test]
fn nothing_resident_requests_the_six_roots() {
    let wanted = wanted_sets().remove(0);
    let leaves = leaves_of(&wanted);
    let reqs = planetary_terrain::plan_requests(&topo(), &leaves, &|_| false, &|_| false, 100);
    let mut roots: Vec<PatchId> = Face::ALL.iter().map(|f| PatchId::root(*f)).collect();
    roots.sort();
    assert_eq!(reqs, roots);
}

#[test]
fn requests_are_ancestors_first_unique_sorted_capped() {
    let sets = wanted_sets();
    let t = topo();
    for seed in 0..20u64 {
        let res = closed_random(seed);
        let pending = move |p: PatchId| splitmix64(seed ^ p.raw() ^ 0xABCD).is_multiple_of(5);
        for wanted in &sets {
            let leaves = leaves_of(wanted);
            let all = planetary_terrain::plan_requests(&t, &leaves, &res, &pending, usize::MAX);
            let capped = planetary_terrain::plan_requests(&t, &leaves, &res, &pending, 7);
            assert!(capped.len() <= 7);
            assert_eq!(capped[..], all[..capped.len()]);
            let uniq: BTreeSet<_> = all.iter().collect();
            assert_eq!(uniq.len(), all.len());
            assert!(
                all.windows(2)
                    .all(|w| (w[0].level(), w[0].raw()) < (w[1].level(), w[1].raw()))
            );
            for p in &all {
                assert!(!res(*p) && !pending(*p));
                assert!(p.parent().is_none_or(&res));
            }
        }
    }
}

#[test]
fn closure_and_stale_requests() {
    let wanted = wanted_sets().remove(5);
    let leaves = leaves_of(&wanted);
    let closure = planetary_terrain::wanted_closure(&topo(), &leaves);
    for l in &leaves {
        let mut cur = Some(*l);
        while let Some(c) = cur {
            assert!(closure.contains(&c));
            cur = c.parent();
        }
    }
    let outside = PatchId::new(Face::NegZ, MAX_LEVEL, 3, 5).unwrap();
    assert!(!closure.contains(&outside));
    let inside = leaves[0];
    let requested: BTreeSet<PatchId> = [outside, inside].into_iter().collect();
    assert_eq!(
        planetary_terrain::stale_requests(&requested, &closure),
        vec![outside]
    );
}

#[test]
fn priority_by_level() {
    use engine_sdk::Priority;
    assert_eq!(planetary_terrain::priority_for_level(0), Priority::High);
    assert_eq!(planetary_terrain::priority_for_level(3), Priority::High);
    assert_eq!(planetary_terrain::priority_for_level(4), Priority::Normal);
}
