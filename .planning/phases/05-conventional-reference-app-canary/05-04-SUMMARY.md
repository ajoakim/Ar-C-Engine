---
phase: 05-conventional-reference-app-canary
plan: 04
subsystem: engine-render
tags: [render, lighting, materials, packet-validation]
requires: [05-03]
provides: [material/light-aware extract, PipelineSet, lit DrawCommand/FramePacket, lit packet validation]
affects: [engine-render-wgpu (05-05), engine-sdk (05-06)]
tech-stack:
  added: []
  patterns: [camera-relative world-axis lighting, layout-driven pipeline selection]
key-files:
  created: [crates/engine-render/tests/lit_packet.rs]
  modified: [crates/engine-render/src/scene.rs, crates/engine-render/src/graph.rs, crates/engine-render/src/backend.rs, crates/engine-render/src/resources.rs, crates/engine-render/src/lib.rs, crates/engine-render/tests/null_backend.rs, crates/engine-render/tests/precision.rs]
decisions:
  - Light and normals stay in world-axis camera-relative space (not multiplied by view)
metrics:
  tasks: 2
  completed: 2026-11-25
requirements: [REND-04]
---

# Phase 5 Plan 04: Lit frame path Summary

Materials, normal matrices and the directional light now flow `RenderItem -> extract -> RenderScene -> build_frame_packet -> FramePacket -> validate_packet`, with per-draw pipeline selection through `PipelineSet`.

## Tasks

1. Material/light-aware `extract` (7-param signature), `DrawItem{normal, material}`, `RenderScene.light`, `DrawCommand{model, normal, base_color, texture}`, `FramePacket.light`, `PipelineSet`, layout-driven `build_frame_packet`. Commit 101b70f.
2. `validate_packet` checks light, draw textures (live, not surface targets), finite model/normal/base_color/mvp; new `tests/lit_packet.rs` (7 tests); existing tests updated. Commit 9eadcfa.

## Verification

`cargo test -p engine-render --locked` passes (38 unit, 7 lit_packet, 18 null_backend, 5 precision). `cargo clippy -p engine-render --all-targets --locked -- -D warnings` and `cargo fmt --check` clean.

## Deviations from Plan

None in behavior. Added one extra test (mixed layouts submit on NullBackend). The missing-pipeline test reuses a real PipelineId from the rig because `PipelineId::from_raw` is crate-private.

## Known Stubs

None. engine-render-wgpu and engine-sdk are intentionally not compiling until 05-05/05-06 (not edited).

## Self-Check: PASSED
