//! Generator v3 calibration: land slope at mesh resolution, mountain share, relief versus v2.
//! The bands are the phase-11 decision D-02 (Earth-like relief, no exaggeration knob); they are
//! never relaxed to pass, the v3 constants in `terrain.rs` are tuned instead.

use engine_sdk::DVec3;
use engine_sdk::procedural::splitmix64;
use planetary_core::hex::mapping::locate as hex_locate;
use planetary_core::hex::mesh::{build_hex_cell, vertex_count_hex};
use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
use planetary_core::terrain::smoothstep;
use planetary_core::{HexSphere, PlanetTerrain, TerrainParams, Topology, build_patch, locate};

const SEED: u64 = 0x0007_5EED_2026_0011;
const DIRS: u64 = 240;

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

fn pct(v: &[f64], f: f64) -> f64 {
    v[((v.len() - 1) as f64 * f) as usize]
}

/// Land slope angles (degrees) of the grid vertices of real meshes.
fn cube_slopes(p: TerrainParams, level: u8) -> Vec<f64> {
    let n = 32u32;
    let t = PlanetTerrain::new(SEED, p, n).expect("terrain");
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let mut out = Vec::new();
    for k in 0..DIRS {
        let id = locate(dir(k), level);
        let mesh = build_patch(&t, &m, id);
        let w = n + 1;
        for v in mesh.vertices.iter().take((w * w) as usize) {
            let pos = DVec3::new(
                mesh.origin.x + f64::from(v.position[0]),
                mesh.origin.y + f64::from(v.position[1]),
                mesh.origin.z + f64::from(v.position[2]),
            );
            push_land(&mut out, &t, pos, v.normal);
        }
    }
    out.sort_by(|a, b| a.total_cmp(b));
    out
}

fn hex_slopes(p: TerrainParams, level: u8) -> Vec<f64> {
    let n = 16u32;
    let t = PlanetTerrain::new(SEED, p, n).expect("terrain");
    let hs = HexSphere::new(n).expect("hex");
    let m = LodModel::for_topology(&t, DEFAULT_KAPPA, hs.sample_angle0());
    let sv = ((n + 1) * (n + 2) / 2) as usize;
    let mut out = Vec::new();
    for k in 0..DIRS {
        let c = hex_locate(dir(k), level);
        let mesh = build_hex_cell(c, &t, &m, false, &|| false).expect("mesh");
        let sectors = if mesh.vertices.len() == vertex_count_hex(n) as usize {
            6
        } else {
            5
        };
        for v in mesh.vertices.iter().take(sectors * sv) {
            let pos = DVec3::new(
                mesh.origin.x + f64::from(v.position[0]),
                mesh.origin.y + f64::from(v.position[1]),
                mesh.origin.z + f64::from(v.position[2]),
            );
            push_land(&mut out, &t, pos, v.normal);
        }
    }
    out.sort_by(|a, b| a.total_cmp(b));
    out
}

fn push_land(out: &mut Vec<f64>, t: &PlanetTerrain, pos: DVec3, normal: [f32; 3]) {
    let len = (pos.x * pos.x + pos.y * pos.y + pos.z * pos.z).sqrt();
    let h = len - t.params().radius;
    if h <= t.params().sea_level + 1.0 {
        return;
    }
    let d = DVec3::new(pos.x / len, pos.y / len, pos.z / len);
    let nd = f64::from(normal[0]) * d.x + f64::from(normal[1]) * d.y + f64::from(normal[2]) * d.z;
    out.push(nd.clamp(-1.0, 1.0).acos().to_degrees());
}

fn report(name: &str, v: &[f64]) -> (f64, f64) {
    let plains = v.iter().filter(|a| **a < 1.0).count() as f64 / v.len() as f64;
    eprintln!(
        "{name}: n {} p50 {:.2} p90 {:.2} p99 {:.2} max {:.2} deg; 1-n.d p50 {:.2e} p99 {:.2e}; plains(<1deg) {:.3}",
        v.len(),
        pct(v, 0.5),
        pct(v, 0.9),
        pct(v, 0.99),
        v[v.len() - 1],
        1.0 - pct(v, 0.5).to_radians().cos(),
        1.0 - pct(v, 0.99).to_radians().cos(),
        plains
    );
    (pct(v, 0.5), pct(v, 0.99))
}

#[test]
fn v3_land_slope_percentiles_in_band() {
    let cube8 = cube_slopes(TerrainParams::demo_v3(), 8);
    let cube10 = cube_slopes(TerrainParams::demo_v3(), 10);
    let hex8 = hex_slopes(TerrainParams::demo_v3(), 8);
    for (name, v) in [
        ("cube L8", &cube8),
        ("cube L10", &cube10),
        ("hex L8", &hex8),
    ] {
        assert!(v.len() > 10_000, "{name}: {}", v.len());
        let (p50, p99) = report(name, v);
        assert!(
            (25.0..=35.0).contains(&p99),
            "{name}: land slope p99 {p99:.2} deg outside 25..=35"
        );
        assert!(p50 < 8.0, "{name}: land slope p50 {p50:.2} deg >= 8");
    }
}

fn land_mask_samples(p: &TerrainParams) -> (Vec<f64>, usize, usize) {
    let t = PlanetTerrain::new(SEED, *p, 32).expect("terrain");
    let mut masks = Vec::new();
    let mut land = 0usize;
    let mut mountain = 0usize;
    for k in 0..40_000u64 {
        let s = t.sample(dir(k + 77));
        if s.height <= p.sea_level + 1.0 {
            continue;
        }
        land += 1;
        masks.push(s.mask);
        if smoothstep(p.range_gate_lo, p.range_gate_hi, s.mask) > 0.5 {
            mountain += 1;
        }
    }
    masks.sort_by(|a, b| a.total_cmp(b));
    (masks, land, mountain)
}

#[test]
fn v3_mountain_share_and_gate_width() {
    let p = TerrainParams::demo_v3();
    let (masks, land, mountain) = land_mask_samples(&p);
    eprintln!(
        "mask percentiles (land): p10 {:.3} p25 {:.3} p50 {:.3} p75 {:.3} p90 {:.3} p99 {:.3}; gate {:.3}..{:.3}",
        pct(&masks, 0.1),
        pct(&masks, 0.25),
        pct(&masks, 0.5),
        pct(&masks, 0.75),
        pct(&masks, 0.9),
        pct(&masks, 0.99),
        p.range_gate_lo,
        p.range_gate_hi
    );
    let share = mountain as f64 / land as f64;
    eprintln!("mountain share of land (g > 0.5): {share:.3} ({mountain}/{land})");
    assert!(
        (0.15..=0.35).contains(&share),
        "mountain share {share:.3} outside 0.15..=0.35"
    );
    assert!(
        p.range_gate_hi - p.range_gate_lo >= 0.3 - 1e-12,
        "gate width {}",
        p.range_gate_hi - p.range_gate_lo
    );
}

fn land_heights(p: &TerrainParams) -> (Vec<f64>, f64) {
    let t = PlanetTerrain::new(SEED, *p, 32).expect("terrain");
    let mut hs = Vec::new();
    let mut top = f64::NEG_INFINITY;
    for k in 0..40_000u64 {
        let h = t.height_at_dir(dir(k + 77));
        top = top.max(h);
        if h > p.sea_level + 1.0 {
            hs.push(h - p.sea_level);
        }
    }
    hs.sort_by(|a, b| a.total_cmp(b));
    (hs, top)
}

#[test]
fn v3_relief_exceeds_v2() {
    let p3 = TerrainParams::demo_v3();
    let p2 = TerrainParams::demo_v2();
    let (h3, top3) = land_heights(&p3);
    let (h2, top2) = land_heights(&p2);
    let range3 = h3[h3.len() - 1] - h3[0];
    let range2 = h2[h2.len() - 1] - h2[0];
    let (q3, q2) = (pct(&h3, 0.99), pct(&h2, 0.99));
    eprintln!(
        "land height range v3 {range3:.0} m v2 {range2:.0} m (x{:.2}); p99 above sea v3 {q3:.0} v2 {q2:.0} (x{:.2}); observed max v3 {top3:.0} v2 {top2:.0}; max_relief v3 {:.0}",
        range3 / range2,
        q3 / q2,
        p3.max_relief()
    );
    assert!(range3 >= 1.5 * range2, "range {range3} vs v2 {range2}");
    assert!(q3 >= 1.5 * q2, "p99 {q3} vs v2 {q2}");
    let mr = p3.max_relief();
    assert!((9000.0..=11_000.0).contains(&mr), "max_relief {mr}");
    assert!(top3 <= mr, "observed {top3} > max_relief {mr}");
}

#[test]
fn v3_is_not_pure_hills_everywhere() {
    let v = cube_slopes(TerrainParams::demo_v3(), 8);
    let plains = v.iter().filter(|a| **a < 1.0).count() as f64 / v.len() as f64;
    eprintln!("plains share (slope < 1 deg): {plains:.3}");
    assert!(
        (0.20..=0.60).contains(&plains),
        "plains share {plains:.3} outside 0.20..=0.60"
    );
}
