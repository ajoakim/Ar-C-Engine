# ADR-0006: Spatial Index - Uniform Hash Grid First, Benchmark Later

- Status: accepted
- Date: 2026-11-26
- Requirement/Phase: WORLD-05, WS-05 (spatial backend) / Phase 4

## Context

WORLD-05 needs visibility and region queries through a pluggable interface. Planetary and conventional scenes differ wildly in scale and density, and there is no benchmark data yet to pick a structure on evidence.

## Decision

1. An engine-owned `SpatialIndex` trait lives in engine-spatial with insert/update/remove, `query_frustum`, `query_region` and `query_radius`. Queries return `EntityId`s: the output is cleared, then filled sorted ascending and deduplicated.
2. The default backend is `HashGridIndex`, a uniform hash grid over f64 coordinates in one explicit index `FrameId`. The default cell size is 64 m. Entities spanning more than `max_cells_per_entity` (default 512) cells are kept in an always-tested oversize list. Queries whose cell range exceeds the number of stored entities fall back to a linear scan.
3. A brute-force `LinearIndex` backend is kept as the correctness oracle and as the second implementation.
4. Volumes (`Aabb`, `Sphere`, `Frustum`) are f64 and built from `WorldPos` in the index frame. A different frame yields a `FrameMismatch` error.
5. The index frame should not be rebased. If it is, the caller re-issues `update` for every entity with shifted bounds. There is no automatic tracking.
6. The frustum convention is right-handed, +Y up, -Z forward (matches engine-render camera.rs). A finite far plane is required.

## Alternatives Considered

- BVH: better for static, uneven scenes, but has a rebuild cost.
- rstar R-tree: adds a dependency, f64 support needs checking, insert cost is higher.
- kiddo kd-tree: designed for points, not boxes.
- Octree: needs depth tuning.

All are rejected for now because there is no benchmark evidence. The benchmark is deferred to the Phase 8 decision spike (WS-05). ECS-integrated spatial data is out of scope (no ECS).

## Consequences

The design is simple, has O(1) updates and gives deterministic sorted output. It performs poorly for very non-uniform density and huge objects (the oversize list mitigates this). The trait has no backend-specific types, so a second backend can slot in.

## Evidence

Phase 4 oracle tests compare `HashGridIndex` with `LinearIndex` over pseudo-random entities (crates/engine-spatial/tests/oracle.rs). The Phase 8 benchmark spike will add performance evidence and may supersede this ADR.

## Amendment (Phase 8)

Date: 2026-11-26. Conclusion: keep the hash grid as the default backend; the decision above is not superseded. The "benchmark later" item is now done.

Evidence: `crates/engine-spatial/benches/spatial.rs` (criterion, release, `--quick`, three runs; full table and method in `.planning/phases/08-hardening-decisions/08-BENCH-RESULTS.md`). Entity density is constant across N; each query figure is a batch of 16 queries.

- Flat scaling: the hash grid is essentially flat in N from 1e3 to 1e5 (region 4.9 to 7.5 us, radius 11.7 to 15.0 us, frustum 53.6 to 87.1 us) while the linear scan grows linearly to 7 to 10 ms at N = 1e5. Linear / HashGrid at N = 1e5: region 1308.75x, radius 473.10x, frustum 106.67x; at N = 1e4: 177.19x, 58.24x, 16.74x.
- Crossover: region and radius queries break even at roughly N = 100 to 300 (HashGrid is already 3.74x and 1.57x faster at N = 300; Linear is equal or faster at N = 100: ratios 1.05x and 0.57x). Frustum queries break even between N = 300 (Linear 2.8x faster, ratio 0.36x) and N = 1000 (HashGrid 1.72x faster). Below a few hundred queried entities the linear scan is as good as or better than the grid.
- Updates are the weak point: moving 10 percent of the entities and back is 8x to 24x slower on the hash grid than on the linear scan at every N (Linear / HashGrid 0.04x to 0.12x; 7.66 ms against 906 us at N = 1e5), because the grid re-buckets cells and maintains a `BTreeMap`. This is the first thing to revisit if dynamic-entity counts grow; the O(1) update claim in Consequences holds per entity, not relative to an in-place write.
- Not measured: rstar and kd-tree alternatives (out of the plan scope). The oversize-list and non-uniform-density concerns are still untested by a benchmark.

Revisit trigger: supersede this ADR if a reference workload is update-dominated and profiling shows `HashGridIndex` updates in the top frame costs, or if a second backend benchmarks better on the real workload.
