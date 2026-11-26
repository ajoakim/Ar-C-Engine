---
phase: 05-conventional-reference-app-canary
plan: 09
subsystem: conventional-3d
tags: [scene-builder, upload, basic_scene, headless, app-01]
requires: [05-06, 05-07, 05-08]
provides: [upload_model, upload_registered, SceneBuilder, basic_scene example, sdk_only policy test]
key-files:
  created:
    - extensions/conventional-3d/src/scene_builder.rs
    - extensions/conventional-3d/examples/basic_scene.rs
    - extensions/conventional-3d/tests/headless_scene.rs
    - extensions/conventional-3d/tests/sdk_only.rs
  modified:
    - extensions/conventional-3d/src/lib.rs
requirements: [APP-01, REND-04, REND-05]
completed: 2026-11-25
---

# Phase 5 Plan 09: Conventional reference app Summary

SDK-only `basic_scene` (lit textured glTF grid, directional light, fly camera) with a World-backed `SceneBuilder`, runnable headless on NullBackend, plus an APP-01 source/manifest policy test.

## Commits
- 463c7ae: upload_model, upload_registered, SceneBuilder, headless_scene tests
- e30784a: basic_scene example and sdk_only test

## Notes
- Per plan review: the example routes the imported model through `ModelAssets::register` and uploads via the new `upload_registered` (resolves `RegisteredModel` handles). Added `SceneBuilder::spawn_nodes(&[ImportedNode], ..)` (used by spawn_model) so registered nodes can be placed.
- Headless run: `basic_scene headless: 5 frames, 10 draws in last frame, 5 submits`. The windowed run was NOT launched (plan 05-10 checkpoint).
- Tests: headless_scene 5 pass (incl. registry path, f64 1.0e6 hierarchy, bad-index rejection), sdk_only 4 pass.

## Deviations from Plan
- [Rule 2] Added `upload_registered` and `spawn_nodes` (review note requirement); not in the original interface.

## Gates
build, test, clippy -D warnings, fmt --check, xtask check-deps, check-canary, cargo deny check: all exit 0.

## Self-Check: PASSED
