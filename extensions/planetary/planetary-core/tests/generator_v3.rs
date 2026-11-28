//! Generator v3 (hills + mountains) tests: inert-layer guard against v2, determinism, stack ==
//! pure function, range, added relief, thread agreement, validation. No v3 golden is pinned:
//! the constants are starting estimates calibrated later.

use engine_sdk::DVec3;
use engine_sdk::procedural::{Fnv1a64, Params, StableId, canonical_bits, layer_seed};
use planetary_core::terrain::{HILLS_LAYER, MOUNTAINS_LAYER, hills_at, mountains_at, ridge_at_v3};
use planetary_core::{
    Face, PatchId, PlanetTerrain, RIDGE_LAYER, TerrainError, TerrainParams, base_at, compose_v2,
    detail_at, mask_at, vertex_dir,
};

const WORLD_SEED: u64 = 0x0007_5EED_2026_0001;
const N: u32 = 8;

fn terrain_v2() -> PlanetTerrain {
    PlanetTerrain::new(WORLD_SEED, TerrainParams::demo_v2(), N).unwrap()
}

fn terrain_v3() -> PlanetTerrain {
    PlanetTerrain::new(WORLD_SEED, TerrainParams::demo_v3(), N).unwrap()
}

fn patches() -> [PatchId; 4] {
    [
        PatchId::root(Face::PosX),
        PatchId::new(Face::PosY, 3, 7, 3).unwrap(),
        PatchId::new(Face::PosZ, 2, 0, 0).unwrap(),
        PatchId::new(Face::NegX, 19, 123_456, 201_234).unwrap(),
    ]
}

fn directions(count: usize) -> Vec<DVec3> {
    let mut state = 0x1234_5678_9ABC_DEF1u64;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 11) as f64) / 9_007_199_254_740_992.0 * 2.0 - 1.0
    };
    let mut out = Vec::with_capacity(count);
    while out.len() < count {
        let v = DVec3::new(next(), next(), next());
        if v.length() < 1e-3 {
            continue;
        }
        out.push(v.normalize());
    }
    out
}

fn inert_v3() -> TerrainParams {
    TerrainParams {
        generator_version: 3,
        hills_amplitude: 0.0,
        mountains_amplitude: 0.0,
        ..TerrainParams::demo_v2()
    }
}

#[test]
fn v3_without_new_layers_equals_v2() {
    // The v3 ridge uses the rounded crease shape (11-06C), so with zero hills and mountains v3
    // equals the v2 composition of the same base, detail, mask and the v3 ridge value.
    let v3 = PlanetTerrain::new(WORLD_SEED, inert_v3(), N).unwrap();
    let p = *v3.params();
    let seeds = v3.seeds();
    let expect = |d: DVec3| {
        let (base, detail, mask) = (
            base_at(seeds.base, &p, d),
            detail_at(seeds.detail, &p, d),
            mask_at(seeds.mask, &p, d),
        );
        let ridge = ridge_at_v3(seeds.ridge, &p, d);
        (compose_v2(&p, base, detail, mask, ridge), mask)
    };
    for id in patches() {
        let b = v3.patch_samples(id);
        for j in 0..=N {
            for i in 0..=N {
                let k = (j * (N + 1) + i) as usize;
                let (h, m) = expect(vertex_dir(id, N, i, j));
                assert_eq!(b[k].height.to_bits(), h.to_bits());
                assert_eq!(b[k].mask.to_bits(), m.to_bits());
            }
        }
    }
    for d in directions(2000) {
        let b = v3.sample(d);
        let (h, m) = expect(d);
        assert_eq!(b.height.to_bits(), h.to_bits());
        assert_eq!(b.mask.to_bits(), m.to_bits());
    }
}

#[test]
fn v3_is_deterministic_and_seed_dependent() {
    let a = terrain_v3();
    let b = terrain_v3();
    let other = PlanetTerrain::new(WORLD_SEED + 1, TerrainParams::demo_v3(), N).unwrap();
    let mut differs = false;
    for id in patches() {
        assert_eq!(a.patch_samples(id), b.patch_samples(id));
        differs |= a.patch_samples(id) != other.patch_samples(id);
    }
    assert!(differs);
}

#[test]
fn v3_stack_equals_pure_function() {
    let t = terrain_v3();
    let seeds = t.seeds();
    let p = *t.params();
    assert_eq!(seeds.ridge, layer_seed(WORLD_SEED, "planet.ridge", 2));
    assert_eq!(seeds.hills, layer_seed(WORLD_SEED, "planet.hills", 3));
    assert_eq!(
        seeds.mountains,
        layer_seed(WORLD_SEED, "planet.mountains", 3)
    );
    for id in patches() {
        let out = t.stack().generate(StableId(id.raw()), &Params::new());
        let ridge = out.layer(RIDGE_LAYER).unwrap();
        let hills = out.layer(HILLS_LAYER).unwrap();
        let mtn = out.layer(MOUNTAINS_LAYER).unwrap();
        let samples = t.patch_samples(id);
        assert_eq!(samples.len(), ((N + 1) * (N + 1)) as usize);
        for j in 0..=N {
            for i in 0..=N {
                let k = (j * (N + 1) + i) as usize;
                let d = vertex_dir(id, N, i, j);
                assert_eq!(
                    ridge[k].to_bits(),
                    ridge_at_v3(seeds.ridge, &p, d).to_bits()
                );
                assert_eq!(hills[k].to_bits(), hills_at(seeds.hills, &p, d).to_bits());
                assert_eq!(
                    mtn[k].to_bits(),
                    mountains_at(seeds.mountains, &p, d).to_bits()
                );
                assert_eq!(samples[k].height.to_bits(), t.height_at_dir(d).to_bits());
                assert_eq!(samples[k].height.to_bits(), t.sample(d).height.to_bits());
            }
        }
    }
}

#[test]
fn v3_heights_stay_within_max_relief() {
    let t = terrain_v3();
    let p = *t.params();
    assert!(p.max_relief() > TerrainParams::demo_v2().max_relief());
    assert_eq!(
        p.max_relief(),
        p.base_amplitude
            + p.detail_amplitude
            + p.ridge_amplitude
            + p.hills_amplitude
            + p.mountains_amplitude
    );
    for d in directions(10_000) {
        let h = t.sample(d).height;
        assert!(h >= p.sea_level && h <= p.max_relief());
    }
}

#[test]
fn v3_adds_relief_over_v2() {
    let v2 = terrain_v2();
    let v3 = terrain_v3();
    let sea = TerrainParams::demo_v2().sea_level;
    let (mut lo2, mut hi2, mut lo3, mut hi3) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    let (mut land, mut differing) = (0usize, 0usize);
    for d in directions(2000) {
        let a = v2.sample(d).height;
        let b = v3.sample(d).height;
        if a > sea {
            lo2 = lo2.min(a);
            hi2 = hi2.max(a);
        }
        if b > sea {
            lo3 = lo3.min(b);
            hi3 = hi3.max(b);
        }
        if a > sea && b > sea {
            land += 1;
            if a.to_bits() != b.to_bits() {
                differing += 1;
            }
        }
    }
    assert!(land > 0);
    assert!(hi3 - lo3 > hi2 - lo2, "v3 {} v2 {}", hi3 - lo3, hi2 - lo2);
    assert!(differing * 10 > land * 9, "{differing} of {land}");
}

fn height_words(t: &PlanetTerrain, id: PatchId) -> Vec<u64> {
    let mut w = vec![id.raw()];
    for s in t.patch_samples(id) {
        w.push(canonical_bits(s.height).expect("finite height"));
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

fn run(threads: u32, t: &PlanetTerrain) -> u64 {
    let ids = patches();
    if threads == 1 {
        return combine(ids.iter().map(|&id| height_words(t, id)).collect());
    }
    let parts = std::thread::scope(|s| {
        let handles: Vec<_> = ids
            .iter()
            .map(|&id| s.spawn(move || height_words(t, id)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("worker panicked"))
            .collect::<Vec<_>>()
    });
    combine(parts)
}

#[test]
fn v3_threads_1_and_4_agree() {
    let t = terrain_v3();
    let h1 = run(1, &t);
    let h4 = run(4, &t);
    eprintln!("v3 height hash (not pinned) = {h1:#018x}");
    assert_eq!(h1, h4);
}

#[test]
fn v3_validation() {
    let v3 = TerrainParams::demo_v3();
    assert!(v3.validate().is_ok());
    // Zero amplitudes are allowed (inert-layer guard).
    assert!(inert_v3().validate().is_ok());
    for version in [0u32, 4] {
        assert!(
            TerrainParams {
                generator_version: version,
                ..v3
            }
            .validate()
            .is_err()
        );
    }
    let bad = [
        TerrainParams {
            range_gate_lo: 0.6,
            range_gate_hi: 0.6,
            ..v3
        },
        TerrainParams {
            range_gate_lo: 0.7,
            range_gate_hi: 0.5,
            ..v3
        },
        TerrainParams {
            range_gate_lo: -0.1,
            ..v3
        },
        TerrainParams {
            range_gate_hi: 1.1,
            ..v3
        },
        TerrainParams {
            range_gate_lo: f64::NAN,
            ..v3
        },
        TerrainParams {
            hills_amplitude: -1.0,
            ..v3
        },
        TerrainParams {
            mountains_amplitude: f64::INFINITY,
            ..v3
        },
        TerrainParams {
            hills_frequency: 0.0,
            ..v3
        },
        TerrainParams {
            hills_octaves: 0,
            ..v3
        },
        TerrainParams {
            hills_octaves: 25,
            ..v3
        },
        TerrainParams {
            mountains_octaves: 0,
            ..v3
        },
        TerrainParams {
            mountains_frequency: 1e12,
            ..v3
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
    // v3 fields are ignored below version 3.
    let ignored = TerrainParams {
        hills_amplitude: -5.0,
        mountains_octaves: 99,
        range_gate_lo: 0.9,
        range_gate_hi: 0.1,
        ..TerrainParams::demo_v2()
    };
    assert!(ignored.validate().is_ok());
}
