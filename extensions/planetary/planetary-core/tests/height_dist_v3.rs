//! Height distribution of generator v3 (11-06D, ADR-0020 item 9). User feedback after flying
//! 11-06C: "all the hills/mountains are the same height ... elevations should fall on a bell
//! curve". The cause was that `r = 1 - round_abs(2 f - 1)` of a 5-octave fbm sits near 1 almost
//! everywhere (the fbm is concentrated around 0.5), so `A * r * r` saturates at the amplitude and
//! every crest reaches about the same height. This test pins the shape of the land-height and the
//! mountain-peak distributions; the negative control rebuilds the old shape and must fail the
//! "peaks are not all equal" bound.

use engine_sdk::DVec3;
use engine_sdk::procedural::{fbm_3d, splitmix64};
use planetary_core::terrain::{
    CREASE_EPS, MOUNTAIN_SHAPE_SLOPE, compose_v3, hills_at, mountains_at, ridge_at_v3, round_abs,
    smoothstep,
};
use planetary_core::{PlanetTerrain, TerrainParams, base_at, detail_at, mask_at};

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

/// Which mountains shape the height function uses.
#[derive(Clone, Copy, PartialEq)]
enum Shape {
    /// The library shape (`mountains_at`).
    Current,
    /// The pre-11-06D shape rebuilt here: `A * r * r`, no range-height modulation.
    Old,
}

fn height(t: &PlanetTerrain, shape: Shape, d: DVec3) -> f64 {
    let p = t.params();
    let s = t.seeds();
    let base = base_at(s.base, p, d);
    let detail = detail_at(s.detail, p, d);
    let mask = mask_at(s.mask, p, d);
    let ridge = ridge_at_v3(s.ridge, p, d);
    let hills = hills_at(s.hills, p, d);
    let mtn = match shape {
        Shape::Current => mountains_at(s.mountains, p, d),
        Shape::Old => {
            let f = p.mountains_frequency;
            let v = fbm_3d(s.mountains, d.x * f, d.y * f, d.z * f, p.mountains_octaves);
            let r = 1.0 - round_abs(2.0 * v - 1.0);
            p.mountains_amplitude * (r * r)
        }
    };
    compose_v3(p, base, detail, mask, ridge, hills, mtn)
}

/// Statistics of a sorted sample.
struct Stats {
    n: usize,
    mean: f64,
    std: f64,
    skew: f64,
    kurt: f64,
    p: [f64; 7],
}

const PCTS: [f64; 7] = [0.05, 0.25, 0.5, 0.75, 0.95, 0.99, 1.0];

fn stats(v: &mut [f64]) -> Stats {
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    let mean = v.iter().sum::<f64>() / n as f64;
    let var = v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n as f64;
    let std = var.sqrt();
    let skew = v.iter().map(|x| ((x - mean) / std).powi(3)).sum::<f64>() / n as f64;
    let kurt = v.iter().map(|x| ((x - mean) / std).powi(4)).sum::<f64>() / n as f64;
    let mut p = [0.0; 7];
    for (o, f) in p.iter_mut().zip(PCTS) {
        *o = v[((n - 1) as f64 * f) as usize];
    }
    Stats {
        n,
        mean,
        std,
        skew,
        kurt,
        p,
    }
}

fn show(name: &str, s: &Stats) {
    eprintln!(
        "{name}: n {} mean {:.0} std {:.0} (cv {:.3}) skew {:.3} kurt {:.2} | p5 {:.0} p25 {:.0} p50 {:.0} p75 {:.0} p95 {:.0} p99 {:.0} max {:.0}",
        s.n,
        s.mean,
        s.std,
        s.std / s.mean,
        s.skew,
        s.kurt,
        s.p[0],
        s.p[1],
        s.p[2],
        s.p[3],
        s.p[4],
        s.p[5],
        s.p[6]
    );
}

/// Fraction of the mass of an equal-width 12-bin histogram between min and max (printed).
fn histogram(name: &str, v: &[f64]) {
    let (lo, hi) = (v[0], v[v.len() - 1]);
    let mut bins = [0usize; 12];
    for x in v {
        let b = (((x - lo) / (hi - lo)) * 12.0) as usize;
        bins[b.min(11)] += 1;
    }
    let peak = *bins.iter().max().unwrap() as f64;
    eprintln!("{name} histogram {lo:.0}..{hi:.0} m:");
    for (i, c) in bins.iter().enumerate() {
        let bar = "#".repeat((40.0 * *c as f64 / peak) as usize);
        eprintln!(
            "  {:6.0} {:5.1}% {bar}",
            lo + (hi - lo) * i as f64 / 12.0,
            100.0 * *c as f64 / v.len() as f64
        );
    }
}

/// Land heights above sea level over uniformly random directions.
fn land_heights(t: &PlanetTerrain, shape: Shape) -> Vec<f64> {
    let sea = t.params().sea_level;
    let mut out = Vec::new();
    for k in 0..60_000u64 {
        let h = height(t, shape, dir(k + 77));
        if h > sea + 1.0 {
            out.push(h - sea);
        }
    }
    out
}

/// Local maxima of `f` reached by pattern-search hill climbing (step 3 km down to 200 m) from
/// 700 random starts inside mountain regions (gate above 0.9, above sea), de-duplicated at 2 km.
fn climb_peaks(t: &PlanetTerrain, f: &dyn Fn(DVec3) -> f64) -> Vec<f64> {
    let p = *t.params();
    let ang0 = 3000.0 / p.radius;
    let ang_min = 200.0 / p.radius;
    let mut found: Vec<(DVec3, f64)> = Vec::new();
    let mut tried = 0u64;
    let mut k = 0u64;
    while tried < 700 {
        let mut d = dir(k + 900_001);
        k += 1;
        let s = t.sample(d);
        if smoothstep(p.range_gate_lo, p.range_gate_hi, s.mask) <= 0.9 || s.height <= p.sea_level {
            continue;
        }
        tried += 1;
        let mut h = f(d);
        let mut ang = ang0;
        while ang > ang_min {
            let axis = if d.x.abs() < 0.9 {
                DVec3::new(1.0, 0.0, 0.0)
            } else {
                DVec3::new(0.0, 1.0, 0.0)
            };
            let e1 = d.cross(axis).normalize();
            let e2 = d.cross(e1);
            let mut best = (h, d);
            for i in 0..8 {
                let a = f64::from(i) * std::f64::consts::FRAC_PI_4;
                let c = (d + (e1 * a.cos() + e2 * a.sin()) * ang).normalize();
                let hc = f(c);
                if hc > best.0 {
                    best = (hc, c);
                }
            }
            if best.0 > h {
                h = best.0;
                d = best.1;
            } else {
                ang *= 0.5;
            }
        }
        let dup = found
            .iter()
            .any(|(q, _)| (*q - d).length() * p.radius < 2000.0);
        if !dup {
            found.push((d, h));
        }
    }
    found.into_iter().map(|(_, h)| h).collect()
}

/// Heights above sea of the crests of the total height.
fn peak_heights(t: &PlanetTerrain, shape: Shape) -> Vec<f64> {
    let sea = t.params().sea_level;
    climb_peaks(t, &|d| height(t, shape, d))
        .into_iter()
        .map(|h| h - sea)
        .collect()
}

/// The mountains shape (`term / amplitude`, in `[0, 1]`) for the chosen shape.
fn mountain_shape(t: &PlanetTerrain, shape: Shape, d: DVec3) -> f64 {
    let p = t.params();
    let seed = t.seeds().mountains;
    match shape {
        Shape::Current => mountains_at(seed, p, d) / p.mountains_amplitude,
        Shape::Old => {
            let f = p.mountains_frequency;
            let v = fbm_3d(seed, d.x * f, d.y * f, d.z * f, p.mountains_octaves);
            let r = 1.0 - round_abs(2.0 * v - 1.0);
            r * r
        }
    }
}

/// Crest values of the mountains shape itself (1.0 = full amplitude).
fn crest_shapes(t: &PlanetTerrain, shape: Shape) -> Vec<f64> {
    climb_peaks(t, &|d| mountain_shape(t, shape, d))
}

fn terrain() -> PlanetTerrain {
    PlanetTerrain::new(SEED, TerrainParams::demo_v3(), 32).expect("terrain")
}

/// 12-bin histogram counts between min and max of a sorted sample.
fn bins(v: &[f64]) -> [usize; 12] {
    let (lo, hi) = (v[0], v[v.len() - 1]);
    let mut b = [0usize; 12];
    for x in v {
        b[((((x - lo) / (hi - lo)) * 12.0) as usize).min(11)] += 1;
    }
    b
}

/// Unimodal up to noise: left of the mode the counts never fall, right of it never rise, by more
/// than `noise` times the mode count (0.1 for the 60000 land samples, 0.2 for the 700 crests).
fn unimodal(b: &[usize; 12], noise: f64) -> bool {
    let (mode, max) = b
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| **c)
        .map(|(i, c)| (i, *c))
        .unwrap();
    let tol = max as f64 * noise;
    (1..=mode).all(|i| b[i] as f64 + tol >= b[i - 1] as f64)
        && (mode + 1..12).all(|i| b[i] as f64 <= b[i - 1] as f64 + tol)
}

/// Every shape bound the test pins, as the names of the violated ones (empty = all hold).
fn violations(t: &PlanetTerrain, shape: Shape) -> Vec<&'static str> {
    let mut bad = Vec::new();
    let mut land = land_heights(t, shape);
    let l = stats(&mut land);
    show("land", &l);
    histogram("land", &land);
    if !(0.3..=1.5).contains(&l.skew) {
        bad.push("land skew");
    }
    if !(1.8..=4.0).contains(&(l.p[4] / l.p[2])) {
        bad.push("land p95/p50");
    }
    if !unimodal(&bins(&land), 0.1) {
        bad.push("land unimodal");
    }
    let mut peaks = peak_heights(t, shape);
    let ps = stats(&mut peaks);
    show("peaks", &ps);
    histogram("peaks", &peaks);
    eprintln!("peak p99/p50 {:.3}", ps.p[5] / ps.p[2]);
    if ps.std / ps.mean < 0.18 {
        bad.push("peak cv");
    }
    if !(-0.7..=0.7).contains(&ps.skew) {
        bad.push("peak skew");
    }
    if !(1.35..=2.2).contains(&(ps.p[5] / ps.p[2])) {
        bad.push("peak p99/p50");
    }
    if !unimodal(&bins(&peaks), 0.2) {
        bad.push("peak unimodal");
    }
    let mut crest = crest_shapes(t, shape);
    let cs = stats(&mut crest);
    let hi = crest.iter().filter(|c| **c > 0.9).count() as f64 / crest.len() as f64;
    eprintln!(
        "crest shape: n {} mean {:.3} std {:.3} cv {:.3} skew {:.3} p5 {:.3} p25 {:.3} p50 {:.3} p75 {:.3} p95 {:.3} frac>0.9 {hi:.3}",
        cs.n,
        cs.mean,
        cs.std,
        cs.std / cs.mean,
        cs.skew,
        cs.p[0],
        cs.p[1],
        cs.p[2],
        cs.p[3],
        cs.p[4]
    );
    if cs.std / cs.mean < 0.13 {
        bad.push("crest cv");
    }
    if hi > 0.2 {
        bad.push("crest frac above 0.9");
    }
    if !(-1.0..=0.5).contains(&cs.skew) {
        bad.push("crest skew");
    }
    if !(0.5..=0.85).contains(&cs.mean) {
        bad.push("crest mean");
    }
    if !unimodal(&bins(&crest), 0.2) {
        bad.push("crest unimodal");
    }
    bad
}

#[test]
fn v3_heights_are_bell_shaped_and_peaks_differ() {
    let bad = violations(&terrain(), Shape::Current);
    assert!(bad.is_empty(), "violated bounds: {bad:?}");
}

/// Negative control: the pre-11-06D shape must fail the "peaks are not all equal" bounds.
#[test]
fn old_shape_fails_peaks_not_all_equal() {
    let bad = violations(&terrain(), Shape::Old);
    eprintln!("old shape violations: {bad:?}");
    assert!(bad.contains(&"crest cv"), "{bad:?}");
    assert!(bad.contains(&"crest frac above 0.9"), "{bad:?}");
    assert!(bad.contains(&"peak cv"), "{bad:?}");
    assert!(bad.contains(&"peak p99/p50"), "{bad:?}");
}

/// `MOUNTAIN_SHAPE_SLOPE` must bound `max (1 - round_abs(x)) * round_abs'(x)` (the LodModel proof
/// uses it): checked on a 2e6-point grid of `[0, 1]` (the function is smooth, its slope is below
/// 10, so the grid error is below 1e-5).
#[test]
fn shape_slope_constant_bounds_the_true_maximum() {
    let e = CREASE_EPS;
    let mut best = 0.0f64;
    for i in 0..=2_000_000u32 {
        let x = f64::from(i) / 2_000_000.0;
        let u = round_abs(x);
        let du = x / (x * x + e * e).sqrt();
        best = best.max((1.0 - u) * du);
    }
    eprintln!("max (1 - u) u' = {best:.5}");
    assert!(
        best <= MOUNTAIN_SHAPE_SLOPE,
        "{best} > {MOUNTAIN_SHAPE_SLOPE}"
    );
    assert!(
        best >= MOUNTAIN_SHAPE_SLOPE - 0.03,
        "constant {MOUNTAIN_SHAPE_SLOPE} is not tight against {best}"
    );
}
