//! Surface attribute v2: `uv.y` is one dry-to-rock ramp row `(0.5 + 7 q) / 8` with
//! `q = 0.4 * (1 - mask) + 0.6 * rock_t`; v1 meshes keep `uv.y = mask`. Border vertices shared by
//! adjacent cells carry bit-equal attributes (the border normal is face-independent).

use engine_sdk::DVec3;
use engine_sdk::procedural::splitmix64;
use planetary_core::hex::CellId;
use planetary_core::hex::mesh::build_hex_cell;
use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
use planetary_core::{
    Edge, Face, HexSphere, PatchId, PlanetTerrain, ROCK_SLOPE_HI, ROCK_SLOPE_LO, TerrainParams,
    Topology, build_patch, neighbor, ramp_row, ramp_value, vertex_dir,
};

const SEED: u64 = 0x0007_5EED_2026_0003;
const N: u32 = 8;

fn terrain(p: TerrainParams) -> PlanetTerrain {
    PlanetTerrain::new(SEED, p, N).expect("terrain")
}

fn cube_model(t: &PlanetTerrain) -> LodModel {
    LodModel::from_terrain(t, DEFAULT_KAPPA)
}

fn hex_model(t: &PlanetTerrain) -> LodModel {
    LodModel::for_topology(
        t,
        DEFAULT_KAPPA,
        HexSphere::new(N).expect("hex").sample_angle0(),
    )
}

fn unit(v: [f32; 3]) -> DVec3 {
    DVec3::new(f64::from(v[0]), f64::from(v[1]), f64::from(v[2]))
}

fn cube_ids() -> Vec<PatchId> {
    vec![
        PatchId::root(Face::PosX),
        PatchId::new(Face::PosY, 3, 7, 3).unwrap(),
        PatchId::new(Face::PosZ, 2, 0, 0).unwrap(),
        PatchId::new(Face::NegX, 9, 123, 201).unwrap(),
    ]
}

#[test]
fn ramp_formula_endpoints() {
    let d = DVec3::new(0.0, 0.0, 1.0);
    // Flat (normal == direction): slope 0, rock_t 0, q = 0.4 (1 - mask).
    assert_eq!(ramp_value(1.0, d, d), 0.0);
    assert!((ramp_value(0.0, d, d) - 0.4).abs() < 1e-15);
    // A steep vertex: normal tilted so that slope = 2 * ROCK_SLOPE_HI.
    let slope = 2.0 * ROCK_SLOPE_HI;
    let c = 1.0 - slope;
    let s = (1.0 - c * c).sqrt();
    let steep = DVec3::new(s, 0.0, c);
    assert!(ROCK_SLOPE_HI < slope);
    for mask in [0.0, 0.3, 0.9] {
        let q = ramp_value(mask, steep, d);
        assert!(q > 0.6, "mask {mask} q {q}");
    }
    assert!((ramp_value(0.0, steep, d) - 1.0).abs() < 1e-12);
    // The rock threshold is exactly where rock_t starts.
    let c = 1.0 - ROCK_SLOPE_LO;
    let s = (1.0 - c * c).sqrt();
    assert!((ramp_value(1.0, DVec3::new(s, 0.0, c), d)).abs() < 1e-9);
    // Row mapping: texel centres of 8 rows.
    assert_eq!(ramp_row(0.0), 0.5 / 8.0);
    assert_eq!(ramp_row(1.0), 7.5 / 8.0);
}

#[test]
fn v1_meshes_keep_the_mask_in_uv_y() {
    let t = terrain(TerrainParams::default());
    let m = cube_model(&t);
    for id in cube_ids() {
        let mesh = build_patch(&t, &m, id);
        for j in 0..=N {
            for i in 0..=N {
                let s = t.sample(vertex_dir(id, N, i, j));
                let uv = mesh.vertices[(j * (N + 1) + i) as usize].uv;
                assert_eq!(uv[1], s.mask.clamp(0.0, 1.0) as f32);
            }
        }
    }
}

#[test]
fn cube_v2_uv_y_is_the_ramp_row() {
    let t = terrain(TerrainParams::demo_v2());
    let m = cube_model(&t);
    let w = N + 1;
    let (lo, hi) = (0.5 / 8.0 - 1e-6, 7.5 / 8.0 + 1e-6);
    for id in cube_ids() {
        let mesh = build_patch(&t, &m, id);
        for j in 0..=N {
            for i in 0..=N {
                let k = (j * w + i) as usize;
                let d = vertex_dir(id, N, i, j);
                let v = mesh.vertices[k];
                let mask = f64::from(t.sample(d).mask.clamp(0.0, 1.0) as f32);
                let want = ramp_row(ramp_value(mask, unit(v.normal), d));
                // The vertex normal is stored as f32 (component error <= 6e-8), which moves
                // `slope = 1 - n.d` by <= 2e-7; over the ramp width that is a row error of
                // 7/8 * 0.6 * 2e-7 / (HI - LO) = 1.2e-3. Away from the rock threshold the
                // dry part of the ramp (rock_t = 0) must match to f32 precision.
                let slope = 1.0 - unit(v.normal).dot(d);
                let tol = if slope < ROCK_SLOPE_LO - 1e-6 {
                    2e-5
                } else {
                    2e-3
                };
                assert!(
                    (f64::from(v.uv[1]) - want).abs() < tol,
                    "{id:?} ({i},{j}): {} vs {want} (slope {slope:e})",
                    v.uv[1]
                );
                assert!(f64::from(v.uv[1]) >= f64::from(lo as f32) - 1e-6);
                assert!(f64::from(v.uv[1]) <= hi);
                assert!((0.0..=1.0).contains(&v.uv[0]));
            }
        }
        // Skirt vertices copy their border vertex attribute.
        let mut s = (w * w) as usize;
        for e in Edge::ALL {
            for tt in 0..w {
                let (i, j) = e.vertex(N, tt);
                let b = mesh.vertices[(j * w + i) as usize];
                assert_eq!(mesh.vertices[s].uv, b.uv);
                s += 1;
            }
        }
    }
}

#[test]
fn flat_ocean_vertices_are_dry_end_of_the_ramp() {
    // Vertices at sea level with a (near) radial normal have rock_t 0, so q <= 0.4.
    let t = terrain(TerrainParams::demo_v2());
    let m = cube_model(&t);
    let sea = t.params().sea_level;
    let mut seen = 0;
    for id in cube_ids()
        .into_iter()
        .chain(PatchId::level_iter(2).take(12))
    {
        let mesh = build_patch(&t, &m, id);
        for j in 0..=N {
            for i in 0..=N {
                let d = vertex_dir(id, N, i, j);
                if t.height_at_dir(d) > sea {
                    continue;
                }
                let v = mesh.vertices[(j * (N + 1) + i) as usize];
                let q = ramp_value(t.sample(d).mask, unit(v.normal), d);
                if q <= 0.4 + 1e-5 {
                    seen += 1;
                }
                // Even at the coast the ramp never reaches the rock half of the palette for a
                // vertex lying exactly at sea level on a gentle normal.
                let slope = 1.0 - (unit(v.normal).dot(d));
                if slope < ROCK_SLOPE_LO {
                    assert!(q <= 0.4 + 1e-5, "{id:?} q {q}");
                }
            }
        }
    }
    assert!(seen > 0, "no ocean vertices sampled");
}

#[test]
fn cube_v2_shared_borders_have_bit_equal_uv() {
    let t = terrain(TerrainParams::demo_v2());
    let m = cube_model(&t);
    let w = N + 1;
    let mut checked = 0;
    for level in 0..=2u8 {
        for p in PatchId::level_iter(level) {
            let a = build_patch(&t, &m, p);
            for e in Edge::ALL {
                let nb = neighbor(p, e);
                let b = build_patch(&t, &m, nb.patch);
                for s in 0..=N {
                    let (i, j) = Edge::vertex(e, N, s);
                    let s2 = if nb.flipped { N - s } else { s };
                    let (i2, j2) = Edge::vertex(nb.edge, N, s2);
                    let ua = a.vertices[(j * w + i) as usize].uv;
                    let ub = b.vertices[(j2 * w + i2) as usize].uv;
                    assert_eq!(
                        [ua[0].to_bits(), ua[1].to_bits()],
                        [ub[0].to_bits(), ub[1].to_bits()],
                        "{p:?} {e:?} s{s}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 0);
}

/// Grid index of `(sector, s, t)` (the hex mesh vertex layout).
fn gi(n: u32, k: u32, s: u32, t: u32) -> usize {
    let per = (n + 1) * (n + 2) / 2;
    (k * per + s * (n + 1) - s * s.saturating_sub(1) / 2 + t) as usize
}

#[test]
fn hex_v2_shared_borders_have_bit_equal_uv() {
    let t = terrain(TerrainParams::demo_v2());
    let m = hex_model(&t);
    let mut checked = 0;
    for level in 0..=2u8 {
        for c in CellId::level_iter(level) {
            let ring_c = planetary_core::hex::mapping::ring(c);
            let a = build_hex_cell(c, &t, &m, false, &|| false).expect("mesh");
            for (k, x) in ring_c.iter().enumerate() {
                let ring_x = planetary_core::hex::mapping::ring(*x);
                let mk = ring_x.iter().position(|y| *y == c).expect("reciprocal");
                let b = build_hex_cell(*x, &t, &m, false, &|| false).expect("mesh");
                for j in 0..=N {
                    let ia = gi(N, k as u32, N - j, j);
                    let jb = N - j;
                    let ib = gi(N, mk as u32, N - jb, jb);
                    let (ua, ub) = (a.vertices[ia].uv, b.vertices[ib].uv);
                    assert_eq!(
                        [ua[0].to_bits(), ua[1].to_bits()],
                        [ub[0].to_bits(), ub[1].to_bits()],
                        "{c:?} sector {k} vs {x:?} at {j}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 0);
}

#[test]
fn seam_negative_control_perturbed_normal_breaks_equality() {
    // The equality asserted above is sensitive to the border normal: tilting one side's normal
    // changes the attribute, so the test would fail for face-dependent normals.
    let d = DVec3::new(0.0, 0.0, 1.0);
    let n_a = d;
    let n_b = DVec3::new(0.6, 0.0, 0.8);
    let ra = ramp_row(ramp_value(0.5, n_a, d)) as f32;
    let rb = ramp_row(ramp_value(0.5, n_b, d)) as f32;
    assert_ne!(ra.to_bits(), rb.to_bits());
}

fn seeded_patch(level: u8, k: u64) -> PatchId {
    let z = splitmix64(k.wrapping_add(u64::from(level) << 32));
    let face = Face::ALL[(z % 6) as usize];
    let n = 1u64 << level;
    let x = ((z >> 8) % n) as u32;
    let y = ((z >> 32) % n) as u32;
    PatchId::new(face, level, x, y).expect("id")
}

/// Land-vertex slopes (`1 - n.d`) of `count` seeded patches at `level`.
fn land_slopes(t: &PlanetTerrain, m: &LodModel, level: u8, count: u64) -> Vec<f64> {
    let mut out = Vec::new();
    for k in 0..count {
        let id = seeded_patch(level, k);
        let mesh = build_patch(t, m, id);
        for j in 0..=N {
            for i in 0..=N {
                let d = vertex_dir(id, N, i, j);
                if t.height_at_dir(d) <= t.params().sea_level {
                    continue;
                }
                let v = mesh.vertices[(j * (N + 1) + i) as usize];
                out.push(1.0 - unit(v.normal).dot(d));
            }
        }
    }
    out.sort_by(|a, b| a.total_cmp(b));
    out
}

#[test]
fn measure_real_slopes() {
    // Informational: the land slope distribution (`1 - n.d`, quadratic in the inclination angle)
    // on real v2 terrain by level. Used to calibrate ROCK_SLOPE_LO / ROCK_SLOPE_HI.
    let t = terrain(TerrainParams::demo_v2());
    let m = cube_model(&t);
    for level in [4u8, 8, 12, 16, 18, 20] {
        let v = land_slopes(&t, &m, level, 400);
        if v.is_empty() {
            continue;
        }
        let q = |f: f64| v[((v.len() - 1) as f64 * f) as usize];
        eprintln!(
            "level {level}: n {} p50 {:.2e} p90 {:.2e} p99 {:.2e} max {:.2e}",
            v.len(),
            q(0.5),
            q(0.9),
            q(0.99),
            v[v.len() - 1]
        );
    }
}

#[test]
fn rock_ramp_is_not_inert_on_real_land() {
    // Guard: with the calibrated thresholds a real share of fine-level land vertices is on the
    // rock half of the ramp, some are fully rock, and most are not (rock stays a minority).
    let t = terrain(TerrainParams::demo_v2());
    let m = cube_model(&t);
    let v = land_slopes(&t, &m, 12, 200);
    assert!(v.len() > 10_000);
    let partial = v.iter().filter(|s| **s > ROCK_SLOPE_LO).count();
    let full = v.iter().filter(|s| **s >= ROCK_SLOPE_HI).count();
    let frac = |c: usize| c as f64 / v.len() as f64;
    assert!(frac(partial) > 0.2, "rock starts on {:.3}", frac(partial));
    assert!(frac(full) > 0.005, "full rock on {:.4}", frac(full));
    assert!(frac(full) < 0.2, "full rock on {:.4}", frac(full));
}
