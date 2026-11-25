---
phase: 03-platform-render-backend-vertical-slice
plan: 05
subsystem: render
tags: [wgpu, wgsl, reverse-z, surface]
requires: [03-02]
provides: [WgpuBackend, DRAW_UNIFORM_STRIDE]
affects: [03-06]
key-files:
  created:
    - crates/engine-render-wgpu/src/target.rs
    - crates/engine-render-wgpu/src/pipeline.rs
    - crates/engine-render-wgpu/src/backend.rs
    - crates/engine-render-wgpu/src/shaders/unlit_vertex_color.wgsl
    - crates/engine-render-wgpu/tests/gpu_smoke.rs
  modified:
    - crates/engine-render-wgpu/src/lib.rs
requirements-completed: [REND-01]
completed: 2026-11-25
---

# Phase 3 Plan 05: WgpuBackend Summary

WgpuBackend implements engine-render's RenderBackend over wgpu 30.0.2 with a WGSL vertex-color pipeline, Depth32Float reverse-Z (clear 0.0, compare Greater), shared ResourceTable validation, and non-panicking surface handling; no wgpu types are public.

## Commits
- a37607c: target newtype, WGSL shader, pipeline helpers (4 pure tests)
- 8513500: WgpuBackend + ignored GPU smoke test

## Verification
- `cargo test -p engine-render-wgpu --locked`: 4 passed, 1 ignored
- clippy `-D warnings`, `cargo xtask check-deps`, `cargo deny check bans`: pass
- `cargo test -p engine-render-wgpu -- --ignored`: PASSED on Intel(R) Arc(TM) B580 Graphics (Vulkan); two submits, resize 32x32, resize 0x0 skip.

## wgpu 30 API differences found (vs plan)
- `VertexState.buffers` is `&[Option<VertexBufferLayout>]` (wrap in `Some`).
- `PipelineLayoutDescriptor.bind_group_layouts` is `&[Option<&BindGroupLayout>]` and has `immediate_size: 0` (replaces push constant ranges).
- `DepthStencilState.depth_write_enabled` and `depth_compare` are `Option<_>`.
- `SurfaceConfiguration` has a new `color_space: SurfaceColorSpace` field (use `Auto`).
- Presenting is `queue.present(surface_texture)`, not `SurfaceTexture::present()`.
- `bind_group_layouts`, `multiview_mask`, `cache`, `depth_slice`, `CurrentSurfaceTexture`, `InstanceDescriptor::new_without_display_handle()` matched the plan.

## Decisions
- Surface Lost: recreate from stored target; Outdated: reconfigure and skip frame; Suboptimal: render, present, reconfigure.
- Pipeline built on GPU before allocating the table id, so failure leaves no dangling entry.
- Per-draw uniforms use one 64-byte dynamic-offset slot per draw at 256-byte stride.

## Deviations from Plan
None of substance. Note: `new()` surface path, Lost/Outdated recovery and window presentation are not exercised by the headless smoke test (no window); they are compile-verified only and get real coverage in plan 03-06.

## Known Stubs
None.

## Self-Check: PASSED
