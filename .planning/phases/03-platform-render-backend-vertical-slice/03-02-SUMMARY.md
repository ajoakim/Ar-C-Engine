---
phase: 03-platform-render-backend-vertical-slice
plan: 02
subsystem: render
tags: [render-backend, null-backend, validation]
requires: ["03-01"]
provides: [RenderBackend trait, FramePacket types, ResourceTable, NullBackend]
affects: [03-04, 03-05, 03-06]
key-files:
  created: [crates/engine-render/src/handles.rs, error.rs, backend.rs, resources.rs, null.rs, crates/engine-render/tests/null_backend.rs]
  modified: [crates/engine-render/src/lib.rs]
requirements: [REND-01]
metrics:
  completed: 2026-11-25
---

# Phase 3 Plan 02: Render boundary and NullBackend Summary

Engine-owned RenderBackend trait, handles, packets and RenderTarget (no GPU/window names), with shared ResourceTable validation and a headless NullBackend with bounded frame history.

## Commits
- 232a9e1: boundary contracts (handles, error, backend)
- ResourceTable + NullBackend + tests: 3a13c01

## Verification
- `cargo test -p engine-render --locked`: 2 unit + 1 resources unit + 12 null_backend tests pass
- clippy -D warnings clean, `cargo xtask check-deps` OK

## Deviations from Plan
Task 1 clippy (`-D warnings`) flags `from_raw` as dead code until Task 2 uses it; Task 1 tests pass, clippy clean after Task 2. Otherwise none.

## Known Stubs
None.

## Self-Check: PASSED
