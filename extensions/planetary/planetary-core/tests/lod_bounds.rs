//! Height ranges, residual bound, extruded-footprint distance and culling primitives.

use engine_sdk::DVec3;
use planetary_core::lod::{
    DEFAULT_KAPPA, HeightRange, LodModel, View, cell_distance, frustum_visible, horizon_visible,
    patch_footprint,
};
use planetary_core::{
    CubeSphere, Face, HexSphere, PatchId, PlanetTerrain, TerrainParams, Topology, locate,
    patch_center_dir, vertex_dir,
};

const SEED: u64 = 0x0007_5EED_2026_0002;
const N: u32 = 32;

fn terrain() -> PlanetTerrain {
    PlanetTerrain::new(SEED, TerrainParams::default(), N).expect("terrain")
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn dir(&mut self) -> DVec3 {
        loop {
            let v = DVec3::new(
                self.unit() * 2.0 - 1.0,
                self.unit() * 2.0 - 1.0,
                self.unit() * 2.0 - 1.0,
            );
            let l2 = v.x * v.x + v.y * v.y + v.z * v.z;
            if l2 > 0.01 && l2 <= 1.0 {
                let l = l2.sqrt();
                return DVec3::new(v.x / l, v.y / l, v.z / l);
            }
        }
    }

    fn patch(&mut self, lo: u8, hi: u8) -> PatchId {
        let level = lo + (self.next() % u64::from(hi - lo + 1)) as u8;
        let face = Face::ALL[(self.next() % 6) as usize];
        let n = 1u64 << level;
        let x = (self.next() % n) as u32;
        let y = (self.next() % n) as u32;
        PatchId::new(face, level, x, y).expect("id")
    }
}

fn measured(t: &PlanetTerrain, id: PatchId) -> HeightRange {
    let mut r = HeightRange {
        min: f64::INFINITY,
        max: f64::NEG_INFINITY,
    };
    for j in 0..=N {
        for i in 0..=N {
            let h = t.height_at_dir(vertex_dir(id, N, i, j));
            r.min = r.min.min(h);
            r.max = r.max.max(h);
        }
    }
    r
}

#[test]
fn global_range_contains_samples() {
    let t = terrain();
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let mut rng = Rng(1);
    for _ in 0..100_000 {
        let h = t.height_at_dir(rng.dir());
        assert!(h >= m.height.min && h <= m.height.max, "{h} {:?}", m.height);
    }
}

#[test]
fn residual_bound_contains_fine_samples() {
    let t = terrain();
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let mut rng = Rng(2);
    let fine = 4 * N;
    for _ in 0..300 {
        let id = rng.patch(2, 16);
        let r = measured(&t, id);
        let res = m.residual(id.level());
        for j in (0..=fine).step_by(3) {
            for i in (0..=fine).step_by(3) {
                let h = t.height_at_dir(vertex_dir(id, fine, i, j));
                assert!(
                    h >= r.min - res && h <= r.max + res,
                    "{id:?} h {h} range {r:?} res {res}"
                );
            }
        }
    }
}

#[test]
fn ranges_and_error_are_monotone() {
    let t = terrain();
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    for l in 0..24u8 {
        assert!(m.geometric_error(l + 1) <= m.geometric_error(l));
        assert!(m.residual(l + 1) <= m.residual(l), "level {l}");
    }
    let mut rng = Rng(3);
    for _ in 0..100 {
        let id = rng.patch(1, 15);
        let rp = measured(&t, id);
        let dp = m.derived_range(rp, id.level());
        let widened = dp.widen(m.residual(id.level())).clamp_to(m.height);
        for c in id.children().expect("children") {
            let rc = measured(&t, c);
            let dc = m.derived_range(rc, c.level());
            assert!(
                dc.min >= widened.min - 1e-9 && dc.max <= widened.max + 1e-9,
                "{id:?} child {c:?}: {dc:?} not in {widened:?}"
            );
            assert!(dc.min >= m.height.min && dc.max <= m.height.max);
        }
    }
}

#[test]
fn distance_is_conservative() {
    let r = 6_371_000.0;
    let range = HeightRange {
        min: -200.0,
        max: 3800.0,
    };
    let mut rng = Rng(4);
    for _ in 0..5000 {
        let id = rng.patch(2, 10);
        let fp = patch_footprint(id);
        let scale = [1.0e2, 1.0e4, 1.0e6, 1.0e7][(rng.next() % 4) as usize];
        let e = rng.dir();
        let re = r + range.min - 100.0 + rng.unit() * scale;
        let eye = DVec3::new(e.x * re, e.y * re, e.z * re);
        let d = cell_distance(r, eye, &fp, range);
        assert!(d >= 0.0 && d.is_finite());
        // Sample points of the cell at the extreme and a mid radius.
        for _ in 0..8 {
            let i = (rng.next() % u64::from(N + 1)) as u32;
            let j = (rng.next() % u64::from(N + 1)) as u32;
            let dir = vertex_dir(id, N, i, j);
            for h in [range.min, range.max, 0.5 * (range.min + range.max)] {
                let p = DVec3::new(dir.x * (r + h), dir.y * (r + h), dir.z * (r + h));
                let td = (eye - p).length();
                assert!(d <= td + 1e-6, "{id:?}: bound {d} > true {td}");
            }
        }
    }
    // Straight above the center and inside the extruded footprint.
    let id = locate(DVec3::new(0.3, 0.5, 0.8), 6);
    let fp = patch_footprint(id);
    let c = patch_center_dir(id);
    let above = r + range.max + 1000.0;
    let eye = DVec3::new(c.x * above, c.y * above, c.z * above);
    let d = cell_distance(r, eye, &fp, range);
    assert!((d - 1000.0).abs() < 1e-3, "{d}");
    let inside = r + 100.0;
    let eye = DVec3::new(c.x * inside, c.y * inside, c.z * inside);
    assert_eq!(cell_distance(r, eye, &fp, range), 0.0);
}

#[test]
fn horizon_cases() {
    let r = 6_371_000.0;
    let range = HeightRange {
        min: -200.0,
        max: 3800.0,
    };
    let occ = r + range.min;
    let id = locate(DVec3::new(1.0, 0.2, 0.1), 5);
    let c = patch_center_dir(id);
    let fp = patch_footprint(id);
    for alt in [1.0, 1000.0, 1_000_000.0] {
        let s = r + 2000.0 + alt;
        let eye = DVec3::new(c.x * s, c.y * s, c.z * s);
        assert!(horizon_visible(r, eye, &fp, range, occ), "alt {alt}");
    }
    let anti = locate(DVec3::new(-1.0, -0.2, -0.1), 3);
    let afp = patch_footprint(anti);
    let s = r + 10_000.0;
    let eye = DVec3::new(c.x * s, c.y * s, c.z * s);
    assert!(!horizon_visible(r, eye, &afp, range, occ));
    // The eye below the occluder disables culling.
    let low = occ - 10.0;
    let eye = DVec3::new(c.x * low, c.y * low, c.z * low);
    assert!(horizon_visible(r, eye, &afp, range, occ));
}

fn view(eye: DVec3) -> View {
    View::new(eye, 720, 0.5).expect("view")
}

#[test]
fn frustum_cases() {
    let r = 6_371_000.0;
    let range = HeightRange {
        min: -200.0,
        max: 3800.0,
    };
    let id = locate(DVec3::new(1.0, 0.0, 0.0), 8);
    let fp = patch_footprint(id);
    let b = planetary_core::lod::cell_bound(r, &fp, range);
    let eye = DVec3::new(r + 100_000.0, 0.0, 0.0);
    assert!(frustum_visible(&view(eye), &b));
    // Camera looking along +x (away from the planet): the near plane at the eye culls the cell
    // below it.
    let away = [
        [1.0, 0.0, 0.0, -eye.x],
        [0.0, 1.0, 0.0, 1e12],
        [0.0, -1.0, 0.0, 1e12],
        [0.0, 0.0, 1.0, 1e12],
        [0.0, 0.0, -1.0, 1e12],
        [-1.0, 0.0, 0.0, 1e12],
    ];
    assert!(!frustum_visible(&view(eye).with_frustum(away), &b));
    // Looking along -x: the same cell is in front of the camera.
    let toward = [
        [-1.0, 0.0, 0.0, eye.x],
        [0.0, 1.0, 0.0, 1e12],
        [0.0, -1.0, 0.0, 1e12],
        [0.0, 0.0, 1.0, 1e12],
        [0.0, 0.0, -1.0, 1e12],
        [1.0, 0.0, 0.0, 1e12],
    ];
    assert!(frustum_visible(&view(eye).with_frustum(toward), &b));
    // Non-finite planes are rejected.
    let mut bad = toward;
    bad[2][3] = f64::NAN;
    assert_eq!(view(eye).with_frustum(bad), view(eye));
}

// ---- generator v2 (ridge layer): the selection bounds must include the ridge term ----

fn terrain_v2() -> PlanetTerrain {
    PlanetTerrain::new(SEED, TerrainParams::demo_v2(), N).expect("terrain v2")
}

#[test]
fn v2_global_range_contains_samples() {
    let t = terrain_v2();
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    assert!(m.height.max >= TerrainParams::demo_v2().max_relief() - 1e-9);
    let mut rng = Rng(11);
    let mut top = f64::NEG_INFINITY;
    for _ in 0..20_000 {
        let h = t.height_at_dir(rng.dir());
        top = top.max(h);
        assert!(h >= m.height.min && h <= m.height.max, "{h} {:?}", m.height);
    }
    // The ridge layer lifts terrain above the v1 ceiling somewhere on the sample.
    assert!(top > 0.0);
}

#[test]
fn v2_residual_bound_contains_fine_samples() {
    let t = terrain_v2();
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    let mut rng = Rng(12);
    let fine = 4 * N;
    for _ in 0..300 {
        let id = rng.patch(2, 16);
        let r = measured(&t, id);
        let res = m.residual(id.level());
        for j in (0..=fine).step_by(3) {
            for i in (0..=fine).step_by(3) {
                let h = t.height_at_dir(vertex_dir(id, fine, i, j));
                assert!(
                    h >= r.min - res && h <= r.max + res,
                    "{id:?} h {h} range {r:?} res {res}"
                );
            }
        }
    }
}

#[test]
fn v2_ranges_and_error_are_monotone() {
    let t = terrain_v2();
    let m = LodModel::from_terrain(&t, DEFAULT_KAPPA);
    for l in 0..24u8 {
        assert!(m.residual(l + 1) <= m.residual(l), "level {l}");
    }
    let v1 = LodModel::from_terrain(&terrain(), DEFAULT_KAPPA);
    for l in 0..=24u8 {
        assert!(m.residual(l) > v1.residual(l), "level {l}");
    }
}

fn relief_bound_pairs(m: &LodModel, t: &PlanetTerrain, seed: u64) {
    let mut rng = Rng(seed);
    for level in 1..=12u8 {
        let theta = m.sample_angle0 / f64::from(1u32 << level);
        for _ in 0..2000 {
            let p = rng.dir();
            // A perpendicular unit vector, then rotate p by exactly theta towards it.
            let mut u = rng.dir();
            let dot = u.x * p.x + u.y * p.y + u.z * p.z;
            u = DVec3::new(u.x - dot * p.x, u.y - dot * p.y, u.z - dot * p.z);
            let l = (u.x * u.x + u.y * u.y + u.z * u.z).sqrt();
            if l < 1e-6 {
                continue;
            }
            let (s, c) = theta.sin_cos();
            let q = DVec3::new(
                p.x * c + u.x / l * s,
                p.y * c + u.y / l * s,
                p.z * c + u.z / l * s,
            );
            let dh = (t.height_at_dir(p) - t.height_at_dir(q)).abs();
            assert!(
                dh <= m.residual(level) + 1e-9,
                "level {level}: |dh| {dh} > residual {}",
                m.residual(level)
            );
        }
    }
}

#[test]
fn v2_relief_bound_over_direction_pairs_cube_and_hex() {
    let t = terrain_v2();
    let cube = LodModel::for_topology(&t, DEFAULT_KAPPA, CubeSphere::new(N).sample_angle0());
    relief_bound_pairs(&cube, &t, 21);
    let hex = LodModel::for_topology(
        &t,
        DEFAULT_KAPPA,
        HexSphere::new(N).unwrap().sample_angle0(),
    );
    relief_bound_pairs(&hex, &t, 22);
}
