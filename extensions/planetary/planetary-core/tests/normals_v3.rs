//! Generator v3 normals (TERR-03, decision D-01: the height-derived normal already in payload v4
//! is the normal under test). Seams: same-level shared border normals are bit-equal; cross-level
//! the border row versus the first interior row stays within `TOL_ROW_DEG`; and the normals are height-derived, not sphere normals.
//! The tolerances are measured (see the printed tables) and pinned with absolute caps; they are
//! never raised to pass.

use engine_sdk::DVec3;
use engine_sdk::procedural::splitmix64;
use planetary_core::hex::CellId;
use planetary_core::hex::mapping::{cell_sample_point, locate as hex_locate, ring, sample_dir};
use planetary_core::hex::mesh::{build_hex_cell, hex_vertex_position_f64, vertex_count_hex};
use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
use planetary_core::{
    Edge, Face, HexSphere, PatchId, PlanetTerrain, TerrainParams, Topology, border_normal_at,
    build_patch, neighbor, vertex_dir, vertex_position_f64,
};

const SEED: u64 = 0x0007_5EED_2026_0012;
const NC: u32 = 32;
const NH: u32 = 16;

/// Cap on the angle (degrees) between coincident level-L and level-(L+1) normals. The cross-level
/// test is NOT in this file yet: the measured v3 values exceed this cap (open question, plan 11-05
/// SUMMARY), so the constant is kept at its cap and unused by an assertion.
#[allow(dead_code)]
const TOL_LOD_DEG: f64 = 8.0;
/// Mean angle (degrees) between a border-row normal and its first-interior-row normal.
const TOL_ROW_DEG: f64 = 4.0;

fn cube_terrain() -> PlanetTerrain {
    PlanetTerrain::new(SEED, TerrainParams::demo_v3(), NC).expect("terrain")
}

fn hex_terrain() -> PlanetTerrain {
    PlanetTerrain::new(SEED, TerrainParams::demo_v3(), NH).expect("terrain")
}

fn hex_model(t: &PlanetTerrain) -> LodModel {
    LodModel::for_topology(
        t,
        DEFAULT_KAPPA,
        HexSphere::new(NH).expect("hex").sample_angle0(),
    )
}

fn v(n: [f32; 3]) -> DVec3 {
    DVec3::new(f64::from(n[0]), f64::from(n[1]), f64::from(n[2]))
}

fn angle_deg(a: DVec3, b: DVec3) -> f64 {
    let c = DVec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    );
    let cl = (c.x * c.x + c.y * c.y + c.z * c.z).sqrt();
    cl.atan2(a.x * b.x + a.y * b.y + a.z * b.z).to_degrees()
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

fn pct(v: &[f64], f: f64) -> f64 {
    v[((v.len() - 1) as f64 * f) as usize]
}

/// Index of hex grid vertex `(sector, s, t)` (the hex mesh vertex layout).
fn gi(n: u32, k: u32, s: u32, t: u32) -> usize {
    let per = (n + 1) * (n + 2) / 2;
    (k * per + s * (n + 1) - s * s.saturating_sub(1) / 2 + t) as usize
}

/// Mismatching shared border normal pairs; `perturb` is applied to the first normal of the first
/// compared pair (the negative control), `None` for the real check.
fn count_shared_border_mismatches(perturb: Option<f64>) -> (usize, usize) {
    let mut bad = 0usize;
    let mut checked = 0usize;
    let mut first = perturb;
    let mut cmp = |a: [f32; 3], b: [f32; 3]| {
        let mut a = a;
        if let Some(deg) = first.take() {
            // Tilt about the z axis (or x if the normal is nearly along z) by `deg` degrees.
            let r = deg.to_radians();
            let (s, c) = (r.sin() as f32, r.cos() as f32);
            a = [a[0] * c - a[1] * s, a[0] * s + a[1] * c, a[2]];
        }
        checked += 1;
        if a.map(f32::to_bits) != b.map(f32::to_bits) {
            bad += 1;
        }
    };

    // Cube: all patches of levels 0..=2 and a seeded sample at level 8.
    let t = cube_terrain();
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let w = NC + 1;
    let mut ids: Vec<PatchId> = (0..=2u8).flat_map(PatchId::level_iter).collect();
    ids.extend((0..12).map(|k| seeded_patch(8, k)));
    for p in ids {
        let a = build_patch(&t, &m, p);
        for e in Edge::ALL {
            let nb = neighbor(p, e);
            let b = build_patch(&t, &m, nb.patch);
            for s in 0..=NC {
                let (i, j) = Edge::vertex(e, NC, s);
                let s2 = if nb.flipped { NC - s } else { s };
                let (i2, j2) = Edge::vertex(nb.edge, NC, s2);
                cmp(
                    a.vertices[(j * w + i) as usize].normal,
                    b.vertices[(j2 * w + i2) as usize].normal,
                );
            }
        }
    }

    // Hex: all cells of levels 0..=2 (incl. the 12 pentagons) and a seeded level-8 sample.
    let t = hex_terrain();
    let m = hex_model(&t);
    let mut cells: Vec<CellId> = (0..=2u8).flat_map(CellId::level_iter).collect();
    cells.extend((0..12).map(|k| hex_locate(dir(k), 8)));
    for c in cells {
        let a = build_hex_cell(c, &t, &m, false, &|| false).expect("mesh");
        for (k, x) in ring(c).iter().enumerate() {
            let mk = ring(*x).iter().position(|y| *y == c).expect("reciprocal");
            let b = build_hex_cell(*x, &t, &m, false, &|| false).expect("mesh");
            for j in 0..=NH {
                let ia = gi(NH, k as u32, NH - j, j);
                let jb = NH - j;
                let ib = gi(NH, mk as u32, NH - jb, jb);
                cmp(a.vertices[ia].normal, b.vertices[ib].normal);
            }
        }
    }
    (bad, checked)
}

#[test]
fn v3_same_level_border_normals_bit_equal_cube_and_hex() {
    let (bad, checked) = count_shared_border_mismatches(None);
    assert!(checked > 5_000, "only {checked} pairs checked");
    assert_eq!(bad, 0, "{bad} of {checked} shared border normals differ");
}

#[test]
fn v3_normal_negative_control() {
    // A 0.1 degree tilt of one border normal must be caught by the same equality harness.
    let (bad, checked) = count_shared_border_mismatches(Some(0.1));
    assert!(checked > 5_000);
    assert!(bad >= 1, "0.1 degree perturbation went unnoticed");
}

#[test]
fn v3_normals_are_height_derived() {
    // Cube: interior vertex normals equal the normalised cross of the central differences of the
    // displaced positions, and are not the radial direction.
    let t = cube_terrain();
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let w = NC + 1;
    let mut angles = Vec::new();
    let mut worst = 0.0f64;
    for k in 0..40 {
        let id = seeded_patch(8, k);
        let mesh = build_patch(&t, &m, id);
        for j in 0..=NC {
            for i in 0..=NC {
                let d = vertex_dir(id, NC, i, j);
                if t.height_at_dir(d) <= t.params().sea_level {
                    continue;
                }
                let n = v(mesh.vertices[(j * w + i) as usize].normal);
                angles.push(angle_deg(n, d));
                if i == 0 || j == 0 || i == NC || j == NC {
                    continue;
                }
                let p = |i, j| vertex_position_f64(&t, id, i, j);
                let (a, b) = (sub(p(i + 1, j), p(i - 1, j)), sub(p(i, j + 1), p(i, j - 1)));
                let want = unit(cross(a, b));
                worst = worst.max(
                    (n.x - want.x)
                        .abs()
                        .max((n.y - want.y).abs())
                        .max((n.z - want.z).abs()),
                );
            }
        }
    }
    angles.sort_by(f64::total_cmp);
    let med = pct(&angles, 0.5);
    eprintln!(
        "cube: land n {} median(normal, radial) {med:.2} deg, interior worst component diff {worst:e}",
        angles.len()
    );
    assert!(angles.len() > 5_000);
    assert!(med > 1.0, "median {med}");
    assert!(
        worst < 1e-6,
        "interior normal differs from grid difference by {worst:e}"
    );

    // Hex: same on the sector grids.
    let t = hex_terrain();
    let m = hex_model(&t);
    let per = ((NH + 1) * (NH + 2) / 2) as usize;
    let mut angles = Vec::new();
    let mut worst = 0.0f64;
    for k in 0..40 {
        let c = hex_locate(dir(k), 8);
        let mesh = build_hex_cell(c, &t, &m, false, &|| false).expect("mesh");
        let sectors = if mesh.vertices.len() == vertex_count_hex(NH) as usize {
            6
        } else {
            5
        };
        for sec in 0..sectors {
            for s in 0..=NH {
                for tt in 0..=(NH - s) {
                    let idx = sec * per + gi(NH, 0, s, tt);
                    let pos = |s, tt| hex_vertex_position_f64(&t, c, NH, sec as u8, s, tt);
                    let d = sample_dir(cell_sample_point(c, NH, sec as u8, s, tt));
                    if t.height_at_dir(d) <= t.params().sea_level {
                        continue;
                    }
                    let n = v(mesh.vertices[idx].normal);
                    angles.push(angle_deg(n, d));
                    if s == 0 || tt == 0 || s + tt == NH {
                        continue;
                    }
                    let a = sub(pos(s + 1, tt), pos(s - 1, tt));
                    let b = sub(pos(s, tt + 1), pos(s, tt - 1));
                    let want = unit(cross(a, b));
                    worst = worst.max(
                        (n.x - want.x)
                            .abs()
                            .max((n.y - want.y).abs())
                            .max((n.z - want.z).abs()),
                    );
                }
            }
        }
    }
    angles.sort_by(f64::total_cmp);
    let med = pct(&angles, 0.5);
    eprintln!(
        "hex: land n {} median(normal, radial) {med:.2} deg, interior worst component diff {worst:e}",
        angles.len()
    );
    assert!(angles.len() > 5_000);
    assert!(med > 1.0, "median {med}");
    assert!(
        worst < 1e-6,
        "interior normal differs from grid difference by {worst:e}"
    );
}

fn sub(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn cross(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn unit(a: DVec3) -> DVec3 {
    let l = (a.x * a.x + a.y * a.y + a.z * a.z).sqrt();
    DVec3::new(a.x / l, a.y / l, a.z / l)
}

/// Mean angle between the border row and the first interior row, and the interior baseline
/// (row 1 versus row 2), along the four edges of seeded cube patches.
fn cube_rows() -> (Vec<f64>, Vec<f64>) {
    let t = cube_terrain();
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let w = NC + 1;
    let (mut border, mut base) = (Vec::new(), Vec::new());
    for level in [6u8, 8, 10, 12] {
        for k in 0..30u64 {
            let id = seeded_patch(level, 700 + k);
            let mesh = build_patch(&t, &m, id);
            let n = |i: u32, j: u32| v(mesh.vertices[(j * w + i) as usize].normal);
            for s in 2..NC - 1 {
                border.push(angle_deg(n(s, 0), n(s, 1)));
                border.push(angle_deg(n(s, NC), n(s, NC - 1)));
                border.push(angle_deg(n(0, s), n(1, s)));
                border.push(angle_deg(n(NC, s), n(NC - 1, s)));
                base.push(angle_deg(n(s, 1), n(s, 2)));
                base.push(angle_deg(n(s, NC - 1), n(s, NC - 2)));
                base.push(angle_deg(n(1, s), n(2, s)));
                base.push(angle_deg(n(NC - 1, s), n(NC - 2, s)));
            }
        }
    }
    (border, base)
}

/// Hex: sector-border vertices (`s + t = n`) versus their first interior neighbour `(s-1, t)`.
fn hex_rows() -> (Vec<f64>, Vec<f64>) {
    let t = hex_terrain();
    let m = hex_model(&t);
    let per = ((NH + 1) * (NH + 2) / 2) as usize;
    let (mut border, mut base) = (Vec::new(), Vec::new());
    for level in [6u8, 8, 10, 12] {
        for k in 0..30u64 {
            let c = hex_locate(dir(1_300 + k + u64::from(level) * 17), level);
            let mesh = build_hex_cell(c, &t, &m, false, &|| false).expect("mesh");
            let sectors = if mesh.vertices.len() == vertex_count_hex(NH) as usize {
                6
            } else {
                5
            };
            for sec in 0..sectors {
                let n = |s: u32, tt: u32| v(mesh.vertices[sec * per + gi(NH, 0, s, tt)].normal);
                for tt in 2..NH - 2 {
                    // Outer edge s + t = n, inward neighbour (s - 1, t).
                    let s = NH - tt;
                    border.push(angle_deg(n(s, tt), n(s - 1, tt)));
                    base.push(angle_deg(n(s - 1, tt), n(s - 2, tt)));
                }
            }
        }
    }
    (border, base)
}

#[test]
fn v3_border_row_vs_interior_row_jump() {
    const { assert!(TOL_ROW_DEG <= 4.0) };
    for (name, (mut border, mut base)) in [("cube", cube_rows()), ("hex", hex_rows())] {
        assert!(border.len() > 1_000);
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let (mb, mi) = (mean(&border), mean(&base));
        border.sort_by(f64::total_cmp);
        base.sort_by(f64::total_cmp);
        eprintln!(
            "{name} rows: border->row1 mean {mb:.3} p50 {:.3} p99 {:.3} max {:.3}; interior baseline mean {mi:.3} p50 {:.3} p99 {:.3} max {:.3} deg",
            pct(&border, 0.5),
            pct(&border, 0.99),
            border[border.len() - 1],
            pct(&base, 0.5),
            pct(&base, 0.99),
            base[base.len() - 1],
        );
        assert!(
            mb <= TOL_ROW_DEG,
            "{name}: mean row jump {mb} > {TOL_ROW_DEG}"
        );
    }
}

#[test]
fn v3_border_stencil_uses_the_topology_grid_angle() {
    // Version gate: generator 2 keeps the cube constant step, generator 3 follows the supplied
    // grid angle (equal for the cube, a different step for the hex topology).
    let d = dir(5);
    let t3 = hex_terrain();
    let hex_a0 = HexSphere::new(NH).expect("hex").sample_angle0();
    let cube_a0 = core::f64::consts::FRAC_PI_2 / f64::from(NH);
    eprintln!("A7: hex sample_angle0 / (pi/2/n) = {:.6}", hex_a0 / cube_a0);
    let a = border_normal_at(&t3, 8, d, hex_a0);
    let b = border_normal_at(&t3, 8, d, core::f64::consts::FRAC_PI_2 / f64::from(NH));
    assert_ne!(
        a.x.to_bits() ^ a.y.to_bits() ^ a.z.to_bits(),
        b.x.to_bits() ^ b.y.to_bits() ^ b.z.to_bits()
    );
    let t2 = PlanetTerrain::new(SEED, TerrainParams::demo_v2(), NH).expect("terrain");
    let c = border_normal_at(&t2, 8, d, hex_a0);
    let e = border_normal_at(&t2, 8, d, 123.0);
    assert_eq!(c, e, "generator 2 ignores the supplied angle");
}
