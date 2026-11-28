//! `relief_scale` (generator v3 parameter, ADR-0020): scale 1.0 is bit-identical to the v3 output
//! recorded before the parameter existed; larger scales raise heights and slopes; v1/v2 ignore it.

use engine_sdk::DVec3;
use engine_sdk::procedural::{Fnv1a64, splitmix64};
use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
use planetary_core::{PlanetTerrain, TerrainParams};

const SEED: u64 = 0x0007_5EED_2026_0011;

fn unit_f(k: u64) -> f64 {
    (splitmix64(k) >> 11) as f64 / (1u64 << 53) as f64
}

fn dir(k: u64) -> DVec3 {
    let mut i = 0u64;
    loop {
        let base = k.wrapping_mul(0x9E37_79B9).wrapping_add(i.wrapping_mul(3));
        let v = DVec3::new(
            unit_f(base) * 2.0 - 1.0,
            unit_f(base + 1_000_003) * 2.0 - 1.0,
            unit_f(base + 2_000_003) * 2.0 - 1.0,
        );
        let l2 = v.x * v.x + v.y * v.y + v.z * v.z;
        if l2 > 0.01 && l2 <= 1.0 {
            let l = l2.sqrt();
            return DVec3::new(v.x / l, v.y / l, v.z / l);
        }
        i += 1;
    }
}

/// FNV-1a over the bit patterns of 4000 heights, the masks, `max_relief` and the 25 residuals.
fn v3_fingerprint(p: TerrainParams) -> u64 {
    let t = PlanetTerrain::new(SEED, p, 32).expect("terrain");
    let mut h = Fnv1a64::new();
    for k in 0..4000u64 {
        let s = t.sample(dir(k));
        h.write_u64(s.height.to_bits());
        h.write_u64(s.mask.to_bits());
    }
    h.write_u64(p.max_relief().to_bits());
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    h.write_u64(m.height.max.to_bits());
    for r in m.residual {
        h.write_u64(r.to_bits());
    }
    h.finish()
}

/// Recorded at relief 1.0 (`TerrainParams::demo_v3()`). Re-recorded by 11-06D (range-height
/// multiplier, mountains amplitude 3900 -> 4700 m, gate 0.44..0.74 -> 0.42..0.72); the 11-06C value was
/// 0x6112_42bd_bf33_ae2a and the 11-06B value 0x1b54_f6eb_f399_a5d9.
/// Guards that `relief_scale` 1.0 stays an exact no-op, not a v3 golden (v3 is still unflown).
const V3_FINGERPRINT_BEFORE: u64 = 0x786b_bfbf_93f3_04fe;

fn scaled(scale: f64) -> TerrainParams {
    TerrainParams {
        relief_scale: scale,
        ..TerrainParams::demo_v3()
    }
}

#[test]
fn scale_one_is_bit_identical_to_pre_change_v3() {
    assert_eq!(TerrainParams::demo_v3().relief_scale, 1.0);
    assert_eq!(
        v3_fingerprint(TerrainParams::demo_v3()),
        V3_FINGERPRINT_BEFORE
    );
    assert_eq!(v3_fingerprint(scaled(1.0)), V3_FINGERPRINT_BEFORE);
}

#[test]
fn v1_and_v2_ignore_relief_scale() {
    for base in [TerrainParams::default(), TerrainParams::demo_v2()] {
        let a = PlanetTerrain::new(SEED, base, 32).expect("a");
        let b = PlanetTerrain::new(
            SEED,
            TerrainParams {
                relief_scale: 5.0,
                ..base
            },
            32,
        )
        .expect("b");
        assert_eq!(
            base.max_relief().to_bits(),
            b.params().max_relief().to_bits()
        );
        for k in 0..500u64 {
            let d = dir(k);
            assert_eq!(a.height_at_dir(d).to_bits(), b.height_at_dir(d).to_bits());
        }
        let ma = LodModel::from_terrain(&a, DEFAULT_KAPPA);
        let mb = LodModel::from_terrain(&b, DEFAULT_KAPPA);
        assert_eq!(ma, mb);
    }
}

#[test]
fn validation_rejects_bad_scales_for_v3_only() {
    for bad in [0.0, 0.09, 8.01, -1.0, f64::NAN, f64::INFINITY] {
        assert!(scaled(bad).validate().is_err(), "{bad}");
    }
    for good in [0.1, 1.0, 2.0, 3.0, 8.0] {
        assert!(scaled(good).validate().is_ok(), "{good}");
    }
}

#[test]
fn max_relief_scales_only_the_three_layers() {
    let p1 = scaled(1.0);
    let p2 = scaled(2.0);
    let fixed = p1.base_amplitude + p1.detail_amplitude;
    let rel = p1.ridge_amplitude + p1.hills_amplitude + p1.mountains_amplitude;
    assert!((p1.max_relief() - (fixed + rel)).abs() < 1e-9);
    assert!((p2.max_relief() - (fixed + 2.0 * rel)).abs() < 1e-9);
}

fn land_heights(p: TerrainParams) -> (f64, f64, f64) {
    let t = PlanetTerrain::new(SEED, p, 32).expect("terrain");
    let (mut lo, mut hi, mut observed_top) = (f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for k in 0..40_000u64 {
        let h = t.height_at_dir(dir(k + 77));
        observed_top = observed_top.max(h);
        if h > p.sea_level + 1.0 {
            lo = lo.min(h);
            hi = hi.max(h);
        }
    }
    assert!(observed_top <= p.max_relief());
    (lo, hi, hi - lo)
}

/// Mean land slope (m per rad) from a symmetric finite difference at 1e-4 rad.
fn mean_land_gradient(p: TerrainParams) -> f64 {
    let t = PlanetTerrain::new(SEED, p, 32).expect("terrain");
    let (mut sum, mut n) = (0.0, 0u32);
    let eps = 1.0e-4;
    for k in 0..6000u64 {
        let d = dir(k);
        if t.height_at_dir(d) <= p.sea_level + 1.0 {
            continue;
        }
        let a = DVec3::new(d.x + eps, d.y, d.z);
        let a = a.normalize();
        let b = DVec3::new(d.x - eps, d.y, d.z).normalize();
        sum += (t.height_at_dir(a) - t.height_at_dir(b)).abs();
        n += 1;
    }
    sum / f64::from(n)
}

#[test]
fn heights_and_slopes_grow_with_scale() {
    let (_, top1, range1) = land_heights(scaled(1.0));
    let (_, top2, range2) = land_heights(scaled(2.0));
    let (_, top3, range3) = land_heights(scaled(3.0));
    eprintln!(
        "land height range x1 {range1:.0} x2 {range2:.0} ({:.2}) x3 {range3:.0} ({:.2}); tops {top1:.0} {top2:.0} {top3:.0}",
        range2 / range1,
        range3 / range1
    );
    // Base and detail are deliberately not scaled, so the land range grows by less than the
    // scale itself (measured x1.73 at scale 2, x2.47 at scale 3).
    assert!(range2 >= 1.7 * range1, "{range2} vs {range1}");
    assert!(range3 >= 2.4 * range1, "{range3} vs {range1}");
    assert!(range3 > range2);
    let (g1, g2, g3) = (
        mean_land_gradient(scaled(1.0)),
        mean_land_gradient(scaled(2.0)),
        mean_land_gradient(scaled(3.0)),
    );
    eprintln!("mean land gradient m/rad: {g1:.0} {g2:.0} {g3:.0}");
    assert!(g2 > g1 && g3 > g2);
}
