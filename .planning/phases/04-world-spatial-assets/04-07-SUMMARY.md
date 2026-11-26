---
phase: 04-world-spatial-assets
plan: 07
subsystem: spatial
tags: [hash-grid, spatial-index, oracle-tests]
requires: [04-05]
provides: [HashGridIndex, HashGridConfig]
affects: [crates/engine-spatial]
key-files:
  created: [crates/engine-spatial/src/hash_grid.rs, crates/engine-spatial/tests/oracle.rs]
  modified: [crates/engine-spatial/src/lib.rs]
requirements: [WORLD-05]
completed: 2026-11-25
---

# Phase 4 Plan 07: HashGridIndex Summary

Default uniform hash-grid SpatialIndex backend (cell 64 m, 512 cells/entity cap) with an oversize list and linear-scan fallback, proven equal to the LinearIndex oracle.

## Tasks
1. HashGridIndex + 6 `hash_grid_*` unit tests - 3f79c97
2. Oracle (region, radius, frustum, updates/removals, rebase shift, small cells) and trait-object tests, 7 tests - 5eccc75

## Deviations from Plan
None. Rebase test compares frustums only between backends after the shift, per plan.

## Verification
`cargo test -p engine-spatial` (23 unit + 7 oracle), clippy -D warnings, `cargo xtask check-deps` all pass.

## Self-Check: PASSED
