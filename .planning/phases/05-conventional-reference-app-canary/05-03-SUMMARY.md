---
phase: 05-conventional-reference-app-canary
plan: 03
subsystem: engine-render
tags: [render, materials, textures, index-buffers, validation]
requires: [05-01]
provides: [material/light/texture contract types, PosNormalUv layout, indexed MeshHandle, RenderBackend texture methods]
affects: [engine-render-wgpu (05-05), engine-sdk (05-06)]
key-files:
  created: [crates/engine-render/src/material.rs]
  modified: [handles.rs, backend.rs, error.rs, resources.rs, null.rs, lib.rs, graph.rs, scene.rs, tests/null_backend.rs, tests/precision.rs]
requirements: [REND-04]
duration: short
completed: 2026-11-25
---

# Phase 5 Plan 03: Render contract for lit/textured/indexed meshes Summary

engine-render now has GPU-free Material/MaterialTable/DirectionalLight/TextureDesc types, a PosNormalUv vertex layout, indexed MeshHandle (layout + optional u32 index buffer), RenderBackend create/destroy_texture, and shared validation (layout mismatch, index usage/count/bounds, texture size) in ResourceTable, implemented by NullBackend.

## Verification
- `cargo test -p engine-render --locked`: 31 lib + 18 null_backend + 5 precision tests pass
- `cargo clippy -p engine-render --all-targets --locked -- -D warnings`: clean
- `cargo xtask check-deps`: OK
- engine-render-wgpu / engine-sdk intentionally not built (expected breakage until 05-05/05-06).

## Deviations from Plan
- Both tasks were committed in a single commit (86d5786), because the Task 1 exports in lib.rs and the new trait methods require Task 2's table/NullBackend code to compile.
- [Rule 1] clippy `manual_is_multiple_of` fixed with `is_multiple_of(3)`.

## Known Stubs
None.

## Self-Check: PASSED
