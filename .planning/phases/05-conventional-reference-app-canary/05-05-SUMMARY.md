---
phase: 05-conventional-reference-app-canary
plan: 05
subsystem: render
tags: [wgpu, lighting, textures, reverse-z, readback]
requires: [05-04]
provides: [lit textured indexed wgpu path, read_offscreen_rgba8]
affects: [engine-render-wgpu]
key-files:
  created:
    - crates/engine-render-wgpu/src/shaders/lit_textured.wgsl
    - crates/engine-render-wgpu/tests/gpu_lit.rs
  modified:
    - crates/engine-render-wgpu/src/pipeline.rs
    - crates/engine-render-wgpu/src/backend.rs
    - crates/engine-render-wgpu/src/shaders/unlit_vertex_color.wgsl
    - crates/engine-render-wgpu/tests/gpu_smoke.rs
    - crates/engine-render-wgpu/Cargo.toml
    - Cargo.lock
requirements: [REND-04]
metrics:
  completed: 2026-11-25
---

# Phase 5 Plan 05: Lit/textured/indexed wgpu path Summary

WgpuBackend now renders Lambert + ambient lit, textured, indexed draws (reverse-Z) with per-frame light, per-draw dynamic uniform and per-texture bind groups, proven by readback on an Intel Arc B580 (Vulkan).

## Commit

Tasks 1-3 cannot compile independently (backend.rs depends on the new pipeline API; gpu_smoke.rs on the new MeshHandle API), so they were committed as one unit: `e99d243`.

## Results

- `cargo test -p engine-render-wgpu --locked`: 11 lib tests pass; GPU tests ignored.
- `cargo test -p engine-render-wgpu --locked -- --ignored --nocapture`: adapter `Intel(R) Arc(TM) B580 Graphics (Vulkan)`; gpu_lit (5) and gpu_smoke (1) all pass.
- Observed pixels (sRGB): lit side [255,255,255,255], shaded side [89,89,89,255]; red texture center [255,0,0,255]; default-white + green tint [0,255,0,255]; reverse-Z nearer red wins [255,0,0,255]; mixed unlit tri and lit quad both [255,255,255,255]. Thresholds unchanged from the plan.
- clippy `--all-targets -D warnings`, `cargo fmt --check`, `cargo xtask check-deps`: pass.

## Deviations from Plan

1. [Rule 3 - Blocking] Added `glam.workspace = true` as a dev-dependency of engine-render-wgpu (tests need `Vec3` for `DirectionalLight.direction`; engine-render does not re-export Vec3). This changed Cargo.lock (one line), so `--locked` builds required the lock update first (done with `--offline`).
2. [Rule 3] `draw_uniform_packs_normal_columns` builds its `DrawCommand` via `NullBackend` because `PipelineId::from_raw`/`BufferId::from_raw` are `pub(crate)` in engine-render.
3. clippy `useless_conversion` on `InvalidTexture("...".into())` fixed (variant takes `&'static str`).
4. No wgpu 30 API names needed compiler-guided fixes; the research shapes compiled as listed.
5. Tasks 1-3 committed as a single commit (see above).

## Known Stubs

None.

## Self-Check: PASSED
