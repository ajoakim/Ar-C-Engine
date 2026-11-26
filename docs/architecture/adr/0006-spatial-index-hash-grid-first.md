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
