---
phase: 05-conventional-reference-app-canary
plan: 06
subsystem: engine-sdk
tags: [sdk, lit-rendering, facade]
requires: [05-02, 05-05]
provides: [SDK lit mesh/texture/material/light API, world/assets/jobs re-exports]
affects: [conventional-3d]
key-files:
  modified:
    - crates/engine-sdk/src/lib.rs
    - crates/engine-sdk/src/app.rs
    - crates/engine-sdk/src/error.rs
    - crates/engine-sdk/src/runtime.rs
    - crates/engine-sdk/tests/facade.rs
  created:
    - crates/engine-sdk/tests/headless_lit.rs
decisions:
  - Runtime registers unlit PosColor and lit PosNormalUv pipelines in a PipelineSet by layout
metrics:
  completed: 2026-11-25
requirements: [APP-01, REND-04]
---

# Phase 5 Plan 06: SDK lit facade Summary

SDK now exposes lit indexed meshes, textures, materials, directional light and world/asset/job building blocks with no GPU types, and the full workspace is green again.

## Tasks
1. Re-exports, SdkError World/Asset/Job variants, Resources::create_lit_mesh (validates indices before allocation, destroys vertex buffer on index-buffer failure) and create_texture, SceneState materials/light/add_material, with unit tests. Runtime two-pipeline/extract changes landed in the same commit (7f92ede) because they were required for the crate to compile.
2. tests/headless_lit.rs (lit frame, mixed two-pipeline scene) and facade re-export guard (commit after 7f92ede).

## Verification
build, test, clippy -D warnings, fmt --check, xtask check-deps, check-canary and cargo deny check all exit 0. headless.rs and triangle.rs untouched.

## Deviations
Runtime.rs changes were committed with Task 1 rather than Task 2 (compile dependency). Otherwise none.

## Self-Check: PASSED
