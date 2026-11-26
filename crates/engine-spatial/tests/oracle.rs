//! Oracle tests: HashGridIndex must equal the brute-force LinearIndex.

use engine_math::{DQuat, DVec3, FrameRegistry};
use engine_spatial::{
    Aabb, Frustum, HashGridConfig, HashGridIndex, LinearIndex, SpatialError, SpatialIndex, Sphere,
};
use engine_world::{EntityId, Transform, World};
use glam::EulerRot;

struct XorShift64(u64);

impl XorShift64 {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        let u = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        lo + (hi - lo) * u
    }
    fn vec(&mut self, lo: f64, hi: f64) -> DVec3 {
        DVec3::new(self.range(lo, hi), self.range(lo, hi), self.range(lo, hi))
    }
}

fn q(v: f64, quantize: bool) -> f64 {
    if quantize {
        (v * 1024.0).round() / 1024.0
    } else {
        v
    }
}

fn qv(v: DVec3, quantize: bool) -> DVec3 {
    DVec3::new(q(v.x, quantize), q(v.y, quantize), q(v.z, quantize))
}

fn rand_bounds(rng: &mut XorShift64, quantize: bool) -> Aabb {
    let c = qv(rng.vec(-5000.0, 5000.0), quantize);
    let h = qv(rng.vec(0.1, 50.0), quantize);
    Aabb::from_center_half_extents(c, h).unwrap()
}

struct Scene {
    ids: Vec<EntityId>,
    linear: LinearIndex,
    grid: HashGridIndex,
}

fn build(rng: &mut XorShift64, quantize: bool, config: HashGridConfig) -> Scene {
    let frame = FrameRegistry::new().root();
    let mut world = World::new();
    let mut linear = LinearIndex::new(frame);
    let mut grid = HashGridIndex::new(frame, config).unwrap();
    let mut ids = Vec::new();
    for k in 0..605 {
        let id = world.spawn(frame, Transform::IDENTITY).unwrap();
        let b = if k < 600 {
            rand_bounds(rng, quantize)
        } else {
            let c = qv(rng.vec(-5000.0, 5000.0), quantize);
            Aabb::from_center_half_extents(c, DVec3::splat(1e6)).unwrap()
        };
        linear.insert(id, b).unwrap();
        grid.insert(id, b).unwrap();
        ids.push(id);
    }
    Scene { ids, linear, grid }
}

fn rand_region(rng: &mut XorShift64, quantize: bool) -> Aabb {
    let c = qv(rng.vec(-5000.0, 5000.0), quantize);
    let h = rng.range(1.0, 500.0);
    Aabb::from_center_half_extents(c, DVec3::splat(q(h, quantize))).unwrap()
}

fn rand_sphere(rng: &mut XorShift64, quantize: bool) -> Sphere {
    let c = qv(rng.vec(-5000.0, 5000.0), quantize);
    Sphere::new(c, q(rng.range(1.0, 800.0), quantize)).unwrap()
}

fn rand_frustum(rng: &mut XorShift64) -> Frustum {
    let pos = rng.vec(-5000.0, 5000.0);
    let rot = DQuat::from_euler(
        EulerRot::YXZ,
        rng.range(-3.1, 3.1),
        rng.range(-1.5, 1.5),
        0.0,
    );
    Frustum::from_perspective(
        pos,
        rot,
        rng.range(0.3, 2.0),
        rng.range(0.5, 2.0),
        rng.range(0.1, 5.0),
        rng.range(50.0, 3000.0),
    )
    .unwrap()
}

fn check_region(s: &Scene, rng: &mut XorShift64, n: usize) {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for _ in 0..n {
        let r = rand_region(rng, false);
        s.linear.query_region(&r, &mut a);
        s.grid.query_region(&r, &mut b);
        assert_eq!(a, b);
    }
}

fn check_radius(s: &Scene, rng: &mut XorShift64, n: usize) {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for _ in 0..n {
        let sp = rand_sphere(rng, false);
        s.linear.query_radius(&sp, &mut a);
        s.grid.query_radius(&sp, &mut b);
        assert_eq!(a, b);
    }
}

fn check_frustum(s: &Scene, rng: &mut XorShift64, n: usize) {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for _ in 0..n {
        let f = rand_frustum(rng);
        s.linear.query_frustum(&f, &mut a);
        s.grid.query_frustum(&f, &mut b);
        assert_eq!(a, b);
    }
}

#[test]
fn oracle_region_queries_match_linear() {
    let mut rng = XorShift64(0x2545_F491_4F6C_DD1D);
    let s = build(&mut rng, false, HashGridConfig::default());
    check_region(&s, &mut rng, 200);
}

#[test]
fn oracle_radius_queries_match_linear() {
    let mut rng = XorShift64(0x1234_5678_9ABC_DEF1);
    let s = build(&mut rng, false, HashGridConfig::default());
    check_radius(&s, &mut rng, 200);
}

#[test]
fn oracle_frustum_queries_match_linear() {
    let mut rng = XorShift64(0x0F0F_1234_ABCD_5577);
    let s = build(&mut rng, false, HashGridConfig::default());
    check_frustum(&s, &mut rng, 200);
}

#[test]
fn oracle_after_updates_and_removals_match_linear() {
    let mut rng = XorShift64(0x7777_1111_2222_3333);
    let mut s = build(&mut rng, false, HashGridConfig::default());
    for _ in 0..300 {
        let id = s.ids[(rng.next_u64() % s.ids.len() as u64) as usize];
        let b = rand_bounds(&mut rng, false);
        let l = s.linear.update(id, b);
        let g = s.grid.update(id, b);
        assert_eq!(l, g);
    }
    for _ in 0..100 {
        let id = s.ids[(rng.next_u64() % s.ids.len() as u64) as usize];
        assert_eq!(s.linear.remove(id), s.grid.remove(id));
    }
    assert_eq!(s.linear.len(), s.grid.len());
    check_region(&s, &mut rng, 100);
    check_radius(&s, &mut rng, 100);
    check_frustum(&s, &mut rng, 100);
}

#[test]
fn oracle_rebase_shift_preserves_results() {
    let mut rng = XorShift64(0x5555_AAAA_3333_CCCC);
    let mut s = build(&mut rng, true, HashGridConfig::default());
    let shift = DVec3::new(6.4e6, 0.0, 0.0);

    let regions: Vec<Aabb> = (0..100).map(|_| rand_region(&mut rng, true)).collect();
    let spheres: Vec<Sphere> = (0..100).map(|_| rand_sphere(&mut rng, true)).collect();

    let mut before_r = Vec::new();
    let mut before_s = Vec::new();
    let mut out = Vec::new();
    for r in &regions {
        s.grid.query_region(r, &mut out);
        before_r.push(out.clone());
    }
    for sp in &spheres {
        s.grid.query_radius(sp, &mut out);
        before_s.push(out.clone());
    }

    for id in s.ids.clone() {
        let b = s.grid.bounds(id).unwrap();
        let nb = Aabb::new(b.min() + shift, b.max() + shift).unwrap();
        s.grid.update(id, nb).unwrap();
        s.linear.update(id, nb).unwrap();
    }

    for (r, expect) in regions.iter().zip(&before_r) {
        let sr = Aabb::new(r.min() + shift, r.max() + shift).unwrap();
        s.grid.query_region(&sr, &mut out);
        assert_eq!(&out, expect);
    }
    for (sp, expect) in spheres.iter().zip(&before_s) {
        let ss = Sphere::new(sp.center() + shift, sp.radius()).unwrap();
        s.grid.query_radius(&ss, &mut out);
        assert_eq!(&out, expect);
    }
    let mut other = Vec::new();
    for _ in 0..50 {
        let pos = rng.vec(-5000.0, 5000.0) + shift;
        let rot = DQuat::from_euler(EulerRot::YXZ, rng.range(-3.1, 3.1), 0.0, 0.0);
        let f = Frustum::from_perspective(pos, rot, 1.0, 1.0, 0.5, 2000.0).unwrap();
        s.grid.query_frustum(&f, &mut out);
        s.linear.query_frustum(&f, &mut other);
        assert_eq!(out, other);
    }
}

#[test]
fn oracle_small_cell_size_matches_linear() {
    let mut rng = XorShift64(0x9999_8888_7777_6666);
    let cfg = HashGridConfig {
        cell_size: 1.0,
        max_cells_per_entity: 8,
    };
    let s = build(&mut rng, false, cfg);
    assert!(s.grid.oversize_count() > 5);
    check_region(&s, &mut rng, 100);
    check_radius(&s, &mut rng, 100);
    check_frustum(&s, &mut rng, 100);
}

#[test]
fn trait_object_backends_interchangeable() -> Result<(), SpatialError> {
    let mut rng = XorShift64(0x4242_4242_4242_4242);
    let f = FrameRegistry::new().root();
    let mut world = World::new();
    let mut backends: Vec<Box<dyn SpatialIndex>> = vec![
        Box::new(LinearIndex::new(f)),
        Box::new(HashGridIndex::new(f, HashGridConfig::default())?),
    ];
    for _ in 0..100 {
        let id = world.spawn(f, Transform::IDENTITY).unwrap();
        let b = rand_bounds(&mut rng, false);
        for be in &mut backends {
            be.insert(id, b)?;
        }
    }
    let r = rand_region(&mut rng, false);
    let sp = rand_sphere(&mut rng, false);
    let fr = Frustum::from_perspective(DVec3::ZERO, DQuat::IDENTITY, 1.5, 1.0, 0.1, 6000.0)?;
    let mut results: Vec<[Vec<EntityId>; 3]> = Vec::new();
    for be in &backends {
        assert_eq!(be.len(), 100);
        let (mut a, mut b, mut c) = (Vec::new(), Vec::new(), Vec::new());
        be.query_region(&r, &mut a);
        be.query_radius(&sp, &mut b);
        be.query_frustum(&fr, &mut c);
        results.push([a, b, c]);
    }
    assert_eq!(results[0], results[1]);
    Ok(())
}
