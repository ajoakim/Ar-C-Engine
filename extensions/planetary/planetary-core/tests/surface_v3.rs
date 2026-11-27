//! Surface attribute for generator v3: the rock ramp uses `ROCK_SLOPE_LO_V3` / `ROCK_SLOPE_HI_V3`
//! (v3 terrain is far steeper than v2, whose thresholds would saturate every slope), while v2
//! meshes keep the v2 thresholds.

use engine_sdk::DVec3;
use engine_sdk::procedural::splitmix64;
use planetary_core::hex::mapping::{locate as hex_locate, sample_dir};
use planetary_core::hex::mesh::{build_hex_cell, vertex_count_hex};
use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
use planetary_core::{
    Face, HexSphere, PatchId, PlanetTerrain, ROCK_SLOPE_HI_V3, ROCK_SLOPE_LO_V3, TerrainParams,
    Topology, build_patch, ramp_row, ramp_value, ramp_value_v3, vertex_dir,
};

const SEED: u64 = 0x0007_5EED_2026_0013;
const N: u32 = 32;
const NH: u32 = 16;

fn unit(v: [f32; 3]) -> DVec3 {
    DVec3::new(f64::from(v[0]), f64::from(v[1]), f64::from(v[2]))
}

fn seeded_patch(level: u8, k: u64) -> PatchId {
    let z = splitmix64(k.wrapping_add(u64::from(level) << 32));
    let face = Face::ALL[(z % 6) as usize];
    let n = 1u64 << level;
    let x = ((z >> 8) % n) as u32;
    let y = ((z >> 32) % n) as u32;
    PatchId::new(face, level, x, y).expect("id")
}

fn unit_f(k: u64) -> f64 {
    (splitmix64(k) >> 11) as f64 / (1u64 << 53) as f64
}

fn dir(k: u64) -> DVec3 {
    let mut i = 0u64;
    loop {
        let base = k.wrapping_mul(0x9E37_79B9).wrapping_add(i.wrapping_mul(3));
        let p = DVec3::new(
            unit_f(base) * 2.0 - 1.0,
            unit_f(base + 1_000_003) * 2.0 - 1.0,
            unit_f(base + 2_000_003) * 2.0 - 1.0,
        );
        let l2 = p.x * p.x + p.y * p.y + p.z * p.z;
        if l2 > 0.01 && l2 <= 1.0 {
            let l = l2.sqrt();
            return DVec3::new(p.x / l, p.y / l, p.z / l);
        }
        i += 1;
    }
}

fn tilted(slope: f64) -> DVec3 {
    let c = 1.0 - slope;
    DVec3::new((1.0 - c * c).sqrt(), 0.0, c)
}

#[test]
fn ramp_v3_formula_endpoints() {
    let d = DVec3::new(0.0, 0.0, 1.0);
    let mid = 0.5 * (ROCK_SLOPE_LO_V3 + ROCK_SLOPE_HI_V3);
    for mask in [0.0, 0.3, 1.0] {
        let dry = 0.4 * (1.0 - mask);
        let at = |slope: f64| ramp_value_v3(mask, tilted(slope), d);
        // Rock term 0 at LO and below, 1 at HI and above, linear in between.
        assert!((at(ROCK_SLOPE_LO_V3) - dry).abs() < 1e-9, "mask {mask}");
        assert!((at(0.0) - dry).abs() < 1e-12);
        assert!((at(ROCK_SLOPE_HI_V3) - (dry + 0.6)).abs() < 1e-9);
        assert!((at(2.0 * ROCK_SLOPE_HI_V3) - (dry + 0.6)).abs() < 1e-12);
        assert!((at(mid) - (dry + 0.3)).abs() < 1e-9);
    }
    // Flat vertex: mask term only.
    assert_eq!(ramp_value_v3(1.0, d, d), 0.0);
    assert!((ramp_value_v3(0.0, d, d) - 0.4).abs() < 1e-15);
    // The v3 thresholds are not the v2 ones: a slope that saturates v2 is still dry for v3.
    let slope = 1.0e-3;
    assert!(ramp_value(1.0, tilted(slope), d) > 0.59);
    assert!(ramp_value_v3(1.0, tilted(slope), d) < 1e-9);
}

/// `rock_t` of every land grid vertex of real meshes, from the stored normals.
fn land_rock_t_cube(level: u8, count: u64) -> Vec<f64> {
    let t = PlanetTerrain::new(SEED, TerrainParams::demo_v3(), N).expect("terrain");
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let mut out = Vec::new();
    for k in 0..count {
        let id = seeded_patch(level, 100 + k);
        let mesh = build_patch(&t, &m, id);
        for j in 0..=N {
            for i in 0..=N {
                let d = vertex_dir(id, N, i, j);
                if t.height_at_dir(d) <= t.params().sea_level {
                    continue;
                }
                let v = mesh.vertices[(j * (N + 1) + i) as usize];
                out.push(rock_t(1.0 - unit(v.normal).dot(d)));
            }
        }
    }
    out
}

fn land_rock_t_hex(level: u8, count: u64) -> Vec<f64> {
    let t = PlanetTerrain::new(SEED, TerrainParams::demo_v3(), NH).expect("terrain");
    let m = LodModel::for_topology(
        &t,
        DEFAULT_KAPPA,
        HexSphere::new(NH).expect("hex").sample_angle0(),
    );
    let per = ((NH + 1) * (NH + 2) / 2) as usize;
    let mut out = Vec::new();
    for k in 0..count {
        let c = hex_locate(dir(300 + k), level);
        let mesh = build_hex_cell(c, &t, &m, false, &|| false).expect("mesh");
        let sectors = if mesh.vertices.len() == vertex_count_hex(NH) as usize {
            6
        } else {
            5
        };
        let frame = planetary_core::hex::CellFrame::new(c, NH);
        for sec in 0..sectors {
            let mut idx = sec * per;
            for s in 0..=NH {
                for tt in 0..=(NH - s) {
                    let d = sample_dir(frame.sample(sec as u8, s, tt));
                    if t.height_at_dir(d) > t.params().sea_level {
                        out.push(rock_t(1.0 - unit(mesh.vertices[idx].normal).dot(d)));
                    }
                    idx += 1;
                }
            }
        }
    }
    out
}

fn rock_t(slope: f64) -> f64 {
    ((slope - ROCK_SLOPE_LO_V3) / (ROCK_SLOPE_HI_V3 - ROCK_SLOPE_LO_V3)).clamp(0.0, 1.0)
}

#[test]
fn v3_rock_ramp_is_neither_inert_nor_saturated() {
    for (name, v) in [
        ("cube L8", land_rock_t_cube(8, 120)),
        ("cube L10", land_rock_t_cube(10, 120)),
        ("hex L8", land_rock_t_hex(8, 120)),
    ] {
        assert!(v.len() > 10_000, "{name}: {}", v.len());
        let frac =
            |f: &dyn Fn(f64) -> bool| v.iter().filter(|x| f(**x)).count() as f64 / v.len() as f64;
        let dry = frac(&|x| x == 0.0);
        let full = frac(&|x| x == 1.0);
        let mid = frac(&|x| x > 0.0 && x < 1.0);
        eprintln!(
            "{name}: n {} dry {dry:.3} full rock {full:.3} partial {mid:.3}",
            v.len()
        );
        assert!(dry >= 0.20, "{name}: dry share {dry}");
        assert!(full <= 0.50, "{name}: full-rock share {full}");
        assert!(mid >= 0.05, "{name}: partial share {mid}");
    }
}

#[test]
fn v2_meshes_still_use_the_v2_thresholds() {
    let t = PlanetTerrain::new(SEED, TerrainParams::demo_v2(), 8).expect("terrain");
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let mut checked = 0;
    let mut v2_differs_from_v3 = 0;
    for k in 0..12 {
        let id = seeded_patch(9, k);
        let mesh = build_patch(&t, &m, id);
        for j in 0..=8u32 {
            for i in 0..=8u32 {
                let d = vertex_dir(id, 8, i, j);
                let v = mesh.vertices[(j * 9 + i) as usize];
                let mask = f64::from(t.sample(d).mask.clamp(0.0, 1.0) as f32);
                let want = ramp_row(ramp_value(mask, unit(v.normal), d));
                let slope = 1.0 - unit(v.normal).dot(d);
                // f32 normal storage moves `slope` by <= 2e-7, i.e. a row by <= 1.2e-3 on the
                // narrow v2 ramp; away from the ramp the match is f32-exact.
                let tol = if slope < 5e-6 || slope > 2e-4 {
                    2e-5
                } else {
                    2e-3
                };
                assert!(
                    (f64::from(v.uv[1]) - want).abs() < tol,
                    "{id:?} ({i},{j}) {} vs {want}",
                    v.uv[1]
                );
                let v3 = ramp_row(ramp_value_v3(mask, unit(v.normal), d));
                if (v3 - want).abs() > 1e-3 {
                    v2_differs_from_v3 += 1;
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 500);
    assert!(
        v2_differs_from_v3 > 0,
        "the v3 ramp never differs from the v2 ramp on these vertices, so the dispatch is untested"
    );
}
