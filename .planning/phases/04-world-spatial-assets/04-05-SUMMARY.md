---
phase: 04-world-spatial-assets
plan: 05
subsystem: spatial
tags: [spatial, frustum, aabb, trait]
requires: [04-01, 04-02]
provides: [SpatialIndex trait, LinearIndex oracle, f64 volumes, SpatialError]
affects: [04-07]
key-files:
  created:
    - crates/engine-spatial/src/error.rs
    - crates/engine-spatial/src/volume.rs
    - crates/engine-spatial/src/index.rs
    - crates/engine-spatial/src/linear.rs
  modified:
    - crates/engine-spatial/src/lib.rs
requirements-completed: [WORLD-05]
duration: ~10 min
completed: 2026-11-25
---

# Phase 4 Plan 05: Spatial interface Summary

Validated f64 volumes (Aabb, Sphere, Plane, Frustum with -Z-forward camera convention and frame checks), an object-safe `SpatialIndex` trait, and a brute-force `LinearIndex` oracle.

## Tasks
- Task 1: SpatialError and volumes, commit a0c77bc (11 `volume_` tests)
- Task 2: SpatialIndex and LinearIndex, second task commit (6 `linear_` tests)

## Deviations from Plan
None. `query_radius` takes `&Sphere` as planned. Verified: tests (17), clippy -D warnings, `cargo xtask check-deps` all pass.

## Known Stubs
None.

## Self-Check: PASSED
