---
phase: 02-foundation-core-jobs-diagnostics-precision
plan: 02
subsystem: engine-math
tags: [precision, frames, rebase, f64]
requires: [02-01]
provides: [WorldPos, FrameRegistry, PositionStore, rebase]
affects: [engine-math]
tech-stack:
  added: []
  patterns: [opaque position type, LCA frame conversion, explicit rebase]
key-files:
  created:
    - crates/engine-math/src/error.rs
    - crates/engine-math/src/world_pos.rs
    - crates/engine-math/src/frame.rs
    - crates/engine-math/src/rebase.rs
    - docs/architecture/adr/0004-worldpos-frames-rebasing.md
  modified:
    - crates/engine-math/src/lib.rs
    - docs/architecture/adr/README.md
key-decisions:
  - "Opaque WorldPos (f64 + FrameId), no Add/Sub/From; explicit rebase with no automatic trigger (ADR-0004)"
requirements-completed: [WORLD-02, WORLD-04]
completed: 2026-11-25
---

# Phase 2 Plan 02: WorldPos, Frames and Rebasing Summary

Opaque f64 `WorldPos` with registered coordinate frames (LCA-based conversion, camera-relative f32) and explicit `rebase()` over a `PositionStore`, verified at 6.4e6 m with 1e-8 m tolerance.

## Commits
- 535ea29: WorldPos, FrameRegistry, conversion (Task 1)
- 67834be: PositionStore, rebase, ADR-0004 (Task 2)

## Deviations from Plan
- Added `FrameRegistry::children` in frame.rs up front (the plan allowed this).
- The f32 motivation test uses `std::hint::black_box` to satisfy clippy `assertions_on_constants` under `-D warnings`.

## Verification
16 engine-math tests pass; clippy `-D warnings` clean; `cargo xtask check-deps` OK.

## Known Stubs
None.

## Self-Check: PASSED
