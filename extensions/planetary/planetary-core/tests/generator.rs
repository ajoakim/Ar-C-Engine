//! Planet-space generator tests: purity, stack == pure function, borders, goldens.

use engine_sdk::DVec3;
use engine_sdk::procedural::{Fnv1a64, Generator, Params, StableId, canonical_bits, layer_seed};
use planetary_core::{
    BASE_LAYER, BaseLayer, DETAIL_LAYER, Edge, Face, MASK_LAYER, PatchId, PlanetTerrain,
    RIDGE_LAYER, TerrainError, TerrainParams, base_at, compose, compose_v2, detail_at, locate,
    mask_at, neighbor, ridge_at, vertex_dir,
};

const WORLD_SEED: u64 = 0x0007_5EED_2026_0001;
const N: u32 = 8;

// Windows x86_64; Linux/macOS pending hosted CI (ADR-0008). Never loosen; fix the drifting op.
const GOLDEN_HEIGHTS_V1: u64 = 0xf067_ef3f_b83f_bd54;
const GOLDEN_DIRS_V1: u64 = 0x3697_d55a_c59d_0aa2;
// Generator v2 (`TerrainParams::demo_v2()`), recorded with
// `cargo test -p planetary-core --test generator golden -- --nocapture` (printed by the test).
const GOLDEN_HEIGHTS_V2: u64 = 0x3096_1733_51c3_59fb;

fn terrain() -> PlanetTerrain {
    PlanetTerrain::new(WORLD_SEED, TerrainParams::default(), N).unwrap()
}

fn terrain_v2() -> PlanetTerrain {
    PlanetTerrain::new(WORLD_SEED, TerrainParams::demo_v2(), N).unwrap()
}

fn patches() -> [PatchId; 4] {
    [
        PatchId::root(Face::PosX),
        PatchId::new(Face::PosY, 3, 7, 3).unwrap(),
        PatchId::new(Face::PosZ, 2, 0, 0).unwrap(),
        PatchId::new(Face::NegX, 19, 123_456, 201_234).unwrap(),
    ]
}

#[test]
fn stack_equals_pure_function() {
    let t = terrain();
    let seeds = t.seeds();
    let p = *t.params();
    for id in patches() {
        let out = t.stack().generate(StableId(id.raw()), &Params::new());
        let base = out.layer(BASE_LAYER).unwrap();
        let detail = out.layer(DETAIL_LAYER).unwrap();
        let mask = out.layer(MASK_LAYER).unwrap();
        let samples = t.patch_samples(id);
        assert_eq!(samples.len(), ((N + 1) * (N + 1)) as usize);
        for j in 0..=N {
            for i in 0..=N {
                let k = (j * (N + 1) + i) as usize;
                let d = vertex_dir(id, N, i, j);
                assert_eq!(base[k].to_bits(), base_at(seeds.base, &p, d).to_bits());
                assert_eq!(
                    detail[k].to_bits(),
                    detail_at(seeds.detail, &p, d).to_bits()
                );
                assert_eq!(mask[k].to_bits(), mask_at(seeds.mask, &p, d).to_bits());
                assert_eq!(samples[k].height.to_bits(), t.height_at_dir(d).to_bits());
            }
        }
    }
}

#[test]
fn stack_equals_pure_function_v2() {
    let t = terrain_v2();
    let seeds = t.seeds();
    let p = *t.params();
    assert_eq!(seeds.ridge, layer_seed(WORLD_SEED, "planet.ridge", 2));
    for id in patches() {
        let out = t.stack().generate(StableId(id.raw()), &Params::new());
        let ridge = out.layer(RIDGE_LAYER).unwrap();
        let samples = t.patch_samples(id);
        assert_eq!(samples.len(), ((N + 1) * (N + 1)) as usize);
        for j in 0..=N {
            for i in 0..=N {
                let k = (j * (N + 1) + i) as usize;
                let d = vertex_dir(id, N, i, j);
                assert_eq!(ridge[k].to_bits(), ridge_at(seeds.ridge, &p, d).to_bits());
                // The hex path samples pointwise; the cube path goes through the stack.
                assert_eq!(samples[k].height.to_bits(), t.height_at_dir(d).to_bits());
                assert_eq!(samples[k].height.to_bits(), t.sample(d).height.to_bits());
            }
        }
    }
}

#[test]
fn v1_has_no_ridge_layer_and_seed_zero() {
    let t = terrain();
    assert_eq!(t.seeds().ridge, 0);
    let out = t
        .stack()
        .generate(StableId(patches()[0].raw()), &Params::new());
    assert!(out.layer(RIDGE_LAYER).is_none());
}

#[test]
fn v2_is_deterministic_and_seed_dependent() {
    let a = terrain_v2();
    let b = terrain_v2();
    let other = PlanetTerrain::new(WORLD_SEED + 1, TerrainParams::demo_v2(), N).unwrap();
    let mut differs = false;
    for id in patches() {
        assert_eq!(a.patch_samples(id), b.patch_samples(id));
        differs |= a.patch_samples(id) != other.patch_samples(id);
    }
    assert!(differs);
}

#[test]
fn v2_differs_from_v1_only_through_the_ridge_term() {
    let v1 = terrain();
    let v2 = terrain_v2();
    let p = *v2.params();
    let seeds = v2.seeds();
    let mut changed = 0;
    for id in patches() {
        for j in 0..=N {
            for i in 0..=N {
                let d = vertex_dir(id, N, i, j);
                let (a, b) = (v1.sample(d), v2.sample(d));
                // Same mask and layer seeds; only the ridge term moves the height.
                assert_eq!(a.mask.to_bits(), b.mask.to_bits());
                if a.height.to_bits() != b.height.to_bits() {
                    changed += 1;
                }
                // With the ridge term zero the v2 composition equals v1 bit for bit.
                let base = base_at(seeds.base, &p, d);
                let detail = detail_at(seeds.detail, &p, d);
                let mask = mask_at(seeds.mask, &p, d);
                assert_eq!(
                    compose_v2(&p, base, detail, mask, 0.0).to_bits(),
                    compose(&p, base, detail, mask).to_bits()
                );
            }
        }
    }
    assert!(changed > 0, "the ridge layer must change some heights");
}

#[test]
fn seeds_derive_from_layer_seed() {
    let t = terrain();
    assert_eq!(t.seeds().base, layer_seed(WORLD_SEED, "planet.base", 1));
    assert_eq!(t.seeds().detail, layer_seed(WORLD_SEED, "planet.detail", 1));
    assert_eq!(t.seeds().mask, layer_seed(WORLD_SEED, "planet.mask", 1));
    let other = PlanetTerrain::new(WORLD_SEED + 1, TerrainParams::default(), N).unwrap();
    let d = DVec3::new(0.3, 0.5, 0.2).normalize();
    assert_ne!(
        t.height_at_dir(d).to_bits(),
        other.height_at_dir(d).to_bits()
    );
}

#[test]
fn generation_is_pure_and_order_independent() {
    let t = terrain();
    let ids = patches();
    let fwd: Vec<_> = ids.iter().map(|&id| t.patch_samples(id)).collect();
    let mut rev: Vec<_> = ids.iter().rev().map(|&id| t.patch_samples(id)).collect();
    rev.reverse();
    assert_eq!(fwd, rev);
    assert_eq!(fwd[1], t.patch_samples(ids[1]));
}

#[test]
fn shared_border_heights_bit_equal() {
    let t = terrain();
    for level in 0..=3u8 {
        for p in PatchId::level_iter(level) {
            for e in Edge::ALL {
                let nb = neighbor(p, e);
                for s in 0..=N {
                    let (i, j) = Edge::vertex(e, N, s);
                    let s2 = if nb.flipped { N - s } else { s };
                    let (i2, j2) = Edge::vertex(nb.edge, N, s2);
                    let a = t.height_at_dir(vertex_dir(p, N, i, j));
                    let b = t.height_at_dir(vertex_dir(nb.patch, N, i2, j2));
                    assert_eq!(a.to_bits(), b.to_bits());
                }
            }
        }
    }
}

#[test]
fn heights_and_mask_in_range() {
    let t = terrain();
    let p = *t.params();
    let mut state = 0xABCD_EF01_2345_6789u64;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 11) as f64) / 9_007_199_254_740_992.0 * 2.0 - 1.0
    };
    for _ in 0..10_000 {
        let v = DVec3::new(next(), next(), next());
        if v.length() < 1e-3 {
            continue;
        }
        let d = v.normalize();
        let s = t.sample(d);
        assert!(s.height >= p.sea_level && s.height <= p.max_relief());
        assert!((0.0..1.0).contains(&s.mask));
        // locate must resolve every sampled direction to a valid patch.
        let _ = locate(d, 6);
    }
}

#[test]
fn invalid_id_yields_zeros() {
    assert!(PatchId::from_raw(u64::MAX).is_err());
    let p = TerrainParams::default();
    let layer = BaseLayer {
        params: p,
        cells_per_edge: N,
    };
    let mut out = vec![1.0; layer.output_len()];
    layer.generate(StableId(u64::MAX), 7, &Params::new(), &mut out);
    assert!(out.iter().all(|v| *v == 0.0));
}

#[test]
fn validation_rejects_bad_input() {
    let d = TerrainParams::default();
    let bad = [
        TerrainParams {
            base_amplitude: f64::NAN,
            ..d
        },
        TerrainParams { radius: 0.0, ..d },
        TerrainParams { radius: -1.0, ..d },
        TerrainParams {
            base_octaves: 0,
            ..d
        },
        TerrainParams {
            detail_octaves: 25,
            ..d
        },
        TerrainParams {
            detail_frequency: 1e12,
            ..d
        },
    ];
    for p in bad {
        assert!(
            matches!(
                PlanetTerrain::new(1, p, N),
                Err(TerrainError::InvalidParams(_))
            ),
            "{p:?}"
        );
    }
    for n in [0u32, 3, 128] {
        assert!(matches!(
            PlanetTerrain::new(1, d, n),
            Err(TerrainError::CellsPerEdge(_))
        ));
    }
    assert!(d.validate().is_ok());
}

#[test]
fn validation_covers_generator_version_and_ridge_fields() {
    let v2 = TerrainParams::demo_v2();
    assert!(v2.validate().is_ok());
    let bad = [
        TerrainParams {
            generator_version: 0,
            ..v2
        },
        TerrainParams {
            generator_version: 3,
            ..v2
        },
        TerrainParams {
            ridge_amplitude: 0.0,
            ..v2
        },
        TerrainParams {
            ridge_amplitude: -1.0,
            ..v2
        },
        TerrainParams {
            ridge_amplitude: f64::NAN,
            ..v2
        },
        TerrainParams {
            ridge_frequency: 0.0,
            ..v2
        },
        TerrainParams {
            ridge_octaves: 0,
            ..v2
        },
        TerrainParams {
            ridge_octaves: 25,
            ..v2
        },
        TerrainParams {
            ridge_frequency: 1e12,
            ..v2
        },
    ];
    for p in bad {
        assert!(
            matches!(
                PlanetTerrain::new(1, p, N),
                Err(TerrainError::InvalidParams(_))
            ),
            "{p:?}"
        );
    }
    // Ridge fields are ignored for version 1.
    let ignored = TerrainParams {
        ridge_amplitude: -5.0,
        ridge_frequency: 0.0,
        ridge_octaves: 99,
        ..TerrainParams::default()
    };
    assert!(ignored.validate().is_ok());
}

#[test]
fn v2_heights_stay_within_max_relief() {
    let t = terrain_v2();
    let p = *t.params();
    assert!(p.max_relief() > TerrainParams::default().max_relief());
    let mut state = 0x1234_5678_9ABC_DEF1u64;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 11) as f64) / 9_007_199_254_740_992.0 * 2.0 - 1.0
    };
    for _ in 0..10_000 {
        let v = DVec3::new(next(), next(), next());
        if v.length() < 1e-3 {
            continue;
        }
        let h = t.sample(v.normalize()).height;
        assert!(h >= p.sea_level && h <= p.max_relief());
    }
}

/// Words hashed for one patch: id, then canonical height bits for all samples.
fn height_words(t: &PlanetTerrain, id: PatchId) -> Vec<u64> {
    let mut w = vec![id.raw()];
    for s in t.patch_samples(id) {
        w.push(canonical_bits(s.height).expect("finite height"));
    }
    w
}

fn dir_words(id: PatchId) -> Vec<u64> {
    let mut w = vec![id.raw()];
    for j in 0..=N {
        for i in 0..=N {
            let d = vertex_dir(id, N, i, j);
            for c in [d.x, d.y, d.z] {
                w.push(canonical_bits(c).expect("finite direction"));
            }
        }
    }
    w
}

fn combine(parts: Vec<Vec<u64>>) -> u64 {
    let mut h = Fnv1a64::new();
    for part in parts {
        for w in part {
            h.write_u64(w);
        }
    }
    h.finish()
}

fn run<F>(threads: u32, f: F) -> u64
where
    F: Fn(PatchId) -> Vec<u64> + Sync,
{
    let ids = patches();
    if threads == 1 {
        return combine(ids.iter().map(|&id| f(id)).collect());
    }
    let f = &f;
    let parts = std::thread::scope(|s| {
        let handles: Vec<_> = ids.iter().map(|&id| s.spawn(move || f(id))).collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("worker panicked"))
            .collect::<Vec<_>>()
    });
    combine(parts)
}

fn check_goldens(t: &PlanetTerrain, name: &str, gh: u64, gd: u64) {
    let h1 = run(1, |id| height_words(t, id));
    let h4 = run(4, |id| height_words(t, id));
    let d1 = run(1, dir_words);
    let d4 = run(4, dir_words);
    eprintln!("GOLDEN_HEIGHTS_{name} = {h1:#018x}; GOLDEN_DIRS_{name} = {d1:#018x}");
    assert_eq!(h1, h4, "heights differ between 1 and 4 threads");
    assert_eq!(d1, d4, "directions differ between 1 and 4 threads");
    assert_eq!(h1, gh);
    assert_eq!(d1, gd);
}

#[test]
fn goldens_match_at_1_and_4_threads_v1() {
    check_goldens(&terrain(), "V1", GOLDEN_HEIGHTS_V1, GOLDEN_DIRS_V1);
}

#[test]
fn goldens_match_at_1_and_4_threads_v2() {
    // Directions do not depend on the generator version: the v1 constant applies.
    check_goldens(&terrain_v2(), "V2", GOLDEN_HEIGHTS_V2, GOLDEN_DIRS_V1);
}
