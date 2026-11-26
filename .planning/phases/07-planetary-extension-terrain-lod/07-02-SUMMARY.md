---
phase: 07-planetary-extension-terrain-lod
plan: 02
subsystem: planetary-core
tags: [cube-sphere, topology, patch-id, adjacency]
requires: []
provides: [Face, Edge, FaceBasis, PatchId, MAX_LEVEL, PatchIdError, Neighbor, neighbor, neighbor_children]
affects: [planetary-core]
tech-stack:
  added: []
  patterns: [const-derived link table, independent integer oracle]
key-files:
  created:
    - extensions/planetary/planetary-core/src/face.rs
    - extensions/planetary/planetary-core/src/patch_id.rs
    - extensions/planetary/planetary-core/src/adjacency.rs
    - extensions/planetary/planetary-core/tests/patch_id.rs
    - extensions/planetary/planetary-core/tests/adjacency.rs
  modified:
    - extensions/planetary/planetary-core/src/lib.rs
key-decisions:
  - "Cross-face links are a const 6x4 table built by const fn from the integer bases (back edge outward == +n_f); no runtime Option unwrap"
  - "PatchId::from_parts is a crate-private unchecked constructor so adjacency needs no expect/unwrap"
requirements: [PLAN-02]
metrics:
  completed: 2026-11-25
---

# Phase 7 Plan 02: Cube-sphere topology Summary

Packed u64 PatchId (face 3b | level 5b | morton, MAX_LEVEL 24) and same-level cross-face adjacency derived from six integer face bases, verified exhaustively through level 6 against an independent integer-geometry oracle.

## Results
- `cargo test -p planetary-core --locked`: 16 tests pass (9 patch_id, 6 adjacency integration, 1 const-link unit test); adjacency suite runs in about 0.5 s debug.
- Exhaustive adjacency coverage: levels 0..=6, all (patch, edge) pairs = 24 * (4^7 - 1) / 3 = 130,968 reciprocity checks, each also oracle-checked (shared corner set, flip flag); cross-face pair count equals 24 * 2^L at every level; neighbor_children checked for L 0..=5; all 8 cube corners have 3 mutually adjacent level-2 patches.
- clippy `-D warnings` and `cargo fmt --check` clean; no unwrap()/expect( in adjacency.rs, no HashMap/HashSet.

## Commits
- a1fb23b: Face/Edge bases and packed PatchId
- a4b193f: cross-face adjacency and oracle tests

## Deviations from Plan
- TDD RED commits were not made separately: a failing-to-compile test commit would violate "only commit states that compile", so tests and implementation were committed together per task (tests written first locally).
- Face::from_index/from_normal and FaceBasis::outward/edge_with_outward are `const fn` (needed for the const link table).

## Known Stubs
None.

## Self-Check: PASSED
