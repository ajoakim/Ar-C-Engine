//! engine_world::World (slotmap handles) vs hecs 0.11.2 (ADR-0017 evidence).
//!
//! Groups `slotmap_world/<case>` and `hecs/<case>` for N in {1e4, 1e5}. Cases:
//! `spawn`, `update_all_locals`, `random_1pct`, `despawn10_respawn`,
//! `hierarchy_update`. The engine World has no bulk iteration API beyond
//! `iter()` plus `local`/`set_local`, so those cases measure the API the engine
//! actually exposes, not a hand-tuned loop. hecs uses its natural query path.
//! The hierarchy case uses the same transform composition (`WorldTransform`)
//! in both worlds; the engine caches every world transform, hecs only stores
//! the roots' and recomputes the children's without storing them. The engine
//! implements hierarchy and frame semantics hecs lacks, so this is the cost of
//! what the engine needs, not feature parity. No timing is asserted.

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use ecs_bench::{GROUP, SIZES, nudge, sample_indices, transform};
use engine_math::FrameRegistry;
use engine_world::{EntityId, Transform, World, WorldTransform};
use std::hint::black_box;

struct Parent(hecs::Entity);

fn engine_flat(n: usize) -> (World, Vec<EntityId>) {
    let frame = FrameRegistry::new().root();
    let mut w = World::new();
    let ids = (0..n)
        .map(|i| w.spawn(frame, transform(i)).expect("finite"))
        .collect();
    (w, ids)
}

fn hecs_flat(n: usize) -> (hecs::World, Vec<hecs::Entity>) {
    let mut w = hecs::World::new();
    let ids = (0..n).map(|i| w.spawn((transform(i),))).collect();
    (w, ids)
}

/// Groups of `GROUP`: one root with `GROUP - 1` direct children. Returns roots.
fn engine_tree(n: usize) -> (World, Vec<EntityId>) {
    let frame = FrameRegistry::new().root();
    let mut w = World::new();
    let mut roots = Vec::new();
    for g in 0..n / GROUP {
        let r = w.spawn(frame, transform(g * GROUP)).expect("finite");
        for k in 1..GROUP {
            w.spawn_child(r, transform(g * GROUP + k)).expect("finite");
        }
        roots.push(r);
    }
    (w, roots)
}

fn hecs_tree(n: usize) -> hecs::World {
    let frame = FrameRegistry::new().root();
    let mut w = hecs::World::new();
    for g in 0..n / GROUP {
        let t = transform(g * GROUP);
        let wt = WorldTransform::from_root(frame, &t).expect("finite");
        let r = w.spawn((t, wt));
        for k in 1..GROUP {
            w.spawn((transform(g * GROUP + k), Parent(r)));
        }
    }
    w
}

fn bench_engine(c: &mut Criterion) {
    let frame = FrameRegistry::new().root();
    let mut g = c.benchmark_group("slotmap_world");
    for n in SIZES {
        let sample = sample_indices(n);
        g.bench_function(BenchmarkId::new("spawn", n), |b| {
            b.iter(|| black_box(engine_flat(n)));
        });
        g.bench_function(BenchmarkId::new("update_all_locals", n), |b| {
            let (mut w, ids) = engine_flat(n);
            b.iter(|| {
                for &id in &ids {
                    let t = w.local(id).expect("live");
                    w.set_local(id, nudge(t)).expect("finite");
                }
            });
        });
        g.bench_function(BenchmarkId::new("random_1pct", n), |b| {
            let (mut w, ids) = engine_flat(n);
            b.iter(|| {
                for &i in &sample {
                    let id = ids[i];
                    let t = w.local(id).expect("live");
                    w.set_local(id, nudge(t)).expect("finite");
                }
            });
        });
        g.bench_function(BenchmarkId::new("despawn10_respawn", n), |b| {
            b.iter_batched(
                || engine_flat(n),
                |(mut w, ids)| {
                    for id in ids.iter().step_by(10) {
                        w.despawn(*id).expect("live");
                    }
                    for i in 0..n / 10 {
                        w.spawn(frame, transform(i)).expect("finite");
                    }
                    black_box(w)
                },
                BatchSize::LargeInput,
            );
        });
        g.bench_function(BenchmarkId::new("hierarchy_update", n), |b| {
            let (mut w, roots) = engine_tree(n);
            w.update_transforms();
            b.iter(|| {
                for &r in &roots {
                    let t = w.local(r).expect("live");
                    w.set_local(r, nudge(t)).expect("finite");
                }
                black_box(w.update_transforms())
            });
        });
    }
    g.finish();
}

fn bench_hecs(c: &mut Criterion) {
    let frame = FrameRegistry::new().root();
    let mut g = c.benchmark_group("hecs");
    for n in SIZES {
        let sample = sample_indices(n);
        g.bench_function(BenchmarkId::new("spawn", n), |b| {
            b.iter(|| black_box(hecs_flat(n)));
        });
        g.bench_function(BenchmarkId::new("update_all_locals", n), |b| {
            let (mut w, _ids) = hecs_flat(n);
            b.iter(|| {
                for t in w.query_mut::<&mut Transform>() {
                    *t = nudge(*t);
                }
            });
        });
        g.bench_function(BenchmarkId::new("random_1pct", n), |b| {
            let (mut w, ids) = hecs_flat(n);
            b.iter(|| {
                for &i in &sample {
                    let t = w.query_one_mut::<&mut Transform>(ids[i]).expect("live");
                    *t = nudge(*t);
                }
            });
        });
        g.bench_function(BenchmarkId::new("despawn10_respawn", n), |b| {
            b.iter_batched(
                || hecs_flat(n),
                |(mut w, ids)| {
                    for id in ids.iter().step_by(10) {
                        w.despawn(*id).expect("live");
                    }
                    for i in 0..n / 10 {
                        w.spawn((transform(i),));
                    }
                    black_box(w)
                },
                BatchSize::LargeInput,
            );
        });
        g.bench_function(BenchmarkId::new("hierarchy_update", n), |b| {
            let mut w = hecs_tree(n);
            b.iter(|| {
                // Pass 1: nudge roots and recompute their stored world transform.
                let mut count = 0usize;
                for (t, wt) in w.query_mut::<(&mut Transform, &mut WorldTransform)>() {
                    *t = nudge(*t);
                    *wt = WorldTransform::from_root(frame, t).expect("finite");
                    count += 1;
                }
                // Pass 2: compose each child on its parent's stored world transform.
                for (t, p) in w.query::<(&Transform, &Parent)>().iter() {
                    let pw = *w.get::<&WorldTransform>(p.0).expect("parent world");
                    black_box(pw.compose(t).expect("finite"));
                    count += 1;
                }
                black_box(count)
            });
        });
    }
    g.finish();
}

criterion_group!(benches, bench_engine, bench_hecs);
criterion_main!(benches);
