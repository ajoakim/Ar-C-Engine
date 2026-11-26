//! Effective drawn cut under partial residency, negative control and request planning.

use std::collections::BTreeSet;

use engine_sdk::procedural::splitmix64;
use planetary_core::lod::{
    DEFAULT_KAPPA, LodModel, LodParams, View, balance, is_partition, leaves_of, max_level_jump,
    reference_views, select,
};
use planetary_core::{Face, MAX_LEVEL, PatchId, PlanetTerrain, TerrainParams};
use planetary_terrain::{effective_cut, naive_cut};

const SEED: u64 = 0x0007_5EED_2026_0001;

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
            select(&model, &lod, &view, &BTreeSet::new()).split
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

fn check_cut(wanted: &BTreeSet<PatchId>, resident: &dyn Fn(PatchId) -> bool) {
    let cut = effective_cut(wanted, resident);
    assert!(is_partition(&cut.leaves), "not a partition");
    assert!(max_level_jump(&cut.leaves) <= 1, "unbalanced");
    let closure = closure_of(wanted);
    for l in &cut.leaves {
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
            planetary_terrain::EffectiveCut {
                split: wanted.clone(),
                leaves: leaves_of(&wanted),
                coarsened: 0
            }
        );
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
    assert!(max_level_jump(&naive.leaves) >= 2, "negative control");
    let eff = effective_cut(&wanted, &res);
    assert!(max_level_jump(&eff.leaves) <= 1);
    assert!(eff.coarsened > 0);
    assert!(is_partition(&eff.leaves));
    assert!(eff.leaves.iter().all(|l| res(*l)));
}

#[test]
fn nothing_resident_draws_nothing() {
    let wanted = wanted_sets().remove(0);
    let cut = effective_cut(&wanted, &|_| false);
    assert!(cut.leaves.is_empty());
    assert!(cut.split.is_empty());
}

#[test]
fn missing_root_leaves_five_faces() {
    let missing = PatchId::root(Face::NegZ);
    for wanted in wanted_sets() {
        let cut = effective_cut(&wanted, &|p| p != missing && p.face() != Face::NegZ);
        let area: u128 = cut
            .leaves
            .iter()
            .map(|p| 1u128 << (2 * u32::from(MAX_LEVEL - p.level())))
            .sum();
        assert_eq!(area, 5u128 << (2 * u32::from(MAX_LEVEL)));
        assert!(cut.leaves.iter().all(|p| p.face() != Face::NegZ));
        assert!(max_level_jump(&cut.leaves) <= 1);
    }
}
