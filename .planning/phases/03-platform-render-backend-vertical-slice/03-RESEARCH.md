# Phase 3: Platform, Render Backend & Vertical Slice - Research

**Researched:** 2026-11-25
**Domain:** Rust windowing (winit 0.30), GPU backend (wgpu 30), engine-owned render boundary, camera-relative precision
**Confidence:** HIGH on stack/boundary/deny (compiled and cargo-deny checked in a scratch crate); MEDIUM on render-graph and shader details (design, not external fact)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Small `RenderBackend` trait over engine-owned handles (BufferId, PipelineId, TextureId) with `submit(FramePacket)`; no wgpu types in signatures.
- Null backend records submitted frames and validates handle use, for headless tests.
- wgpu 30.0.2, WGSL shaders, default features, auto-selected native backend.
- `RenderScene` is built each frame in camera-relative f32 from f64 `WorldPos`, geometry-agnostic (draw items = mesh handle + transform).
- Minimal real render graph: named passes with declared read/write resources, topologically sorted; one pass in P0.
- Reverse-Z, Depth32Float, infinite far plane, per ADR-0001.
- winit 0.30.13 only in `engine-platform`, with an adapter feeding engine `InputEvent`s.
- `engine-sdk` re-exports Engine, math types and a `run_app(App)` helper; apps never name wgpu or winit.
- CPU-side precision test: camera at 6.4e6 m, assert camera-relative f32 positions stable to sub-millimeter.
- Headless tests use the null backend only; real-window smoke run is manual; software-adapter CI deferred to Phase 8.

### Claude's Discretion
Exact type/method names, error types, shader file layout, how the SDK example app is structured, what the triangle pass looks like.

### Deferred Ideas (OUT OF SCOPE)
Software-adapter GPU CI and image-diff regression (Phase 8); lit meshes/materials/glTF (Phase 5).
Also in scope per specifics: write an ADR (next number 0005) for the rendering API choice (wgpu behind trait).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REND-01 | Engine-owned backend boundary, wgpu first impl, null backend | Trait + handle design, null backend validation, `RenderTarget` rwh abstraction, deny/xtask confinement |
| REND-02 | Window + input via platform layer | winit 0.30.13 `ApplicationHandler` adapter -> engine `InputEvent` |
| REND-03 | Geometry-agnostic camera-relative `RenderScene` + render graph | Extraction from `WorldPos` via `FrameRegistry::camera_relative_f32`, topo-sorted graph |
| CORE-03 | Minimal public SDK facade | `engine-sdk` re-exports + `run_app` |
| WORLD-03 | Camera-relative f32 transforms; test at 6.4e6 m | CPU precision test design, reverse-Z infinite projection |
</phase_requirements>

## Summary

All four required external stacks were verified live. A scratch crate (outside the repo, toolchain 1.99.0) compiled wgpu 30.0.2 + winit 0.30.13 + raw-window-handle 0.6.2 + pollster, created a winit window, built an `Arc<dyn RenderTarget>` abstraction that wgpu accepted for `create_surface`, and `request_adapter` succeeded on the dev machine (Intel Arc B580, Vulkan) [VERIFIED: local run]. `cargo deny check licenses bans` with the repo's `deny.toml` reported NO license failures for the full wgpu+winit tree (251 packages) and only the expected `banned` hits on wgpu/winit (which the `wrappers` entries whitelist for the two engine crates) plus duplicate-version warnings (hashbrown, rustix, syn, thiserror, windows-sys, etc.) which are `warn` level [VERIFIED: local cargo-deny 0.22.1].

The central design problem, owning a window across the platform/render-wgpu boundary without leaking winit or wgpu, is solved with `raw-window-handle` 0.6 as a neutral vocabulary crate. `engine-platform` exposes a window type that implements `HasWindowHandle + HasDisplayHandle + Send + Sync`; `engine-render` defines `trait RenderTarget` (blanket-implemented for anything with those traits) and the backend's `create_surface` takes `Arc<dyn RenderTarget>`; `engine-render-wgpu` wraps it in a delegating newtype that wgpu accepts. No new crate edges and no tier changes are needed; `raw-window-handle` is not banned and has no wgpu/winit types.

The workspace has `unsafe_code = "forbid"`; every API used here (safe `create_surface` with an owned `Arc` target) needs no unsafe. wgpu 30 has API changes versus older knowledge (see State of the Art) and must be coded to 30.0.2 docs, not memory.

**Primary recommendation:** Add `raw-window-handle` to `engine-render` (trait) and `engine-platform` (impls), `wgpu`+`pollster`+`bytemuck` to `engine-render-wgpu`, `winit` to `engine-platform`; keep the xtask layer table unchanged; relax nothing in deny.toml except (optionally) adding `multiple-versions` skips later.

## Architectural Responsibility Map

| Capability | Primary Tier (crate) | Secondary | Rationale |
|------------|---------------------|-----------|-----------|
| Window, event loop, input translation | engine-platform (T2) | engine-sdk drives it | only crate allowed winit |
| Window handle abstraction (`RenderTarget`) | engine-render (T3) | engine-platform implements via blanket | neutral rwh traits, no GPU types |
| RenderScene extraction (f64->f32 camera-relative) | engine-render | engine-math (`camera_relative_f32`) | geometry-agnostic, backend-free |
| Render graph (declare/sort passes) | engine-render | backend executes | backend-agnostic ordering logic |
| Backend trait, null backend | engine-render | — | testable headless |
| wgpu device/surface/pipelines/WGSL | engine-render-wgpu (T4) | — | only crate allowed wgpu |
| App loop wiring (`run_app`), re-exports | engine-sdk (T5) | — | only crate with both platform and wgpu backend as deps |
| Precision test | engine-render tests | engine-math | CPU only |

## Standard Stack

### Core
| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| wgpu | 30.0.2 (published 2026-11-12) | GPU API | default features (std, dx12, metal, gles, vulkan, wgsl, webgpu) [VERIFIED: crates.io API] |
| winit | 0.30.13 (2026-03-02) | windowing | stable line; crates.io newest is 0.31.0-beta.4 which must NOT be picked up (caret `0.30.13` stays < 0.31) [VERIFIED: crates.io] |
| raw-window-handle | 0.6.2 | window handle vocabulary | wgpu 30 depends on `^0.6.2`; winit 0.30 implements 0.6 traits [VERIFIED: crates.io deps + compile] |
| pollster | 1.0.3 (latest; 0.4.0 also compiled) | block_on for adapter/device futures | tiny, MIT/Apache; use only in engine-render-wgpu [VERIFIED: cargo search] |
| bytemuck | 1.25.2 (features `derive`) | vertex/uniform byte casting | avoids `unsafe` (workspace forbids it); wgpu already depends on it [VERIFIED] |
| glam | 0.34.3 (existing) | `Mat4::perspective_infinite_reverse_rh`, `look_to_rh` | already workspace dep via engine-math |

Add these to `[workspace.dependencies]` with explicit versions (project convention). thiserror/tracing already present for errors/logging.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| pollster | `wgpu` future via `futures::executor::block_on` | extra dep; pollster is smaller |
| winit 0.31 beta | — | beta, API different; locked to 0.30.13 |
| rwh abstraction | engine-render-wgpu depends on engine-platform | allowed by tiers but couples backend to platform; rwh keeps them independent |

**Installation (workspace Cargo.toml):**
```toml
wgpu = "30.0.2"
winit = "0.30.13"
raw-window-handle = "0.6.2"
pollster = "1.0.3"
bytemuck = { version = "1.25.2", features = ["derive"] }
```
Crate usage: `engine-platform`: winit, raw-window-handle. `engine-render`: raw-window-handle, glam via engine-math, thiserror. `engine-render-wgpu`: wgpu, pollster, bytemuck, raw-window-handle. `engine-sdk`: no new third-party deps expected.

## Architecture Patterns

### System Architecture Diagram

```
OS events --> winit EventLoop (engine-platform)
                |  ApplicationHandler: resumed/window_event/about_to_wait
                v  translate -> engine InputEvent (own enum)
         engine-sdk::run_app(App)
                |  per redraw: input -> App::update -> Engine::tick(dt)
                v
        App builds scene state (camera WorldPos f64, items WorldPos + mesh handle)
                |  extract(): FrameRegistry::camera_relative_f32(pos, camera)
                v
        RenderScene { camera view/proj (f32), draws: [(MeshHandle, Mat4 f32)] }
                |  build_graph(): named passes + read/write resources -> topo sort
                v
        FramePacket { ordered passes, draw lists }
                |  RenderBackend::submit(packet)
        +-------+-----------------+
        v                         v
   NullBackend                WgpuBackend (engine-render-wgpu)
   records packets,           surface acquire -> encoder -> render pass
   validates handle use       (Depth32Float, clear 0.0, Greater) -> present
```

### Recommended Project Structure
```
crates/engine-platform/src/{lib.rs, input.rs (InputEvent, KeyCode, MouseButton), window.rs (PlatformWindow), app_loop.rs (run via winit)}
crates/engine-render/src/{lib.rs, handles.rs, backend.rs (trait, FramePacket, RenderTarget), scene.rs (RenderScene, extract), graph.rs, null.rs, camera.rs}
crates/engine-render-wgpu/src/{lib.rs, backend.rs, surface.rs, pipeline.rs, shaders/triangle.wgsl (include_str!)}
crates/engine-sdk/src/{lib.rs (re-exports), app.rs (App trait, run_app)}
crates/engine-sdk/examples/triangle.rs
docs/architecture/adr/0005-rendering-api-wgpu-behind-trait.md
```

### Pattern 1: Cross-boundary window ownership (verified to compile against wgpu 30.0.2 + winit 0.30.13)
```rust
// engine-render
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
pub trait RenderTarget: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static {}
impl<T: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static> RenderTarget for T {}

// engine-render-wgpu: delegating newtype (wgpu needs DisplayAndWindowHandle: Sized)
struct Target(Arc<dyn RenderTarget>);
impl HasWindowHandle for Target { fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> { self.0.window_handle() } }
impl HasDisplayHandle for Target { fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> { self.0.display_handle() } }
// Debug impl only needed if used with InstanceDescriptor display-handle variants.

let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
let surface = instance.create_surface(Target(target))?;          // Surface<'static>
let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
    compatible_surface: Some(&surface), ..Default::default() }))?;  // Result<Adapter, RequestAdapterError>
let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
```
engine-platform: `pub struct PlatformWindow(Arc<winit::window::Window>)` implementing the two rwh traits by delegation (winit `Window` implements them), or hand out `Arc<Window>` wrapped; the winit type never appears in public signatures. Surface lifetime `'static` because the target is owned by the surface. Source: docs.rs/wgpu/30.0.2 Instance/SurfaceTarget; local compile.

Note: `new_without_display_handle` was used and ran; on Wayland/GLES-on-X11 a display handle on the instance may matter. Use `InstanceDescriptor::new_with_display_handle_from_env(Box<dyn ...>)` (requires the handle type to be `Debug`; give `Target` a manual `Debug`) if Linux support is exercised later [VERIFIED compile error message required Debug]. Windows is the dev target; flag for Phase 8.

### Pattern 2: winit 0.30 ApplicationHandler
Create the window inside `resumed(&mut self, &ActiveEventLoop)` via `event_loop.create_window(Window::default_attributes().with_title(..))` (the old `EventLoop::create_window` is deprecated). Use `ControlFlow::Poll` or `request_redraw()` in `about_to_wait`; render on `WindowEvent::RedrawRequested`; handle `Resized` (reconfigure surface), `CloseRequested` (exit), `KeyboardInput` (`event.physical_key` -> `PhysicalKey::Code`), `CursorMoved`, `MouseInput`. Driver: `EventLoop::new()?.run_app(&mut handler)` blocks the main thread; create it only on the main thread. [CITED: winit 0.30 docs; compile verified for ApplicationHandler/resumed/window_event/create_window]

Translate into an engine-owned `InputEvent` (key down/up with engine `KeyCode`, mouse button, cursor position, resize, close). Keep a small `KeyCode` subset (WASD, arrows, Escape, Space) plus `Other(u32)` to avoid a huge mirror enum.

### Pattern 3: Backend trait with engine-owned handles
```rust
pub struct BufferId(u32); pub struct PipelineId(u32); pub struct TextureId(u32); // generational optional; plain u32 + Copy
pub trait RenderBackend {
    fn create_buffer(&mut self, desc: &BufferDesc, data: &[u8]) -> Result<BufferId, RenderError>;
    fn create_pipeline(&mut self, desc: &PipelineDesc) -> Result<PipelineId, RenderError>; // desc names a built-in shader by enum/&'static str; WGSL source lives in the wgpu crate
    fn resize(&mut self, width: u32, height: u32);
    fn submit(&mut self, packet: &FramePacket) -> Result<(), RenderError>;
}
```
Rule: shader source and vertex formats are described by engine enums (`VertexLayout::Pos3Color3`, `ShaderId::Triangle`), so the null backend and the wgpu backend share validation. NullBackend stores `Vec<RecordedFrame>`, rejects unknown/stale ids with `RenderError::InvalidHandle`. Make surface creation a constructor on the concrete `WgpuBackend::new(target: Arc<dyn RenderTarget>, size)` (not on the trait) so the trait stays surface-free.

### Pattern 4: Camera-relative extraction
Use existing `FrameRegistry::camera_relative_f32(pos, camera) -> Result<Vec3, MathError>` for each item (f64 subtract then cast). Camera view matrix is rotation only (`Mat4::look_to_rh(Vec3::ZERO, forward, up)` or quat inverse) since the camera is at the origin of camera space; projection `Mat4::perspective_infinite_reverse_rh(fov_y_rad, aspect, z_near)` [glam; ADR-0001]. Item model matrix = translation(rel_pos) * rotation/scale; upload only f32. `RenderScene` stores `camera: CameraData {view_proj: Mat4}` and `draws: Vec<DrawItem{mesh: MeshHandle, transform: Mat4}>`; add `frame_index`.

### Pattern 5: Minimal render graph
`RenderGraph::add_pass(name, reads: &[ResourceName], writes: &[ResourceName])`, `compile() -> Result<Vec<PassIndex>, GraphError>` using Kahn's algorithm with deterministic tie-break by insertion order; errors: cycle, read of never-written non-external resource, duplicate pass name. External resources (`backbuffer`, `depth`) declared as imports. P0 has one pass `main` (writes backbuffer + depth). Pure data, fully unit-testable headless. Do not use a graph library (petgraph would be overkill; ~50 lines).

### Anti-Patterns
- wgpu/winit types in any `pub` signature outside their crates (also caught by xtask R2/R3 only for declared deps, so reviewers must watch re-exports).
- Making `engine-sdk` re-export `wgpu`/`winit` or rwh types in app-facing API.
- Computing view-proj in f64 world space then casting (re-introduces jitter): subtract camera first.
- Using the non-reversed `perspective_rh` or clearing depth to 1.0.
- Creating the window outside `resumed` (panics/invalid on some platforms).
- `unsafe` to create surfaces (`create_surface_unsafe`); not needed.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Window/event loop | raw Win32 | winit 0.30.13 | locked |
| Future blocking | custom executor | pollster | trivial but subtle wakers |
| Byte casting of vertices | `unsafe` transmute (forbidden) | bytemuck derive | safe |
| Reverse-Z projection | hand matrix | `Mat4::perspective_infinite_reverse_rh` | correct, tested |
| Surface handle plumbing | custom FFI | raw-window-handle traits | wgpu consumes it natively |

## Runtime State Inventory
Not a rename/refactor phase: omitted.

## Common Pitfalls

### Pitfall 1: cargo-deny `banned` errors for wgpu/winit
**What goes wrong:** Verified: with `wrappers = ["engine-render-wgpu"]` the ban only passes when the DIRECT parent of wgpu is exactly that crate. **Prevention:** Only `engine-render-wgpu` declares `wgpu`, only `engine-platform` declares `winit`; no other crate may use `wgpu-types`/`wgpu-core` directly (they are not banned, but don't). Expect the two `unused-wrapper` warnings to disappear once crates are added [VERIFIED: scratch deny output showed `unmatched-wrapper`/`banned` for a crate not in the wrapper list]. Run `cargo deny check` in the real workspace as a task step; `all-features = true` is set in `deny.toml` graph and may widen the tree (e.g., wgpu optional features); if new license errors appear, add the specific SPDX id rather than loosening elsewhere.
**Licenses:** the 251-package tree passed the existing allow list with zero license errors (scratch crate itself only complained for lacking a license). No `deny.toml` license changes expected. [VERIFIED]

### Pitfall 2: multiple-versions noise
Duplicates found: hashbrown, linux-raw-sys, rustix, syn, thiserror(+impl, 1.x vs workspace 2.x), windows-sys (x3). `multiple-versions = "warn"` keeps them non-fatal; the existing comment says "revisit after Phase 3": decide to leave at warn and add `skip` entries only if noise is a problem (recommend: leave warn, record in ADR-0005). Wildcards unaffected (`allow-wildcard-paths`).

### Pitfall 3: Surface texture status handling (wgpu 30)
`Surface::get_current_texture()` now returns `CurrentSurfaceTexture` (not a `Result`): variants `Success(t)`, `Suboptimal(t)` (present then reconfigure), `Timeout`/`Occluded` (skip frame), `Outdated` (reconfigure, retry), `Lost` (recreate surface+configure), `Validation` [CITED: docs.rs/wgpu/30.0.2/wgpu/enum.CurrentSurfaceTexture.html]. Handle all; never `unwrap`. Minimized window gives zero size: skip configure/render when width or height is 0.

### Pitfall 4: Depth buffer must be recreated on resize
Recreate Depth32Float texture and view with surface size; the pipeline's `DepthStencilState { format: Depth32Float, depth_write_enabled, depth_compare: Greater }`, clear value `0.0` (reverse-Z). Triangle at z in (0,1] NDC must pass `Greater` against 0.0.

### Pitfall 5: Precision test that proves nothing
f32 spacing at 6.4e6 is 0.5 m. A naive "cast world pos to f32" test would show 0.5 m error; the test must (a) assert naive f32 cast of world pos differs by >= ~0.25 m (demonstrates the problem) and (b) assert extraction via `camera_relative_f32` of points within e.g. +-100 m of a camera at 6.4e6 m matches the exact f64 delta to < 1 mm, and is stable (bitwise identical) when the camera moves by sub-mm amounts across many frames (no frame-to-frame jitter beyond ~1e-5 m). Also transform a vertex through view-proj and assert the projected NDC is unchanged between camera at origin and camera at 6.4e6 (same relative scene) within epsilon. Pure CPU, in engine-render tests.

### Pitfall 6: winit/wgpu on the main thread and `Engine: Send` modules
`EventLoop::run_app` must run on the main thread; the `Engine` driver has `Module: Send` modules but wgpu backend and window are owned by the SDK app loop, not registered as modules (keep wgpu backend out of `Module`, simply driven by `run_app`). Tick order per redraw: input drain -> `App::update` -> `Engine::tick(dt)` -> extract -> submit.

### Pitfall 7: Clippy/edition/unsafe
`unsafe_code = forbid` applies to the new crates (workspace lints). wgpu `include_str!` for WGSL needs no unsafe. Workspace `rust-version = "1.95"` is below toolchain 1.99.0; wgpu 30 MSRV is lower than 1.95 [ASSUMED]; ok.

### Pitfall 8: Window-less environments
`winit::EventLoop::new()` and adapter requests fail on headless CI (no display / no adapter). All automated tests must avoid both; gate any real-GPU test behind `#[ignore]` or an env var (`ENGINE_GPU_TESTS=1`).

## Code Examples

### WGSL triangle (reverse-Z, camera-relative uniform)
```wgsl
struct Globals { view_proj: mat4x4<f32> };
@group(0) @binding(0) var<uniform> globals: Globals;
struct VsIn { @location(0) pos: vec3<f32>, @location(1) color: vec3<f32> };
struct VsOut { @builtin(position) clip: vec4<f32>, @location(0) color: vec3<f32> };
@vertex fn vs_main(i: VsIn) -> VsOut { var o: VsOut; o.clip = globals.view_proj * vec4(i.pos, 1.0); o.color = i.color; return o; }
@fragment fn fs_main(i: VsOut) -> @location(0) vec4<f32> { return vec4(i.color, 1.0); }
```
Per-draw model transform: push into a second uniform/storage buffer or bake `view_proj * model` on CPU into a per-draw uniform (simplest for P0: one dynamic uniform per frame; single triangle).

### Render pass depth attachment (wgpu 30)
`RenderPassDescriptor { label, color_attachments, depth_stencil_attachment: Some(..{view, depth_ops: Some(Operations{ load: Clear(0.0), store: Store }), stencil_ops: None}), timestamp_writes: None, occlusion_query_set: None, multiview_mask: None }` (note `multiview_mask` field exists in 30) [CITED: docs.rs RenderPassDescriptor]. Prefer compiler-guided fixes for remaining field names (`RenderPipelineDescriptor` also has multiview/cache fields) since only some descriptor pages were retrievable; code against 30.0.2 with `..Default::default()` where available. [MEDIUM]

### Headless frame test
```rust
let mut be = NullBackend::new();
let scene = extract(&frames, camera, &items)?;     // uses camera at 6.4e6
let packet = graph.compile()?.build_packet(&scene);
be.submit(&packet)?;
assert_eq!(be.frames().len(), 1);
```

## State of the Art (wgpu 30 vs older knowledge)

| Old | Current (30.0.2) |
|-----|------------------|
| `request_adapter` -> `Option<Adapter>` | `Result<Adapter, RequestAdapterError>` |
| `request_device(desc, trace_path)` | `request_device(&DeviceDescriptor)` -> `Result<(Device, Queue), _>` |
| `get_current_texture() -> Result<SurfaceTexture, SurfaceError>` | returns `CurrentSurfaceTexture` enum (7 variants) |
| `Instance::new(&desc)` / `default()` | `Instance::new(InstanceDescriptor)` by value; `InstanceDescriptor::new_without_display_handle()` / `new_with_display_handle_from_env(..)`; display handle can be supplied at instance creation |
| `create_surface(Arc<Window>)` | `create_surface(impl Into<SurfaceTarget>)`; `SurfaceTarget::DisplayAndWindow` / `Window` (`from_window_without_display`) |
| RenderPassDescriptor w/o multiview | has `multiview_mask: Option<NonZeroU32>` |
| winit 0.29 closure event loop | 0.30 `ApplicationHandler` trait + `run_app` |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | wgpu 30 MSRV is below workspace rust-version 1.95 (toolchain is 1.99 so builds fine) | Pitfall 7 | None practically |
| A2 | Exact wgpu 30 `RenderPipelineDescriptor`/`RenderPassColorAttachment` field names beyond those cited (`depth_slice`, `cache`, `multiview_mask`) | Code examples | Compile-fix during execution; low |
| A3 | `pollster` 1.0.3 works with wgpu 30 futures (0.4.0 was compile/run verified; 1.0 is a trait-compatible `block_on`) | Stack | Fall back to 0.4.0 |
| A4 | Linux Wayland needs display handle at instance creation | Pattern 1 | Phase 8 concern only |

## Open Questions (all RESOLVED)

1. **How does a winit window reach wgpu without leaking types?** RESOLVED: `RenderTarget` trait (rwh 0.6) in engine-render; `Arc<dyn RenderTarget>` passed to `WgpuBackend::new`; newtype delegation in wgpu crate. Compiled and ran.
2. **Does engine-render-wgpu need an engine-platform dependency / layer table change?** RESOLVED: No. xtask `layers.rs` and `dependency-direction.md` stay unchanged; confirm `cargo xtask check-deps` still passes. If a plan nonetheless adds an edge, update `layers.rs`, the doc table and run `cargo test -p xtask`.
3. **deny.toml changes?** RESOLVED: no license additions needed; wrappers already correct; keep multiple-versions=warn. The `raw-window-handle` crate is allowed anywhere.
4. **Local GPU availability / CI?** RESOLVED: dev machine has an Intel Arc B580 (Vulkan) and a display; CI is unknown, so automated tests use only NullBackend and CPU math; real-window run is a manual `cargo run -p engine-sdk --example triangle`; GPU tests `#[ignore]`/env-gated.
5. **Where does depth/projection live?** RESOLVED: camera data in `RenderScene` (CPU, engine-render); the wgpu crate only applies Depth32Float/Greater/clear 0.0.
6. **Render graph size?** RESOLVED: ~50-line Kahn sort, no dependency.
7. **Wgpu backend as engine Module?** RESOLVED: no; owned by `run_app` loop (main-thread, non-Send resources).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | build | yes | 1.99.0 (pinned) | — |
| cargo-deny | deny check | yes | 0.22.1 | — |
| Display + Vulkan GPU (dev box) | manual smoke | yes | Intel Arc B580, Vulkan | NullBackend tests only |
| GPU in CI | automated render | unknown/no | — | NullBackend; software adapter Phase 8 |
| crates.io network | fetching deps | yes | — | — |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | built-in `cargo test` (existing workspace; xtask tests) |
| Config file | none needed |
| Quick run command | `cargo test -p engine-render -p engine-platform` |
| Full suite command | `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo xtask check-deps && cargo deny check` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REND-01 | NullBackend records frame; rejects invalid handle | unit | `cargo test -p engine-render null` | Wave 0 |
| REND-01 | wgpu/winit not outside crates | static | `cargo xtask check-deps && cargo deny check bans` | exists |
| REND-01 | wgpu backend constructs & draws | manual/ignored | `cargo test -p engine-render-wgpu -- --ignored` (needs GPU) | Wave 0 |
| REND-02 | Winit key/mouse -> InputEvent mapping (pure fn on translated data) | unit | `cargo test -p engine-platform input` | Wave 0 |
| REND-02 | Window opens and receives input | manual | `cargo run -p engine-sdk --example triangle` | Wave 0 |
| REND-03 | extract() output camera-relative, geometry-agnostic | unit | `cargo test -p engine-render extract` | Wave 0 |
| REND-03 | graph topo sort, cycle/missing-resource errors | unit | `cargo test -p engine-render graph` | Wave 0 |
| WORLD-03 | camera at 6.4e6 m, sub-mm stability, naive f32 contrast | unit | `cargo test -p engine-render --test precision` | Wave 0 |
| CORE-03 | example/test compiles using only `engine-sdk` (no wgpu/winit in its deps); headless frame via null backend | integration | `cargo test -p engine-sdk` | Wave 0 |

### Sampling Rate
- Per task commit: `cargo test -p <touched crate>` plus clippy for it
- Per wave merge: full suite command
- Phase gate: full suite green + manual triangle smoke before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `crates/engine-render/tests/precision.rs`, `null_backend.rs`, graph/extract unit tests
- [ ] `crates/engine-platform` input-translation tests (translate from an engine-side intermediate so no window is needed)
- [ ] `crates/engine-sdk/tests/headless.rs`: SDK `run_frame` path with `NullBackend`
- [ ] workspace deps added and `cargo deny check` run on the real workspace

## Security Domain

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2/V3/V4 Auth/Session/Access | no | local engine, no network |
| V5 Input Validation | yes (limited) | validate handle ids, buffer sizes, zero-size surface; typed errors, no `unwrap` on GPU results |
| V6 Cryptography | no | — |
| V14 Config/Supply chain | yes | cargo-deny advisories/licenses/sources already enforced; pin versions; `unsafe_code = forbid` |

### Known Threat Patterns
| Pattern | STRIDE | Mitigation |
|---------|--------|------------|
| Dependency vulnerability/yanked | Tampering | `cargo deny check advisories` (run once after adding tree; network DB) |
| Invalid handle/OOB buffer use | DoS | null+wgpu backends validate ids and sizes, return `RenderError` |
| Unsafe code via GPU interop | Tampering | no `unsafe`; owned `Arc` surface target |

## Sources

### Primary (HIGH)
- Local scratch crate: compiled and ran wgpu 30.0.2 + winit 0.30.13 + rwh 0.6.2 + pollster; adapter enumerated (Intel Arc B580, Vulkan)
- Local `cargo deny check licenses bans` with repo deny.toml on that tree
- crates.io API: wgpu 30.0.2 (features, deps incl. raw-window-handle ^0.6.2), winit 0.30.13, `cargo search` for pollster/bytemuck/rwh versions
- docs.rs/wgpu/30.0.2: Instance, Surface, Adapter, SurfaceTarget, CurrentSurfaceTexture, RenderPassDescriptor
- Repo: ADR-0001, deny.toml, xtask/src/layers.rs, dependency-direction.md, Phase 2 engine code

### Secondary / Tertiary
- None relied upon; unretrievable docs.rs pages (wgpu-types InstanceDescriptor 404, pipeline descriptors) were covered by compile verification.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH, compiled and deny-checked
- Architecture/boundary: HIGH (compiled), render graph/extraction MEDIUM (design)
- Pitfalls: HIGH for wgpu 30 API deltas and deny, MEDIUM for shader/pipeline field specifics

**Research date:** 2026-11-25
**Valid until:** 2026-12-25 (wgpu releases fast; re-check if 30.0.3+ lands before execution)
