//! Mapping tests: exact-op T(s), shared-border bit equality, level independence, locate.

use planetary_core::{
    Edge, Face, PatchId, equal_angle_t, lattice_dir, locate, neighbor, patch_center_dir,
    patch_corner_dirs, vertex_dir,
};

fn bits(d: engine_sdk::DVec3) -> [u64; 3] {
    [d.x.to_bits(), d.y.to_bits(), d.z.to_bits()]
}

#[test]
fn t_endpoints_exact() {
    assert_eq!(equal_angle_t(1.0).to_bits(), 1.0f64.to_bits());
    assert_eq!(equal_angle_t(-1.0).to_bits(), (-1.0f64).to_bits());
    assert_eq!(equal_angle_t(0.0), 0.0);
}

#[test]
fn t_is_odd() {
    for k in 0..=4096i32 {
        let s = f64::from(k) / 4096.0;
        assert_eq!(
            equal_angle_t(-s).to_bits(),
            (-equal_angle_t(s)).to_bits(),
            "s={s}"
        );
    }
}

#[test]
fn t_is_monotone() {
    let mut prev = equal_angle_t(-1.0);
    for k in -65535..=65536i32 {
        let cur = equal_angle_t(f64::from(k) / 65536.0);
        assert!(cur > prev, "not increasing at k={k}");
        prev = cur;
    }
}

#[test]
fn t_matches_libm_tan() {
    for k in -65536..=65536i32 {
        let s = f64::from(k) / 65536.0;
        let want = (core::f64::consts::FRAC_PI_4 * s).tan();
        assert!((equal_angle_t(s) - want).abs() <= 1e-15, "s={s}");
    }
}

#[test]
fn lattice_dirs_are_unit_and_centers_are_normals() {
    for f in Face::ALL {
        let c = lattice_dir(f, 1, 1, 2);
        let n = f.basis().n;
        assert_eq!(
            [c.x, c.y, c.z],
            [f64::from(n[0]), f64::from(n[1]), f64::from(n[2])]
        );
        for (gu, gv) in [(0, 0), (3, 5), (8, 8), (1, 7), (6, 0)] {
            let d = lattice_dir(f, gu, gv, 8);
            let len2 = d.x * d.x + d.y * d.y + d.z * d.z;
            assert!((len2 - 1.0).abs() <= 4e-16, "len2={len2}");
        }
    }
}

#[test]
fn shared_borders_bit_identical() {
    let n = 8u32;
    for level in 0..=4u8 {
        for p in PatchId::level_iter(level) {
            for e in Edge::ALL {
                let nb = neighbor(p, e);
                for t in 0..=n {
                    let (i, j) = Edge::vertex(e, n, t);
                    let t2 = if nb.flipped { n - t } else { t };
                    let (i2, j2) = Edge::vertex(nb.edge, n, t2);
                    assert_eq!(
                        bits(vertex_dir(p, n, i, j)),
                        bits(vertex_dir(nb.patch, n, i2, j2)),
                        "{p:?} {e:?} t={t}"
                    );
                }
            }
        }
    }
}

#[test]
fn level_independent() {
    let n = 8u32;
    for level in 0..=4u8 {
        for p in PatchId::level_iter(level) {
            let Some(children) = p.children() else {
                continue;
            };
            let (px, py) = p.xy();
            for c in children {
                let (cx, cy) = c.xy();
                let qx = cx - 2 * px;
                let qy = cy - 2 * py;
                for i in (0..=n).step_by(2) {
                    for j in (0..=n).step_by(2) {
                        let pi = qx * (n / 2) + i / 2;
                        let pj = qy * (n / 2) + j / 2;
                        assert_eq!(bits(vertex_dir(c, n, i, j)), bits(vertex_dir(p, n, pi, pj)));
                    }
                }
            }
        }
    }
}

#[test]
fn cube_corners_agree_across_faces() {
    for sx in [-1i32, 1] {
        for sy in [-1i32, 1] {
            for sz in [-1i32, 1] {
                let want = [sx, sy, sz];
                let mut found: Vec<[u64; 3]> = Vec::new();
                for f in Face::ALL {
                    let b = f.basis();
                    for (gu, gv) in [(0u64, 0u64), (1, 0), (0, 1), (1, 1)] {
                        let a = 2 * i32::from(gu != 0) - 1;
                        let c = 2 * i32::from(gv != 0) - 1;
                        let pt = [0, 1, 2].map(|k| b.n[k] + a * b.u[k] + c * b.v[k]);
                        if pt == want {
                            found.push(bits(lattice_dir(f, gu, gv, 1)));
                        }
                    }
                }
                assert_eq!(found.len(), 3, "corner {want:?}");
                assert!(found.iter().all(|x| *x == found[0]), "corner {want:?}");
            }
        }
    }
}

#[test]
fn corner_dirs_match_vertex_dirs() {
    let p = PatchId::new(Face::PosY, 3, 5, 2).unwrap();
    let c = patch_corner_dirs(p);
    assert_eq!(bits(c[0]), bits(vertex_dir(p, 8, 0, 0)));
    assert_eq!(bits(c[3]), bits(vertex_dir(p, 8, 8, 8)));
}

#[test]
fn locate_inverts_center() {
    for level in 0..=5u8 {
        for p in PatchId::level_iter(level) {
            assert_eq!(locate(patch_center_dir(p), level), p);
        }
    }
    let mut state = 0x1234_5678_9ABC_DEF0u64;
    for level in [10u8, 19, 24] {
        for _ in 0..500 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let face = Face::from_index(((state >> 33) % 6) as u8).unwrap();
            let k = 1u64 << level;
            let x = ((state >> 8) % k) as u32;
            let y = ((state >> 20) % k) as u32;
            let p = PatchId::new(face, level, x, y).unwrap();
            assert_eq!(locate(patch_center_dir(p), level), p);
        }
    }
}

#[test]
fn locate_degenerate_inputs() {
    let root = PatchId::root(Face::PosX);
    let nan = engine_sdk::DVec3::new(f64::NAN, 0.0, 0.0);
    assert_eq!(locate(nan, 5), root);
    assert_eq!(locate(engine_sdk::DVec3::ZERO, 5), root);
    let inf = engine_sdk::DVec3::new(f64::INFINITY, 1.0, 0.0);
    assert_eq!(locate(inf, 5), root);
}
