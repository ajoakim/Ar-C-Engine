---
phase: 05-conventional-reference-app-canary
verified: 2026-11-25T00:00:00Z
status: passed
score: 4/4 must-haves verified (automated); windowed visual check pending
gaps: []
human_verification:
  - test: "Run `cargo run -p conventional-3d --example basic_scene` (windowed)"
    expected: "Lit, textured cubes with directional shading and correct depth ordering (reverse-Z); WASD moves, RMB-drag looks, Shift speeds up, Esc exits"
    why_human: "Interactive window, visual shading/depth and input feel cannot be checked programmatically. Status: human_needed"
  - test: "Hosted CI run of .github/workflows/ci.yml (incl. `canary` job)"
    expected: "All jobs green on GitHub Actions"
    why_human: "Hosted CI has never run (GitHub billing). The canary job is defined but unexecuted remotely; local equivalents pass."
---

# Phase 5: Conventional Reference App (Canary) Verification Report

**Goal:** A conventional 3D scene runs on the SDK alone, proving the core is general-purpose.
**Status:** human_needed (no code gaps found)

## Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Lit, textured, materialed meshes + directional light + reverse-Z | VERIFIED (visual pending) | `crates/engine-render-wgpu/src/shaders/lit_textured.wgsl` computes `max(dot(n,-light_dir),0)` with ambient/light color times albedo; `pipeline.rs:188` reverse-Z `Greater` depth state with unit test `pipeline_depth_state_is_reverse_z`; Material/light/texture contract in engine-render; tests pass |
| 2 | glTF meshes/textures loaded from disk | VERIFIED | `extensions/conventional-3d/src/import.rs` (gltf+image) with `ImportLimits` hardening (file bytes, texture dim, image alloc, nodes, depth, vertices, indices); `loader.rs` ModelLoader on worker threads; fixture `assets/textured_cube.glb`; tests gltf_import, model_loader pass |
| 3 | Fly camera using only SDK APIs | VERIFIED | `fly_camera.rs` imports only `engine_sdk::{Camera, InputEvent, KeyCode, MouseButton,...}`; pitch clamp; `tests/fly_camera.rs` passes; `tests/sdk_only.rs` enforces manifest deps (engine-sdk, gltf, image, thiserror only) and source-use roots |
| 4 | CI job builds/runs app with planetary crates absent | VERIFIED locally (hosted unrun) | `ci.yml` `canary` job removes `extensions/planetary`, strips workspace member, builds, tests, clippy, runs `basic_scene -- --headless 5`, and runs check-canary. Also a transitive-closure `check-canary` step plus negative leak fixture in main job |

**Score:** 4/4

## Spot-Checks Run

| Check | Result |
|-------|--------|
| `cargo test -p conventional-3d -p engine-sdk -p engine-render --locked` | All suites ok, 0 failures (1 ignored) |
| `xtask check-deps` | OK (14 packages) |
| `xtask check-canary` | OK (conventional-3d reaches 287 packages, none planetary) |

## Requirements

| Req | Status | Evidence |
|-----|--------|----------|
| REND-04 | SATISFIED (visual pending human) | Truth 1 |
| REND-05 | SATISFIED | Truth 2 |
| APP-01 | SATISFIED | Truth 3, sdk_only policy test |
| PLAN-01 | SATISFIED locally; hosted CI unrun | Truth 4, check-canary |

## Notes
- GPU tests (Arc B580) and the reduced-workspace canary job steps were not re-executed here (disk constraints; only targeted checks run). Windowed example deliberately not launched.
- REQUIREMENTS.md/ROADMAP.md checkboxes for Phase 5 still unchecked; update after human sign-off.

## Gaps Summary
No gaps. Remaining: human windowed check and a first hosted CI run.

_Verifier: Claude (gsd-verifier)_


## Human Verification Result

Window run confirmed by the user on 2026-11-26: everything behaves as described (window opens, geometry renders, input received, lit/textured cubes, shading, depth, WASD/RMB-look/Shift/Esc controls). Only hosted CI (GitHub Actions, blocked by billing) remains unrun.
