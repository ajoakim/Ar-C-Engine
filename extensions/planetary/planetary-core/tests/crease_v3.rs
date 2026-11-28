//! Crease guard (11-06C, ADR-0020): the v3 mountains and ridge shapes must not carry a C0 crease.
//! Linear interpolation between mesh vertices cuts a crest by about `slope_jump * step`, which shows
//! as a regular tooth row that grows with `relief_scale`. The test interpolates true heights at
//! the vertex step along lines through mountain-mask sites and bounds the interpolation error
//! divided by the step. The negative control rebuilds the pre-11-06C `|x|` shape (kept only here)
//! and must exceed the same bound.

use engine_sdk::DVec3;
use engine_sdk::procedural::fbm_3d;
use engine_sdk::procedural::splitmix64;
use planetary_core::terrain::{compose_v3, hills_at, smoothstep};
use planetary_core::{PlanetTerrain, TerrainParams, base_at, detail_at, mask_at, ridge_at};

const SEED: u64 = 0x0007_5EED_2026_0011;
const SITES: usize = 320;
const INTERVALS: usize = 32;
/// Vertex steps in meters (about hex L12 / L10 spacing).
const STEPS: [f64; 2] = [17.0, 67.0];

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

fn scaled(scale: f64) -> TerrainParams {
    TerrainParams {
        relief_scale: scale,
        ..TerrainParams::demo_v3()
    }
}

/// Mountain-mask sites: gate above 0.8 and above sea level (the mountains term is active).
fn sites(t: &PlanetTerrain) -> Vec<DVec3> {
    let p = t.params();
    let mut out = Vec::new();
    let mut k = 0u64;
    while out.len() < SITES {
        let d = dir(k + 5000);
        k += 1;
        let s = t.sample(d);
        let g = smoothstep(p.range_gate_lo, p.range_gate_hi, s.mask);
        if g > 0.8 && s.height > p.sea_level + 50.0 {
            out.push(d);
        }
        assert!(k < 200_000, "not enough mountain sites");
    }
    out
}

/// Height with the pre-11-06C shapes: `|x|` crease in the mountains term (rebuilt directly from
/// the fbm; the 11-06D range-height and crest gain are not needed to expose the crease) and the
/// unrounded v2 ridge shape.
fn old_height(t: &PlanetTerrain, d: DVec3) -> f64 {
    let p = t.params();
    let s = t.seeds();
    let base = base_at(s.base, p, d);
    let detail = detail_at(s.detail, p, d);
    let mask = mask_at(s.mask, p, d);
    let ridge = ridge_at(s.ridge, p, d);
    let hills = hills_at(s.hills, p, d);
    let f = p.mountains_frequency;
    let v = fbm_3d(s.mountains, d.x * f, d.y * f, d.z * f, p.mountains_octaves);
    let r_old = 1.0 - (2.0 * v - 1.0).abs();
    let mtn = p.mountains_amplitude * (r_old * r_old);
    compose_v3(p, base, detail, mask, ridge, hills, mtn)
}

/// Max over a 2 km-class line of `|true - linear interpolation between vertices| / step`.
fn crease_error(
    t: &PlanetTerrain,
    site: DVec3,
    k: u64,
    step: f64,
    height: &dyn Fn(DVec3) -> f64,
) -> f64 {
    let radius = t.params().radius;
    let axis = if site.x.abs() < 0.8 {
        DVec3::new(1.0, 0.0, 0.0)
    } else {
        DVec3::new(0.0, 1.0, 0.0)
    };
    let e1 = site.cross(axis).normalize();
    let e2 = site.cross(e1);
    let a = unit_f(k + 91) * std::f64::consts::TAU;
    let tangent = e1 * a.cos() + e2 * a.sin();
    let at = |s: f64| (site + tangent * (s / radius)).normalize();
    let phase = unit_f(k + 7) * step;
    let h: Vec<f64> = (0..=INTERVALS)
        .map(|i| height(at(phase + i as f64 * step)))
        .collect();
    let mut worst = 0.0f64;
    for i in 0..INTERVALS {
        for f in [0.25, 0.5, 0.75] {
            let truth = height(at(phase + (i as f64 + f) * step));
            let lerp = h[i] + (h[i + 1] - h[i]) * f;
            worst = worst.max((truth - lerp).abs() / step);
        }
    }
    worst
}

fn stats(
    t: &PlanetTerrain,
    sites: &[DVec3],
    step: f64,
    height: &dyn Fn(DVec3) -> f64,
) -> (f64, f64) {
    let mut v: Vec<f64> = sites
        .iter()
        .enumerate()
        .map(|(k, s)| crease_error(t, *s, k as u64, step, height))
        .collect();
    v.sort_by(|a, b| a.total_cmp(b));
    (v[(v.len() - 1) * 9 / 10], v[v.len() - 1])
}

/// Pinned bounds on the worst (max over sites) error / step, per relief scale. The pre-fix
/// shape measured about 0.5 / 1.0 / 1.5 (diagnosis), the rounded shape measures 0.005-0.067 (max over 320 sites, steps 17 and 67 m, relief 1-3).
fn bound(scale: f64) -> f64 {
    BOUND_PER_RELIEF * scale
}
const BOUND_PER_RELIEF: f64 = 0.05;

#[test]
fn rounded_crease_keeps_interpolation_error_small_at_relief_1_2_3() {
    for scale in [1.0, 2.0, 3.0] {
        let t = PlanetTerrain::new(SEED, scaled(scale), 32).expect("terrain");
        let s = sites(&t);
        for step in STEPS {
            let (p90, max) = stats(&t, &s, step, &|d| t.height_at_dir(d));
            eprintln!("relief {scale} step {step}: new p90 {p90:.4} max {max:.4}");
            assert!(
                max <= bound(scale),
                "relief {scale} step {step}: crease error/step {max:.4} > {:.3}",
                bound(scale)
            );
        }
    }
}

#[test]
fn negative_control_old_abs_shape_violates_the_bound() {
    for scale in [1.0, 2.0, 3.0] {
        let t = PlanetTerrain::new(SEED, scaled(scale), 32).expect("terrain");
        let s = sites(&t);
        let step = STEPS[0];
        let (p90, max) = stats(&t, &s, step, &|d| old_height(&t, d));
        eprintln!("relief {scale} step {step}: old p90 {p90:.4} max {max:.4}");
        assert!(
            max > bound(scale),
            "relief {scale}: old shape max {max:.4} did not exceed {:.3}",
            bound(scale)
        );
    }
}
