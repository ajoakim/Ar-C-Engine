# Phase 3: Platform, Render Backend & Vertical Slice - Context

**Gathered:** 2026-11-25
**Status:** Ready for planning

<domain>
## Phase Boundary

A window shows geometry rendered through an engine-owned backend boundary, with camera-relative precision proven (REND-01, REND-02, REND-03, CORE-03, WORLD-03). Crates: engine-platform (winit), engine-render (backend trait, RenderScene, render graph, null backend), engine-render-wgpu (only crate allowed to use wgpu), engine-sdk (public facade).

</domain>

<decisions>
## Implementation Decisions

### Backend boundary
- Small `RenderBackend` trait over engine-owned handles (BufferId, PipelineId, TextureId) with `submit(FramePacket)`; no wgpu types in signatures.
- Null backend records submitted frames and validates handle use, for headless tests.
- wgpu 30.0.2, WGSL shaders, default features, auto-selected native backend.

### Frame contract
- `RenderScene` is built each frame in camera-relative f32 from f64 `WorldPos`, geometry-agnostic (draw items = mesh handle + transform).
- Minimal real render graph: named passes with declared read/write resources, topologically sorted; one pass in P0.
- Reverse-Z, Depth32Float, infinite far plane, per ADR-0001.

### Platform & SDK
- winit 0.30.13 only in `engine-platform`, with an adapter feeding engine `InputEvent`s.
- `engine-sdk` re-exports Engine, math types and a `run_app(App)` helper; apps never name wgpu or winit.

### Verification
- CPU-side precision test: camera at 6.4e6 m, assert camera-relative f32 positions stable to sub-millimeter.
- Headless tests use the null backend only; real-window smoke run is manual; software-adapter CI deferred to Phase 8.

### Claude's Discretion
Exact type/method names, error types, shader file layout, how the SDK example app is structured, what the triangle pass looks like.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- Phase 2: `Engine` driver (lifecycle, modules, tick returns FrameStats), `WorldPos`/`FrameRegistry`/rebase in engine-math, jobs, diagnostics.
- Phase 1: xtask check-deps layer table (update it if crate edges change) and deny.toml wrappers: wgpu only in engine-render-wgpu, winit only in engine-platform.

### Established Patterns
- Rust 1.99.0, edition 2024, clippy -D warnings, explicit versions in [workspace.dependencies], ADRs for decisions (next ADR number 0005), ADR-0001 conventions (RH Y-up, -Z forward, reversed-Z, [0,1] NDC).

### Integration Points
- engine-sdk currently a stub; engine-render-wgpu depends on engine-render; engine-platform depends on core.

</code_context>

<specifics>
## Specific Ideas

Write an ADR for the rendering API choice (wgpu behind trait) as part of this phase (WS-05 ADR for rendering API).

</specifics>

<deferred>
## Deferred Ideas

Software-adapter GPU CI and image-diff regression (Phase 8); lit meshes/materials/glTF (Phase 5).

</deferred>
