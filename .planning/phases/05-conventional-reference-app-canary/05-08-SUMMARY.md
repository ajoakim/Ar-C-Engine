---
phase: 05-conventional-reference-app-canary
plan: 08
subsystem: conventional-3d
tags: [jobs, assets, input, camera]
requires: [05-07]
provides: [ModelLoader, ModelAssets, FlyCamera]
affects: [extensions/conventional-3d]
key-files:
  created:
    - extensions/conventional-3d/src/loader.rs
    - extensions/conventional-3d/src/fly_camera.rs
    - extensions/conventional-3d/tests/model_loader.rs
    - extensions/conventional-3d/tests/fly_camera.rs
  modified:
    - extensions/conventional-3d/src/lib.rs
requirements: [REND-05, APP-01]
completed: 2026-11-25
---

# Phase 5 Plan 08: Model loader and fly camera Summary

Asynchronous glTF import on the engine JobSystem (mpsc delivery, poll/wait/wait_all, panic and cancel mapped to ImportError::JobFailed) with deterministic AssetId registration, plus an f64 FlyCamera driven by SDK InputEvents.

## Commits
- 887ff4f: ModelLoader and ModelAssets (6 tests)
- 796b262: FlyCamera (11 tests)

## Deviations from Plan
- Added two extra tests (unknown ticket, non-finite cursor). Fly camera rotates horizontal input by the full orientation (pitch included); vertical keys stay world +Y. Otherwise as written.

## Known Stubs
None.

## Self-Check: PASSED
