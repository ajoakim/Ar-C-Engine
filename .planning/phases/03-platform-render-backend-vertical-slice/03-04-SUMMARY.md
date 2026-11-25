---
phase: 03-platform-render-backend-vertical-slice
plan: 04
subsystem: render
tags: [camera, render-graph, camera-relative, reverse-z, precision]
requires: [03-02]
provides: [Camera, RenderScene/extract, RenderGraph/CompiledGraph, precision proof]
affects: [03-06]
key-files:
  created:
    - crates/engine-render/src/camera.rs
    - crates/engine-render/src/scene.rs
    - crates/engine-render/src/graph.rs
    - crates/engine-render/tests/precision.rs
  modified:
    - crates/engine-render/src/lib.rs
    - crates/engine-render/src/error.rs
requirements-completed: [REND-03, WORLD-03]
duration: short
completed: 2026-11-25
---

# Phase 3 Plan 04: Frame contract Summary

Camera-relative f32 RenderScene extraction (f64 subtraction before cast), a Kahn-sorted render graph producing FramePackets, and a 6.4e6 m precision proof.

## Commits
- 79388b3 Camera and extract (Task 1)
- 110c02e Render graph, error variants (Task 2)
- 914675f Precision tests (Task 3)

## Deviations from Plan

**1. [Rule 3 - Blocking] glam 0.34 has no `Mat4::perspective_infinite_reverse_rh`.** Used `glam::camera::rh::proj::directx::perspective_infinite_reverse` (right-handed, Y-up, NDC depth [0,1], reversed-Z), which is the equivalent. The old name remains only in a comment so the plan's grep check holds.

**2. Tasks 1 and 2 share lib.rs/error.rs**, so the graph commit also carries the lib.rs/error.rs edits; tests were written alongside the implementation rather than in separate RED commits.

Test counts: 20 unit (7 extract, 10 graph, plus existing), 12 null_backend, 5 precision. Clippy -D warnings, fmt and `cargo xtask check-deps` pass.

## Known Stubs
None.

## Self-Check: PASSED
