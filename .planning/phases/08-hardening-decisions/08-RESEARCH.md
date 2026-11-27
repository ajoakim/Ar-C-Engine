# Phase 8: Hardening & Decisions - Research

**Researched:** 2026-11-26
**Domain:** Rust/wgpu engine hardening: egui debug overlay, software-adapter image regression, criterion benches, analytic atmosphere, ECS decision ADR, docs
**Confidence:** MEDIUM overall (versions and repo facts HIGH; CI software-adapter behaviour on hosted runners MEDIUM/LOW)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Atmosphere and debug overlays (PLAN-08, QA-02)**
- Atmosphere: analytic limb-glow and sky-colour shader pass fed by the existing haze/fog uniforms (ADR-0015 is the precursor and this supersedes the haze). Lives in the planetary extension only.
- Overlay UI: new `debug-ui` crate on egui behind a feature flag; engine core stays free of egui.
- Overlay contents: LOD level histogram, selected/drawn counts, streaming queue, memory and upload budget, frame/prepare times (existing telemetry columns).
- Wiring: planetary app feeds an engine-core diagnostics snapshot so the overlay is not planet-specific.

**CI and regression tests (QA-01)**
- Software adapter: lavapipe on Linux CI, WARP on Windows. Image tests skip with a clear message when no adapter is found.
- Image diff: render to texture at fixed size, compare to goldens with per-channel tolerance and max-differing-pixels budget; goldens stored per platform.
- Performance: criterion benches for selection, mesh build and streaming; CI records results and does not hard-fail on timing.
- Goldens on Linux/macOS generated on hosted CI and committed (closes the "pending hosted CI" item from 07.1).

**Docs and decision ADRs (QA-03, WS-05)**
- ECS ADR: record "no ECS in P0" with a small hecs vs slotmap-world benchmark so the ADR cites a measurement.
- Existing ADRs 0005/0006/0010: audit for prototype/benchmark evidence; add reference or amendment where missing, do not rewrite.
- Docs set under `docs/`: architecture overview, assumptions, limitations, SDK usage guide, each checked by `docs_policy`.
- Carry-overs: update the stale Phase 7 `07-VERIFICATION.md` (gaps G1-G3 closed by 07.1) and the STATE.md metrics/current-focus.

### Claude's Discretion
Exact atmosphere constants and colours, overlay layout, bench parameters, and doc structure are at Claude's discretion within the above.

### Deferred Ideas (OUT OF SCOPE)
None - discussion stayed within phase scope. Full LUT-based atmospheric scattering is not planned.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PLAN-08 | Planet shows a minimal atmosphere (first item to cut) | Section "Atmosphere": limb ring + sky colour with NO engine change, using existing unlit pipeline, depth and `scene.fog`/`clear_color` |
| QA-01 | Unit, integration, criterion perf and image-diff regression tests in CI on a software adapter | Sections "Software-adapter image tests" and "criterion benches"; CI facts (hosted CI works again) |
| QA-02 | Debug overlays for LOD and streaming/memory budget | Section "Debug overlay": egui 0.37 aligned with wgpu 30 / winit 0.30; placement under check-deps and cargo-deny; licence finding |
| QA-03 | Architecture, assumptions, limitations, SDK usage docs | Section "Docs gap audit" |
| WS-05 | ADRs for rendering API, ECS, spatial backend, LOD transition, each citing prototype/benchmark | Section "ADR evidence audit", ECS bench design |
</phase_requirements>

## Summary

The repo is in a good state to build this phase on: wgpu 30.0.2 and winit 0.30.13 are locked (`Cargo.lock`), egui/egui-wgpu/egui-winit 0.37.0 depend on exactly `wgpu ^30.0` and `winit ^0.30.13`, so versions align with no changes. The real constraint is not versions but the dependency rules. `egui-wgpu` depends directly on `wgpu` and `egui-winit` directly on `winit`; verified by a scratch spike, cargo-deny `[bans]` then fails with `banned` unless those crates are added to the `wrappers` lists. The overlay therefore has to be painted inside `engine-render-wgpu` (the only wgpu crate); a new `debug-ui` crate can depend on pure `egui` only. A second verified finding: `egui` with `default_fonts` pulls `epaint_default_fonts` (licence `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`), which `deny.toml` rejects today; `criterion` (no default features) and `hecs` pass the licence check.

For QA-01, the existing offscreen backend (`WgpuBackend::new_offscreen`, `read_offscreen_rgba8`) already renders to a texture and reads it back; every GPU test today is `#[ignore]`. It requests an adapter with default options and ignores all `WGPU_*` environment variables (it uses `InstanceDescriptor::new_without_display_handle()`, not the `_from_env` variant), so forcing lavapipe/WARP needs a small backend change. Hosted CI is working again: the last pushed commit (`c472655`, plan 07.1-21) ran green on ubuntu, windows, macos, canary and deny; but 33 local commits (plans 07.1-22..27, 18, Phase 8 context) are unpushed, so the newer goldens (v2 heights, morph, hex prepare) have never run off Windows.

The atmosphere can be done with zero engine-core changes: the unlit pipeline, reverse-Z depth and the existing `scene.fog` and `clear_color` suffice for a camera-facing limb-glow ring mesh plus an altitude and sun-dependent sky colour, all inside `planetary-app`. A new blend mode or sky pass is NOT needed for a "minimal" atmosphere. The ECS ADR needs a hecs 0.11.2 vs `engine_world::World` benchmark in a standalone tooling crate (outside `crates/` and `extensions/`) so hecs never enters an engine crate manifest.

**Primary recommendation:** Do the work in this order: (1) CI/adapter plumbing + benches + docs/ADRs (all low risk), (2) debug overlay with egui painted inside `engine-render-wgpu` behind a feature, (3) atmosphere ring last (PLAN-08 is the declared first item to cut).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Diagnostics snapshot type (generic gauges, histogram, budgets) | engine-diagnostics / engine-core (tier 0-1) | - | Must be planet-agnostic; `engine-diagnostics` is tier 0 and already holds `FrameStats` [VERIFIED: crates/engine-diagnostics/src/frame_stats.rs] |
| Overlay UI layout (egui widgets) | new `debug-ui` crate (pure egui) | - | Depends on egui only; no wgpu/winit; optional dep of apps |
| Painting egui meshes to the frame | engine-render-wgpu (feature-gated) | engine-sdk (feature-gated hand-off) | Only wgpu crate (R2); `egui_wgpu::Renderer::render` needs `wgpu::RenderPass<'static>` [VERIFIED: egui-wgpu-0.37.0/src/renderer.rs:506] |
| Atmosphere geometry/colours | planetary-app (extension) | engine-render (existing unlit pipeline, fog) | Planet knowledge stays in extension (HLD 22.4) |
| Software adapter selection | engine-render-wgpu | CI workflow env | Adapter request lives in the backend |
| Image-diff harness | engine-render-wgpu tests (+ optional engine-sdk test) | - | Existing offscreen readback lives there |
| criterion benches | the crate owning the code (planetary-core, planetary-terrain/app, engine-spatial) | - | `[[bench]]` dev-dependency only; no tier impact |
| hecs-vs-slotmap bench | standalone tooling crate (e.g. `spikes/ecs-bench`) | - | Keeps hecs out of every engine manifest |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| egui | 0.37.0 (published 2026-10-15, MSRV 1.95) | Immediate-mode overlay UI | Stack doc choice; `default = []` features [VERIFIED: crates.io API, egui-0.37.0/Cargo.toml] |
| egui-wgpu | 0.37.0 (dep `wgpu ^30.0`) | Paint egui meshes in wgpu | Aligned with wgpu 30.0.2 in Cargo.lock [VERIFIED: crates.io dependencies API] |
| egui-winit | 0.37.0 (dep `winit ^0.30.13`) | Input translation (NOT needed for a read-only overlay) | Aligned with winit 0.30.13 [VERIFIED] |
| criterion | 0.8.2 (MSRV 1.86, `Apache-2.0 OR MIT`) | Benchmarks | Stack doc; use `default-features = false, features = ["cargo_bench_support"]` (no plotters/rayon) [VERIFIED: crates.io; cargo-deny licences ok in spike] |
| hecs | 0.11.2 (MSRV 1.81, `MIT OR Apache-2.0`, 2026-10-04) | ECS candidate, benchmark only | Stack doc "deferred" row [VERIFIED: crates.io; deps foldhash, hashbrown, spin] |
| wgpu | 30.0.2 (already locked) | Software adapter via `force_fallback_adapter` / env | `wgpu::util::initialize_adapter_from_env_or_default`, `InstanceDescriptor::..._from_env` exist in 30.0.2 [VERIFIED: registry source wgpu-30.0.2/src/util/init.rs, wgpu-types-30.0.2/src/instance.rs] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| png | 0.18.1 (already in Cargo.lock via `image`) | Viewable PNG goldens, as a dev-dependency of engine-render-wgpu | `image` is banned outside conventional-3d/gltf (deny.toml wrappers), `png` is not [VERIFIED: Cargo.lock, deny.toml] |
| PPM P6 (no dependency) | - | Alternative golden format | Zero-dep fallback, trivially diffable by humans with an image viewer |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| egui-wgpu inside engine-render-wgpu | Own mini UI pipeline fed by an engine-owned neutral mesh packet (epaint `Vertex` = pos, uv, premultiplied sRGBA; textures are RGBA only [VERIFIED: epaint-0.37.0/src/image.rs, mesh.rs]) | Keeps egui types out of the SDK completely, but ~200 lines of own pipeline, scissor and texture-delta code; use only if egui-wgpu placement is rejected |
| egui-winit input | None: read-only overlay toggled by an `InputEvent` key (F3) in `App::input`; build `egui::RawInput` by hand (screen rect, time) | Avoids `egui-winit`, the winit wrapper entry and any `engine-platform` change. Recommended |

**Installation (workspace `[workspace.dependencies]`):**
```toml
egui = { version = "0.37.0", default-features = false, features = ["default_fonts"] }
egui-wgpu = { version = "0.37.0" }          # only engine-render-wgpu may depend on it
criterion = { version = "0.8.2", default-features = false, features = ["cargo_bench_support"] }
hecs = "0.11.2"                              # only the ecs-bench tooling crate
```
**Version verification:** `curl https://crates.io/api/v1/crates/<crate>` on 2026-11-26: egui, egui-wgpu, egui-winit 0.37.0; hecs 0.11.2; criterion 0.8.2. [VERIFIED: crates.io API]

## Architecture Patterns

### System Architecture Diagram

```
planetary-app update()/gpu_frame()
   |  per frame: TerrainReport, FrameSample (telemetry), drawn_levels(), StreamBudget
   v
DiagnosticsSnapshot (engine-diagnostics, generic: gauges, budgets, histogram, frame/prepare ms)
   |            (only when feature "debug-overlay" and overlay toggled with F3)
   v
debug-ui::draw(&egui::Context, &DiagnosticsSnapshot) -> egui::FullOutput -> tessellate
   |  (crate depends on egui only)
   v
engine-sdk (feature-gated hand-off: App::overlay_frame() -> Option<OverlayFrame>)
   v
Runtime/run_app -> WgpuBackend::submit(packet) then draw_overlay(frame)   [feature "egui-overlay"]
   |  egui_wgpu::Renderer: update_texture + update_buffers + render into the same color target
   v
window / offscreen target

Atmosphere (independent):
planetary-app update(): altitude, sun dir -> haze_for_altitude() (fog + clear colour, existing)
                        + limb ring mesh (PosColor verts, rebuilt per frame via Resources::create_mesh/destroy_mesh in gpu_frame, ADR-0009)
                        -> RenderItem drawn with existing UnlitVertexColor pipeline
```

### Recommended Project Structure
```
crates/debug-ui/                 # new, tier 4, depends on egui + engine-diagnostics only
crates/engine-render-wgpu/src/overlay.rs   # feature "egui-overlay": egui-wgpu painter
crates/engine-render-wgpu/tests/image_diff.rs + tests/golden/<os>/*.png
crates/engine-spatial/benches/spatial.rs   # HashGridIndex vs LinearIndex (ADR-0006)
extensions/planetary/planetary-core/benches/{select,mesh}.rs
extensions/planetary/planetary-app/benches/stream.rs (or planetary-terrain)
spikes/ecs-bench/                # tooling crate: hecs vs engine_world::World (ADR-0017)
extensions/planetary/planetary-app/src/atmosphere.rs
docs/{architecture-overview,assumptions,limitations,sdk-usage}.md
```

### Pattern 1: Debug overlay placement under the dependency rules
**What:** `debug-ui` (pure `egui`) builds the UI from a generic snapshot. The painter (egui-wgpu) lives in `engine-render-wgpu` behind cargo feature `egui-overlay`. A read-only overlay needs no `egui-winit`: build `RawInput` by hand.
**Why this placement (all verified):**
- R2 in `xtask/src/check.rs` flags only a direct dep named `wgpu` outside `engine-render-wgpu`; `egui-wgpu` is a different name so R2 does not fire, BUT `deny.toml [bans]` `wgpu` has `wrappers = ["engine-render-wgpu"]`; the spike failed with `error[banned]: crate 'wgpu = 30.0.2' is explicitly banned` and `unmatched-wrapper: direct parent 'egui-wgpu = 0.37.0' ... was not marked as a wrapper` (same for `egui-winit`/`winit`).
- Therefore: add `"egui-wgpu"` to the `wgpu` wrappers in `deny.toml`; extend `xtask` R2 (`WGPU_ALLOWED` / dep-name check in `check.rs:~165`) to treat `egui-wgpu` like `wgpu` (allowed only in `engine-render-wgpu`), add a negative fixture under `xtask/fixtures/`, and a CI negative step like the existing `wgpu-winit-misplaced` one. Do not add `egui-winit` at all (no winit wrapper change).
- `debug-ui` needs a row in `xtask/src/layers.rs` LAYERS (tier 4: depends on engine-diagnostics tier 0), a row in `docs/architecture/dependency-direction.md` (`docs_covers_all_crates` test requires `` `debug-ui` `` literal), and the new rules documented.
- `engine-core`, `engine-render`, `engine-world` stay egui-free. `engine-sdk` and `engine-render-wgpu` hold egui only behind the feature, off by default; `planetary-app` makes `debug-ui` an optional dependency (feature `debug-overlay`).
**egui 0.37 API (verified in registry source):** `Context::run_ui(RawInput, |ui| ...) -> FullOutput` (egui-0.37.0/src/context.rs:950; the older `run` is not present), `Context::tessellate(shapes, pixels_per_point)` (context.rs:3358), `FullOutput { textures_delta, shapes, pixels_per_point, .. }`. `egui_wgpu::Renderer::new(&Device, output_color_format, RendererOptions)`, `update_texture`, `update_buffers(&Device,&Queue,&mut CommandEncoder,&[ClippedPrimitive],&ScreenDescriptor)`, `render(&self, &mut RenderPass<'static>, &[ClippedPrimitive], &ScreenDescriptor)`.
**Integration detail to verify in planning:** the backend's `submit` owns its encoder and surface texture (`crates/engine-render-wgpu/src/backend.rs:~543`); the overlay must be drawn in a final pass with `LoadOp::Load` on the same color view, after the scene pass, before present. Offscreen target format is `Rgba8UnormSrgb`; surface format is the first sRGB format (backend.rs:~160). [ASSUMED] egui-wgpu works with an sRGB target when its shader is told so (it handles sRGB output by default); check `RendererOptions` in 0.37 docs.
**SDK hook:** ADR-0009 gives `App::gpu_frame(&mut Resources, &mut SceneState)` after `update` and before extraction (crates/engine-sdk/src/runtime.rs:96-131). The overlay frame is produced in `update`/`gpu_frame`; add a feature-gated default-`None` method (e.g. `App::overlay_frame`) or a `SceneState` field, consumed in `run_app` (which holds the concrete `WgpuBackend`) after `Runtime::frame`'s submit. `Runtime<B: RenderBackend>` stays generic and the null backend ignores the overlay. Keep `FramePacket` unchanged: it is constructed by literal in `crates/engine-render-wgpu/tests/gpu_smoke.rs` and would break.
**Overlay data available today (verified):** `PlanetaryApp::drawn_levels()` / `wanted_levels()` (lib.rs:493/505) give the LOD histogram; `FrameSample` (planetary-app/src/telemetry.rs:79) has `nodes_visited, wanted_leaves, drawn_leaves, held_meshes, pending, in_flight, awaiting_upload, started, completions, uploads, uploaded_bytes, evicted, frame_ms, prepare_ms, prep_scope_ms[6], tau_eff, culled_*`; `PlanetaryApp::stream_budget()` returns `StreamBudget { uploads, starts, requests, scale }`; the memory budget is `TerrainConfig::detail_budget_patches` (default 1500, planetary-terrain/src/system.rs:310) over engine-assets `ClassBudget` (manager.rs:29). Snapshot should carry: `Vec<(label, f64)>` gauges, `Vec<Budget{label, used, cap}>`, `Histogram{label, bins: Vec<u32>}`, `frame_ms`, `prepare_ms`.

### Pattern 2: Software adapter selection and headless readback
**Current code (verified):** `WgpuBackend::new_offscreen_with` (backend.rs:220-251) creates `wgpu::Instance::new(InstanceDescriptor::new_without_display_handle())` and `request_adapter(&RequestAdapterOptions { compatible_surface: None, ..Default::default() })`; format `Rgba8UnormSrgb`; `read_offscreen_rgba8` (backend.rs:371) copies with 256-byte row padding and strips it. Tests `gpu_lit.rs`/`gpu_smoke.rs` are all `#[ignore]`; helpers `render`, `pixel`, `fnv` exist (gpu_lit.rs:119-126, 364).
**wgpu 30.0.2 facts [VERIFIED: registry source]:** `RequestAdapterOptions.force_fallback_adapter: bool` ("generally a software implementation"); `InstanceDescriptor::new_without_display_handle_from_env()` / `.with_env()` honour `WGPU_BACKEND`; `wgpu::util::initialize_adapter_from_env_or_default` honours `WGPU_ADAPTER_NAME` (substring match, panics via `expect` if set but no match, so use only the `_or_default` form); `PowerPreference::from_env` reads `WGPU_POWER_PREF`.
**Recommended change:** add an `AdapterPolicy { Default, Software }` (or a constructor `new_offscreen_software`) to the backend: `Software` sets `force_fallback_adapter: true` and returns `Err(NoAdapter)` cleanly when none exists. Image-diff tests use `Software` only, so goldens never depend on a developer's real GPU; on a dev machine without a software adapter they skip with a printed reason. Keep existing `#[ignore]` hardware tests, or convert them to the same skip-helper.
**CI setup (per OS):**
- Linux (`ubuntu-latest`): `sudo apt-get update && sudo apt-get install -y mesa-vulkan-drivers vulkan-tools` then `vulkaninfo --summary` to log the adapter; lavapipe shows as `llvmpipe`. Optionally `WGPU_BACKEND=vulkan`. Mesa version on the runner matters (old lavapipe fails Vulkan 1.3 instance creation in other projects; Mozilla needed Mesa >= 22.1.2). [MEDIUM: package names are standard Ubuntu; wgpu's own CI documents lavapipe for Linux/Vulkan (search result), exact runner behaviour for this repo is unverified until the first hosted run]
- Windows (`windows-latest`): WARP ("Microsoft Basic Render Driver") is part of Windows; wgpu DX12 enumerates it as a CPU-type adapter, so `force_fallback_adapter: true` + `WGPU_BACKEND=dx12` selects it. wgpu's own CI installs a WARP build (`install-warp` action), i.e. newer WARP than the OS one may be needed for some features [CITED: github.com/gfx-rs/wgpu .github/workflows/ci.yml]. [LOW: unverified on hosted runner for this repo]
- macOS (`macos-latest`): wgpu's CI uses a hardware Metal runner; no software Metal adapter exists. Whether hosted arm64 runners expose a Metal device is not confirmed by any source found. [LOW] Treat macOS image tests as skip-allowed; goldens per CONTEXT are still "generated on hosted CI and committed" if an adapter exists.
- Pitfalls to encode: (a) a Rust "skip" is a vacuous pass, so add an env switch (e.g. `ARC_REQUIRE_ADAPTER=1`) set on the Linux and Windows jobs that turns "no adapter" into a failure, otherwise the suite can silently stop testing; (b) `ARC_UPDATE_GOLDENS=1` rewrites goldens, plus an `actions/upload-artifact` step uploading actual and diff images so a human can commit per-platform files; (c) never let hardware adapters run golden tests (non-determinism across GPUs).
**Image diff recommendation:** fixed 64x64 (existing `W`) and one 128x128 scene; compare RGBA8 per channel with tolerance `<= 2` of 255 and a budget of at most 0.5% differing pixels, per-platform goldens `crates/engine-render-wgpu/tests/golden/{linux,windows,macos}/<name>.png`; write `actual` and `diff` PNGs to `target/image-diff/` on failure. Tolerances are first guesses [ASSUMED]; calibrate by running twice on each OS. Scenes: unlit triangle, lit quad with directional light, fog gradient, morph at 0.5, and the atmosphere ring (so PLAN-08 has a regression test). Existing bit-hash tests (`fnv`, plan 21 identity hash `583ae580ef4842fd`, Intel Arc) are hardware-specific and must not be reused as goldens.
**SDK reach:** `engine-sdk` re-exports `PresentInfo` but not `WgpuBackend` (crates/engine-sdk/src/lib.rs:37). Planetary-level image tests would need a small additive re-export of `WgpuBackend` offscreen constructors; recommend keeping image tests in `engine-render-wgpu` (hand-built `FramePacket`) plus at most one `engine-sdk` test using `Runtime::new(engine, WgpuBackend::new_offscreen_software(..))` and `rt.backend().read_offscreen_rgba8()`.

### Pattern 3: criterion benches (wrap existing code)
- criterion is NOT in the workspace today [VERIFIED: root Cargo.toml, Cargo.lock has no criterion entry]. Add to `[workspace.dependencies]` and as `[dev-dependencies]` + `[[bench]] harness = false` per crate. `cargo clippy --workspace --all-targets -D warnings` (ci.yml) compiles benches, so they must be warning-free.
- Selection: `select_cells<T: Topology>(topo, model, params, view, prev_split, resident_range)` (extensions/planetary/planetary-core/src/topology/select.rs:416). Bench in `planetary-core/benches/select.rs`, built like `tests/lod_select.rs` at real parameters (hex and cube).
- Mesh build: `build_hex_cell` (planetary-core/src/hex/mesh.rs:153), `build_patch` (planetary-core/src/mesh.rs:287). `planetary-core/benches/mesh.rs`.
- Streaming/prepare: `run_stream` (planetary-app/src/stream_bench.rs:86) is wall-clock paced (`spin_for_ms`) so it is NOT a good criterion body; bench the `TerrainSystem` frame via `PlanetaryApp::with_clock` + `Runtime<NullBackend>` with `settle_each_frame` (deterministic, as `bench()` in examples/planetary_flight.rs:69 does) over a short fixed frame count; use `sample_size(10)` and reduced frames because a hex descent frame costs about 14 ms in release (ADR-0015 evidence). Place in `planetary-app/benches/stream.rs`.
- Spatial: `HashGridIndex` vs `LinearIndex` (engine-spatial/src/{hash_grid,linear}.rs) in `engine-spatial/benches/spatial.rs` at N = 1e3, 1e4, 1e5 entities for frustum, region and radius queries plus update. This is the missing evidence for ADR-0006.
- CI: `cargo bench --workspace --benches --locked -- --noplot --quick` in a separate job `continue-on-error`, upload `target/criterion` as an artifact; no baseline comparison, never fail on timing (locked).
- Layer rules: benches are dev-deps of the owning crate; no tier or R1-R6 effect.

### Pattern 4: Atmosphere (PLAN-08) with no engine change
**What the engine offers (verified):** `ShaderId` is a closed enum with two shaders (`UnlitVertexColor`, `LitTextured`; engine-render/src/backend.rs:35); blend is hard-coded `BlendState::REPLACE` (engine-render-wgpu/src/pipeline.rs:284); every `PassPacket` begins with `LoadOp::Clear` for colour and depth (backend.rs:~551/564), so a second full-screen sky pass is impossible without core changes; depth is reverse-Z `Greater`, write on, clear 0.0; `SceneState` has `fog: DistanceFog {color, density}` and `clear_color`; `haze_for_altitude(alt) -> Haze { fog, clear_color }` already blends sky colour and space colour by altitude (planetary-app/src/lib.rs:61); the unlit shader has no fog.
**Recommendation (cheapest, consistent with "analytic limb glow + sky colour fed by haze/fog"):**
1. Sky colour: extend `haze_for_altitude` into an `atmosphere_for_view(altitude, sun_dir, view_dir)` in a new `planetary-app/src/atmosphere.rs`. Keep fog colour == clear colour at the horizon; tint toward warm near the terminator using the sun elevation (pure `+ - * /`, unit-tested like the existing haze tests at lib.rs:796-850).
2. Limb glow: a camera-facing annulus mesh of `VertexPosColor` vertices, centre at the planet centre, in the plane perpendicular to the view axis. Inner radius from the tangent cone: `r_in = d * R / sqrt(d^2 - R^2)` (d = camera distance to centre, R = outer terrain radius), slightly reduced (about 0.98) so terrain relief overlaps and occludes it; outer radius `r_in * (1 + h/R * k)`. Vertex colours go from the haze/sky colour at the inner edge to exactly the space clear colour at the outer edge, modulated by the sun-facing term, so opaque `REPLACE` is seamless. The planet disc occludes the inner part through reverse-Z depth because the ring plane passes through the planet centre. Draw it as a normal `RenderItem` with the existing unlit pipeline; rebuild its ~128 vertices per frame in `gpu_frame` using `Resources::create_mesh` and `destroy_mesh` (ADR-0009, double destroy is an `Err`), or keep a static unit ring and scale/rotate it with `RenderItem.rotation/scale`. Fade the ring out below roughly 20 km altitude (same band as `HAZE_NONE/FULL_ALTITUDE`, lib.rs:32-36) where fog and clear colour take over.
3. Only if the ring looks wrong is a blend mode on `PipelineDesc` plus an alpha unlit shader justified; that is a core change and out of "minimal". Record as the fallback in the ADR.
**Constraints:** all planet numbers (scale height, colours, radii) live in `planetary-app`; the SDK/render API gets no atmosphere type (HLD 22.4). `AppSettings.haze` flag (lib.rs:173) should gain a sibling `atmosphere` flag; `--no-haze` pattern exists in examples/planetary_flight.rs.
**Supersession:** ADR-0015 decision 9 says Phase 8 supersedes the haze; per ADR README rules do not rewrite 0015, add a new ADR (atmosphere) and set 0015's status line text to reference it. `DistanceFog` stays (generic).

### Pattern 5: ECS benchmark (ADR-0017 "no ECS in P0")
- Standalone tooling crate at `spikes/ecs-bench` (new top-level directory). `location_of` (xtask/src/check.rs:101) classifies anything outside `crates/` and `extensions/` as `Location::Tooling`, so it is exempt from R5/R6 and from the layer table. It still must (a) be a workspace member or its own workspace, (b) appear in `docs/architecture/dependency-direction.md` if a member (`docs_cover_all_crates`), (c) not be allowed wgpu/winit (it is not in `TOOLING`, so R2/R3 apply). hecs then appears in no engine crate manifest and cannot reach `conventional-3d` (check-canary only looks for planetary, but the doc rule is satisfied).
- Design: entity count N in {1e4, 1e5}; component `engine_world::Transform`; operations: spawn N, iterate-and-update all local transforms, random `get`/`set_local` of 1% entities, despawn 10% and respawn, parent-child hierarchy update (`World::update_transforms`, engine-world/src/world.rs:263 vs a hecs query + manual parent walk). criterion groups `slotmap_world/*` and `hecs/*`. State that the engine `World` implements hierarchy and frame-rebasing semantics that hecs does not, so the comparison is "cost of what we need", not feature parity.
- ADR-0017 content: Decision "no ECS in P0; `engine_world::World` (slotmap) stays"; cite the measured numbers; alternatives hecs, bevy_ecs 0.20.1 (stack doc, not benchmarked, MEDIUM); revisit trigger e.g. "if iteration of 1e5 transforms exceeds X ms or systems scheduling is needed". hecs 0.11.2 matches the CLAUDE.md stack row.

### Anti-Patterns to Avoid
- Putting `egui-wgpu` or `egui-winit` in `debug-ui` or any non-backend crate: fails cargo-deny `[bans]` and weakens R2/R3.
- Using hardware adapters for golden tests: unstable; always force software.
- Adding a `FramePacket` field: breaks literal constructions in tests (`gpu_smoke.rs`).
- Making criterion a hard CI gate: contradicts the locked decision.
- Rewriting accepted ADRs: amend with a dated section (as 0010 already does) or supersede.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Immediate-mode UI + font atlas + tessellation | Own text/bar renderer | egui 0.37 + egui-wgpu painter | Text shaping, atlas deltas, scissor, premultiplied alpha |
| Benchmark statistics | Own timing loops | criterion 0.8.2 | Warm-up, outlier handling, comparisons |
| Software GPU | Own CPU rasteriser | lavapipe / WARP via wgpu fallback adapter | Real driver behaviour |
| ECS comparison baseline | Own mini-ECS | hecs 0.11.2 | Credible measurement for the ADR |
| PNG encode/decode for goldens | Custom format | `png` 0.18.1 (already locked) or PPM | `image` is banned outside extensions |

**Key insight:** every item above already exists; the work is plumbing under the repo's isolation rules (deny wrappers, check-deps, layer table, docs_policy), not new algorithms.

## Runtime State Inventory

Not a rename/refactor phase. Omitted. (Only note: `deny.toml` and `xtask` rule tables are config that must stay in sync with new crates.)

## Common Pitfalls

### Pitfall 1: cargo-deny wrapper failure on egui-wgpu/egui-winit
**What goes wrong:** `cargo deny check bans` fails in CI (`deny` job).
**Why:** `wgpu`/`winit` bans list only `engine-render-wgpu`/`engine-platform` as wrappers [VERIFIED by spike].
**How to avoid:** add `egui-wgpu` to the wgpu wrappers; do not use `egui-winit`; add a negative fixture (existing pattern: `xtask/fixtures/wgpu-winit-misplaced`).
**Warning signs:** `unmatched-wrapper` warnings in `cargo deny` output.

### Pitfall 2: Font licences
**What goes wrong:** `cargo deny check licenses` rejects `epaint_default_fonts` (`OFL-1.1`, `Ubuntu-font-1.0`) [VERIFIED by spike: only these two were rejected; the rest of the egui tree, criterion and hecs passed with the repo allow list].
**How to avoid:** a deliberate decision: add `OFL-1.1` and `Ubuntu-font-1.0` to `[licenses].allow` (recommended; fonts are only embedded data) with a comment, or disable `default_fonts` and ship a permissively licensed font via `egui::FontDefinitions` (note: with `default_fonts` off egui has no fonts and text will not render [ASSUMED from egui docs]). Record in the debug-ui ADR; this changes a project-wide policy, so flag for user confirmation.

### Pitfall 3: Vacuous skip passes
**What goes wrong:** image tests silently skip on CI when lavapipe fails to install.
**How to avoid:** `ARC_REQUIRE_ADAPTER=1` on Linux/Windows jobs; print the adapter summary in test output (`adapter_summary()` exists, backend.rs:365).

### Pitfall 4: Goldens not yet proven cross-OS
**What goes wrong:** CPU goldens (GOLDEN_HEIGHTS_V2 `0x3096173351c359fb`, hex mesh, morph) were produced on Windows. The last green hosted run (run 45893531848, commit `c472655`, plan 07.1-21) predates plans 22-27; 33 local commits are unpushed (`git rev-list --count origin/master..HEAD` = 33). Floating-point determinism problems could appear at first push.
**How to avoid:** first Phase 8 task: push and run the matrix, fix any golden drift (ADR-0008 exact-op rules) before layering new tests. Earlier runs failed because of GitHub billing ("recent account payments have failed"), now resolved: the last two runs succeeded [VERIFIED: `gh run list`, `gh run view`].

### Pitfall 5: Reverse-Z with the atmosphere ring
Depth clears to 0.0 with compare `Greater`; anything at "far" fails. The ring is at the planet-centre plane, so it is nearer than the planet far side and farther than the near side: correct occlusion. Do not push it to infinity. (Core constraint, verified in pipeline.rs:211-212.)

### Pitfall 6: CI runtime
The Test step took about 30 minutes in the last green run. Adding benches and image tests to the same job risks timeouts; put benches in a separate non-blocking job and image tests in a dedicated job (`--test image_diff`).

### Pitfall 7: Docs policy drift
Every new ADR needs a README table row and a `docs_policy.rs` test (`adr_00NN_present`, pattern in xtask/tests/docs_policy.rs). Every new workspace crate needs its backticked name in `dependency-direction.md`. New docs need required-literal tests too.

### Pitfall 8: Criterion bench too slow or nondeterministic
A hex frame costs about 14 ms release; limit iterations; use `NullBackend` and `settle_each_frame` for determinism.

## Code Examples

### Forcing a software adapter (wgpu 30.0.2)
```rust
// Source: wgpu-types-30.0.2/src/adapter.rs (RequestAdapterOptions), wgpu-30.0.2/src/util/init.rs
let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
    force_fallback_adapter: true,
    compatible_surface: None,
    ..Default::default()
}));
// Err(..) => skip the test with a printed reason unless ARC_REQUIRE_ADAPTER=1
```

### egui 0.37 frame without egui-winit
```rust
// Source: egui-0.37.0/src/context.rs (run_ui, tessellate) [VERIFIED in registry source]
let mut raw = egui::RawInput::default();
raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w as f32, h as f32)));
raw.time = Some(sim_time_s);
let out = ctx.run_ui(raw, |ui| debug_ui::panel(ui, &snapshot));
let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
// out.textures_delta -> Renderer::update_texture, prims -> update_buffers/render (in engine-render-wgpu)
```

### Tangent-cone limb radius (atmosphere ring)
```rust
// d = |camera - planet_centre|, r = outer terrain radius, d > r
let r_in = d * r / (d * d - r * r).sqrt();
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `egui::Context::run` | `Context::run_ui` | egui 0.37 (no `run` in registry source) | Use `run_ui` |
| Windows software GPU via OpenGL | DX12 WARP through wgpu fallback adapter | wgpu CI practice | Prefer dx12 + force_fallback |
| Separate `Backends` env only | `InstanceDescriptor::..._from_env`, `with_env()` | wgpu 30 | One call opts into `WGPU_BACKEND` etc. |

**Deprecated/outdated:** none relevant; `bincode`/`noise` not used this phase.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | egui-wgpu renders correctly into an `Rgba8UnormSrgb` / sRGB surface target with default `RendererOptions` | Pattern 1 | Washed-out overlay colours; adjust options |
| A2 | With `default_fonts` off egui renders no text | Pitfall 2 | Option "ship own font" is moot |
| A3 | Image tolerances (<=2/255, 0.5% pixels) suffice across runs of one software adapter | Pattern 2 | Flaky tests; calibrate |
| A4 | WARP and lavapipe are present on `windows-latest` / `ubuntu-latest` after the stated setup | Pattern 2 | Image tests skip or fail; check first hosted run |
| A5 | macOS hosted runner exposes a usable Metal device | Pattern 2 | macOS goldens impossible; mark skip-allowed |
| A6 | Opaque vertex-colour limb ring looks acceptable without alpha blending | Pattern 4 | Needs a blend mode (core change) |
| A7 | `ctx.tessellate` plus egui-wgpu can be fed from `update`/`gpu_frame` outside the backend's pass | Pattern 1 | May need a new backend method; verified only at API-signature level |
| A8 | Snapshot type home (engine-diagnostics vs engine-core) | Architecture map | CONTEXT says "engine-core diagnostics snapshot"; engine-diagnostics is the lower tier, both are core. Confirm |

## Open Questions (RESOLVED)

1. **Font licences:** accept `OFL-1.1` and `Ubuntu-font-1.0` in `deny.toml`, or ship a font? Recommendation: accept (data only). Needs user confirmation (policy change).
   RESOLVED: the user explicitly chose to allow both licences (their answer to "Allow both licences (Recommended)"). `deny.toml` allows `OFL-1.1` and `Ubuntu-font-1.0` (plan 06 Task 2, before egui enters the tree); ADR-0018 records the policy change (plan 07).
2. **macOS image tests:** if no adapter exists on hosted macOS, CONTEXT's "goldens on Linux/macOS generated on hosted CI" cannot be met for macOS images. Recommend: Linux and Windows image goldens required, macOS skip-allowed, CPU goldens (heights/mesh) cover macOS.
   RESOLVED: macOS image goldens are skip-allowed (no `ARC_REQUIRE_ADAPTER`, no `ARC_REQUIRE_GOLDENS` on macOS); Linux and Windows are required (plans 03 and 11).
3. **Overlay hand-off API:** SDK method vs `SceneState` field; decide in plan 1 of the debug-ui work after a 1-hour spike.
   RESOLVED: a feature-gated, default-None SDK method `App::overlay_frame` (plan 08); `FramePacket` and `SceneState` are unchanged.
4. **Does Phase 7.1 unpushed work pass on hosted CI?** Unknown until pushed.
   RESOLVED: the 33 commits plus the README commit were pushed at the user's request. Plan 01 verifies by comparing specific 07.1 commit shas against origin and pinning CI checks with `--commit <sha>`; any further push is a `checkpoint:human-action` for the user.
5. **Plan 26 present-mode and title readout** were never human-exercised (07.1-18 caveat C2); atmosphere visuals will also need meshes to judge (C1). Carry into Phase 8 human verification.
   RESOLVED: carried into plan 12 Task 2 (human flight checkpoint).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | all | yes | 1.99.0 pinned (rust-toolchain.toml) | - |
| cargo-deny | licence/bans checks | yes (07.1-18 gate; spike ran it) | installed | - |
| gh CLI + hosted GitHub Actions | QA-01 CI, Linux/macOS goldens | yes; runs succeed again, billing failures ended after 2026-11-26 18:15Z | last green run 45893531848 (31 min) | local only |
| Hardware GPU (Intel Arc B580, Vulkan) | local hardware tests | yes | - | not used for goldens |
| lavapipe (mesa-vulkan-drivers) | Linux image tests | not local (Windows dev machine); installed in CI by apt | Mesa version unknown | skip + `ARC_REQUIRE_ADAPTER` |
| WARP | Windows image tests | expected built into Windows [LOW: not probed] | - | skip |
| Metal software adapter | macOS | none exists | - | skip-allowed |
| crates.io registry | criterion, hecs, egui (already resolved in a spike lockfile) | yes | see Standard Stack | - |

**Missing dependencies with no fallback:** none blocking planning.
**Blocking risk:** 33 unpushed commits; the push must happen before goldens/CI work can complete (user action).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | built-in `cargo test` (863 passed, 11 ignored in the 07.1-18 gate), proptest/insta not required; criterion 0.8.2 (new) |
| Config file | workspace `Cargo.toml`; `.github/workflows/ci.yml`; `xtask/tests/*.rs` |
| Quick run command | `cargo test -p xtask --test docs_policy --locked` and `cargo test -p engine-render-wgpu --locked` |
| Full suite command | `cargo test --workspace --locked` (about 20 minutes locally per 07.1-18, 1169 s) |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PLAN-08 | atmosphere colour monotone/finite, ring radius formula, no-NaN, fades to 0 below/above bands | unit | `cargo test -p planetary-app atmosphere --locked` | Wave 0 (new `atmosphere.rs`) |
| PLAN-08 | ring renders on a software adapter and is occluded by the planet disc | image-diff | `cargo test -p engine-render-wgpu --test image_diff` | Wave 0 |
| QA-01 | image tests render, compare to goldens with tolerance, skip with message / fail under `ARC_REQUIRE_ADAPTER` | integration | `cargo test -p engine-render-wgpu --test image_diff -- --nocapture` | Wave 0 |
| QA-01 | negative control: perturbed golden or shifted geometry is detected | integration | same | Wave 0 |
| QA-01 | benches compile and run | bench | `cargo bench --workspace --benches --locked -- --noplot --quick` | Wave 0 |
| QA-02 | snapshot builder yields histogram from `drawn_levels()` and budgets; overlay produces non-empty tessellation headless | unit | `cargo test -p debug-ui -p planetary-app --features debug-overlay` | Wave 0 |
| QA-02 | overlay pixels appear on a software adapter (corner region differs from baseline) | image-diff | `image_diff` test with overlay feature | Wave 0 |
| QA-02 | check-deps and deny still pass with egui crates; negative fixture for misplaced `egui-wgpu` fails | policy | `cargo run -p xtask -- check-deps`; `cargo deny check bans licenses`; negative CI steps | extend existing |
| QA-03 | each doc exists and contains required literals | policy | `cargo test -p xtask --test docs_policy` | Wave 0 (add tests) |
| WS-05 | ADR-0017 (ECS) and amendments to 0005/0006/0010 present and cite bench/test names | policy | `cargo test -p xtask --test docs_policy` | Wave 0 |
| WS-05 | ECS bench runs | bench | `cargo bench -p ecs-bench --benches --locked -- --noplot --quick` | Wave 0 |

### Sampling Rate
- **Per task commit:** crate-scoped `cargo test -p <crate> --locked` plus `cargo fmt --all -- --check`.
- **Per wave merge:** `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `check-deps`, `check-canary`, negative fixtures, `cargo deny check`.
- **Phase gate:** hosted CI green on all three OS jobs including image jobs and bench job (non-blocking), then `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `crates/engine-render-wgpu/tests/image_diff.rs` and golden dirs per OS, `AdapterPolicy`/software constructor
- [ ] `crates/debug-ui/` crate, layer row, doc row, deny wrapper, xtask rule and fixture
- [ ] `engine-spatial/benches`, `planetary-core/benches`, `planetary-app/benches`, `spikes/ecs-bench`
- [ ] `xtask/tests/docs_policy.rs` tests for ADR-0017, new docs, and amended ADRs
- [ ] CI jobs: image-diff (apt lavapipe on Linux, `ARC_REQUIRE_ADAPTER`), bench (non-blocking, artifact), golden-update artifact upload

## Security Domain

Applicable ASVS categories are minimal: no auth, sessions, or network input in this phase.

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2/V3/V4 | no | - |
| V5 Input Validation | yes (small) | Validate golden/PPM/PNG dimensions before indexing; `RawInput` values finite; reuse `DistanceFog::validate`; env switches parsed strictly |
| V6 Cryptography | no | - |
| V14 Config/Supply chain | yes | `cargo deny` bans/licences/advisories/sources (new crates egui, criterion, hecs pass licences except fonts); `unsafe_code = forbid` workspace lint stays |

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Supply-chain via new dependency tree (egui 0.37 pulls vello_cpu, skrifa, harfrust) | Tampering | `cargo deny`, `--locked`, pinned versions; feature-gate egui off by default |
| CI downloads via apt (mesa) | Tampering | Standard Ubuntu repos, no third-party PPA |
| Golden-update env enabled accidentally in CI | Tampering | Update mode only when `ARC_UPDATE_GOLDENS=1`, never set in the blocking jobs |

## ADR evidence audit (WS-05)

| ADR | Existing evidence (repo) | Missing | Action |
|-----|--------------------------|---------|--------|
| 0005 rendering API (wgpu behind trait) | Scratch crate compile + `request_adapter` on Intel Arc (recorded in the ADR); null-backend tests; `engine-sdk` triangle example; gpu_lit/gpu_smoke readback tests (hardware, `#[ignore]`); plan 21 identity hash. Explicit deferrals: "Software-adapter CI deferred to Phase 8", "Linux Wayland display handle deferred to Phase 8" | Software-adapter image-diff CI evidence; the Wayland/display-handle item is still open | Amend with an "Amendment (Phase 8)" section citing the image-diff tests and CI run; list Wayland as a known limitation in `docs/limitations.md` unless done |
| 0006 spatial index (hash grid first) | `engine-spatial/tests/oracle.rs` correctness vs `LinearIndex` only; ADR itself says "The benchmark is deferred to the Phase 8 decision spike (WS-05)" | Any performance measurement | New criterion bench `engine-spatial/benches/spatial.rs` (HashGrid vs Linear; optionally rstar 0.14.0 from stack doc, LOW fit for f64 boxes); amend ADR with numbers and the conclusion (keep or supersede) |
| 0010 terrain LOD (skirts, shared-border, 2:1) | Strong: evidence table with seam proofs and negative controls (`planetary-core/tests/seam.rs`), scripted descent, amendments for 0012 and hex; ADR-0014 adds geomorph evidence (oracle, seam, identity hash) | Cross-OS golden confirmation of the CPU goldens (ADR says pending hosted CI); a visual regression for the transition | Add one-line amendment pointing to hosted CI green run and (optional) an image-diff scene of a morph at 0.5; no rewrite |
| ECS | No ADR exists (ADR numbers run 0000-0016) | Entire ADR and measurement | Create ADR-0017 with the hecs benchmark |
| LOD transition technique (WS-05 wording) | 0010 (skirts) + 0014 (geomorph) | Cross-reference only | State in ADR README or an amendment that these two ADRs together satisfy "LOD transition technique" |

## Docs gap audit (QA-03)

Existing: `docs/architecture/dependency-direction.md`, `docs/architecture/adr/0000-0016` + README, root `README.md` (status, highlights, run instructions per git log; not read in detail). Missing (all four): architecture overview, assumptions, limitations, SDK usage guide. `HLD.md` at repo root is untracked (`git status`), so the architecture overview should either commit it or not rely on it. Content sources: limitations are scattered in ADR Consequences (0010 f32 origin quantisation, 0014 unmorphed normal, 0015 first-guess haze values, 0011 hex), 07.1-18 caveats C1-C4, `docs/` none; SDK usage: `crates/engine-sdk/examples/triangle.rs`, `extensions/conventional-3d/examples/basic_scene.rs`, `extensions/planetary/planetary-app/examples/planetary_flight.rs` (documented flags in its header), ADR-0007/0009. Add `docs_policy` tests requiring headings/literals (pattern at xtask/tests/docs_policy.rs). Carry-overs: update `.planning/phases/07-planetary-extension-terrain-lod/07-VERIFICATION.md` (frontmatter `status: gaps_found`, gaps G1-G3 `status: failed`; its own Linux/macOS CI human item) and `.planning/STATE.md` (currently "Current focus: Phase 1", metrics table, stale `stopped_at`).

## Sources

### Primary (HIGH confidence)
- Repo files: `Cargo.toml`, `Cargo.lock`, `deny.toml`, `.github/workflows/ci.yml`, `xtask/src/{check,layers}.rs`, `xtask/tests/docs_policy.rs`, `docs/architecture/adr/{0005,0006,0009,0010,0014,0015}.md`, `crates/engine-render-wgpu/src/{backend,pipeline}.rs`, `crates/engine-render-wgpu/tests/{gpu_lit,gpu_smoke}.rs`, `crates/engine-sdk/src/{app,runtime,lib}.rs`, `extensions/planetary/planetary-app/src/{lib,telemetry,stream_bench}.rs`, `.planning/phases/07.1-*/07.1-18-SUMMARY.md`, `07.1-VALIDATION.md`
- crates.io API (2026-11-26): egui/egui-wgpu/egui-winit 0.37.0 dependency lists, hecs 0.11.2, criterion 0.8.2, `epaint_default_fonts` licence
- Registry source (cargo registry): wgpu-30.0.2, wgpu-types-30.0.2, egui-0.37.0, egui-wgpu-0.37.0, epaint-0.37.0
- Scratch spike (scratchpad `spike/`): `cargo deny check bans licenses` with egui (default_fonts), egui-wgpu, egui-winit, hecs, criterion; results quoted above
- `gh run list` / `gh run view` on ajoakim/Ar-C-Engine: run 45893531848 success on 3 OS + canary + deny; earlier failures were billing

### Secondary (MEDIUM confidence)
- gfx-rs/wgpu `.github/workflows/ci.yml` (WebFetch): uses install-mesa / install-warp / install-vulkan-sdk actions; macOS runner has no GPU-specific setup; search result summary: platform table lists Linux/Vulkan on lavapipe, Windows/DX12 on WARP, macOS/Metal on a hardware runner
- Web search: lavapipe via `mesa-vulkan-drivers`, version caveats (Mozilla bug 1837027, onnxruntime commits)

### Tertiary (LOW confidence)
- macOS hosted runner Metal availability: no confirming source found
- WARP presence on `windows-latest` for this repo: not probed

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - versions from crates.io and registry source, licence/ban behaviour from an actual `cargo deny` spike
- Architecture: MEDIUM - placement of the egui painter and SDK hand-off is designed from verified API signatures but not compiled
- Atmosphere approach: MEDIUM - based on verified engine limits, visual quality unproven
- CI software adapters: MEDIUM for Linux, LOW for Windows hosted and macOS
- Pitfalls: HIGH for deny/licence/CI facts, MEDIUM for tolerances

**Research date:** 2026-11-26
**Valid until:** 2026-12-26 (egui/wgpu release cadence is fast; re-check versions at plan time)
