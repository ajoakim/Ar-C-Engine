//! HashGridIndex vs LinearIndex (ADR-0006 evidence, Phase 8 / QA-01, WS-05).
//!
//! Entity density is held constant across N (the world extent grows with the
//! cube root of N) so the comparison shows how each backend scales with entity
//! count rather than with crowding. Query volumes are the same at every N.
//! Positions come from a seeded xorshift generator; no timing is asserted.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use engine_math::{DQuat, DVec3, FrameRegistry};
use engine_spatial::{
    Aabb, Frustum, HashGridConfig, HashGridIndex, LinearIndex, SpatialIndex, Sphere,
};
use engine_world::{EntityId, Transform, World};
use std::hint::black_box;

/// 100 and 300 are extra points to locate the query crossover below the plan's 1e3.
const SIZES: [usize; 5] = [100, 300, 1_000, 10_000, 100_000];
/// Half-extent of the world at N = 1000; scaled by cbrt(N / 1000).
const BASE_HALF_WORLD: f64 = 500.0;

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

fn half_world(n: usize) -> f64 {
    BASE_HALF_WORLD * (n as f64 / 1000.0).cbrt()
}

fn rand_bounds(rng: &mut XorShift64, hw: f64) -> Aabb {
    let c = rng.vec(-hw, hw);
    let h = rng.vec(0.1, 5.0);
    Aabb::from_center_half_extents(c, h).unwrap()
}

struct Scene {
    ids: Vec<EntityId>,
    bounds: Vec<Aabb>,
    linear: LinearIndex,
    grid: HashGridIndex,
}

fn build(n: usize) -> Scene {
    let mut rng = XorShift64(0x2545_F491_4F6C_DD1D ^ n as u64);
    let hw = half_world(n);
    let frame = FrameRegistry::new().root();
    let mut world = World::new();
    let mut linear = LinearIndex::new(frame);
    let mut grid = HashGridIndex::new(frame, HashGridConfig::default()).unwrap();
    let mut ids = Vec::with_capacity(n);
    let mut bounds = Vec::with_capacity(n);
    for _ in 0..n {
        let id = world.spawn(frame, Transform::IDENTITY).unwrap();
        let b = rand_bounds(&mut rng, hw);
        linear.insert(id, b).unwrap();
        grid.insert(id, b).unwrap();
        ids.push(id);
        bounds.push(b);
    }
    Scene {
        ids,
        bounds,
        linear,
        grid,
    }
}

fn queries(n: usize) -> (Vec<Aabb>, Vec<Sphere>, Vec<Frustum>) {
    let mut rng = XorShift64(0x1234_5678_9ABC_DEF1);
    let hw = half_world(n);
    let regions = (0..16)
        .map(|_| {
            Aabb::from_center_half_extents(rng.vec(-hw, hw), DVec3::splat(rng.range(20.0, 80.0)))
                .unwrap()
        })
        .collect();
    let spheres = (0..16)
        .map(|_| Sphere::new(rng.vec(-hw, hw), rng.range(20.0, 100.0)).unwrap())
        .collect();
    let frustums = (0..16)
        .map(|_| {
            let rot = DQuat::from_euler(
                glam::EulerRot::YXZ,
                rng.range(-3.1, 3.1),
                rng.range(-1.0, 1.0),
                0.0,
            );
            Frustum::from_perspective(rng.vec(-hw, hw), rot, 1.0, 1.5, 0.5, 200.0).unwrap()
        })
        .collect();
    (regions, spheres, frustums)
}

fn bench_queries(c: &mut Criterion) {
    for &n in &SIZES {
        let scene = build(n);
        let (regions, spheres, frustums) = queries(n);
        let mut out = Vec::new();

        let mut g = c.benchmark_group("region");
        if n >= 100_000 {
            g.sample_size(20);
        }
        g.bench_with_input(BenchmarkId::new("hash_grid", n), &n, |b, _| {
            b.iter(|| {
                for r in &regions {
                    scene.grid.query_region(black_box(r), &mut out);
                }
                black_box(out.len())
            })
        });
        g.bench_with_input(BenchmarkId::new("linear", n), &n, |b, _| {
            b.iter(|| {
                for r in &regions {
                    scene.linear.query_region(black_box(r), &mut out);
                }
                black_box(out.len())
            })
        });
        g.finish();

        let mut g = c.benchmark_group("radius");
        if n >= 100_000 {
            g.sample_size(20);
        }
        g.bench_with_input(BenchmarkId::new("hash_grid", n), &n, |b, _| {
            b.iter(|| {
                for s in &spheres {
                    scene.grid.query_radius(black_box(s), &mut out);
                }
                black_box(out.len())
            })
        });
        g.bench_with_input(BenchmarkId::new("linear", n), &n, |b, _| {
            b.iter(|| {
                for s in &spheres {
                    scene.linear.query_radius(black_box(s), &mut out);
                }
                black_box(out.len())
            })
        });
        g.finish();

        let mut g = c.benchmark_group("frustum");
        if n >= 100_000 {
            g.sample_size(20);
        }
        g.bench_with_input(BenchmarkId::new("hash_grid", n), &n, |b, _| {
            b.iter(|| {
                for f in &frustums {
                    scene.grid.query_frustum(black_box(f), &mut out);
                }
                black_box(out.len())
            })
        });
        g.bench_with_input(BenchmarkId::new("linear", n), &n, |b, _| {
            b.iter(|| {
                for f in &frustums {
                    scene.linear.query_frustum(black_box(f), &mut out);
                }
                black_box(out.len())
            })
        });
        g.finish();
    }
}

/// Move 10% of entities by a small offset, then move them back, so state is
/// stable across iterations (two update passes per iteration).
fn bench_update(c: &mut Criterion) {
    for &n in &SIZES {
        let mut scene = build(n);
        let moved: Vec<(EntityId, Aabb, Aabb)> = (0..n)
            .step_by(10)
            .map(|i| {
                let b = scene.bounds[i];
                let d = DVec3::new(30.0, 10.0, -20.0);
                let nb = Aabb::new(b.min() + d, b.max() + d).unwrap();
                (scene.ids[i], b, nb)
            })
            .collect();

        let mut g = c.benchmark_group("update_10pct_roundtrip");
        if n >= 100_000 {
            g.sample_size(20);
        }
        g.bench_with_input(BenchmarkId::new("hash_grid", n), &n, |b, _| {
            b.iter(|| {
                for (id, _, nb) in &moved {
                    scene.grid.update(*id, black_box(*nb)).unwrap();
                }
                for (id, ob, _) in &moved {
                    scene.grid.update(*id, black_box(*ob)).unwrap();
                }
            })
        });
        g.bench_with_input(BenchmarkId::new("linear", n), &n, |b, _| {
            b.iter(|| {
                for (id, _, nb) in &moved {
                    scene.linear.update(*id, black_box(*nb)).unwrap();
                }
                for (id, ob, _) in &moved {
                    scene.linear.update(*id, black_box(*ob)).unwrap();
                }
            })
        });
        g.finish();
    }
}

criterion_group!(benches, bench_queries, bench_update);
criterion_main!(benches);
