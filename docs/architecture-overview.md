# Architecture Overview

This is a general-purpose 3D engine in Rust with a strictly layered core and pluggable extensions. Its core value: the engine core stays general-purpose and independent of any specialised capability (planetary rendering, physics, scripting, an editor). Specialised features are proven by demanding reference applications but never become assumptions inside the core.

Two reference applications exercise the engine:

- **Conventional 3D** (`extensions/conventional-3d`): lit meshes, materials, glTF import, a fly camera. A CI canary builds and runs it with every planetary crate deleted (ADR-0007).
- **Planetary** (`extensions/planetary`): a streamed, procedurally generated planet you fly down to from orbit, on a hex-grid or cube-sphere topology.

Everything here is checked against the code and the ADRs listed at the end. The authoritative, machine-checked version of the dependency rules is [dependency-direction.md](architecture/dependency-direction.md); the layer table lives in `xtask/src/layers.rs`.

## Layers

Dependencies point downward only. A crate may depend on workspace crates of a strictly lower tier (rule `tier-violation`, ADR-0002, ADR-0003).

| Tier | Crates | Role |
|------|--------|------|
| 0 | `engine-math`, `engine-diagnostics` | Math types, f64 `WorldPos` and coordinate frames (ADR-0004); logging, spans, frame stats and the planet-agnostic `DiagnosticsSnapshot` |
| 1 | `engine-core`, `engine-jobs`, `engine-procedural` | Engine lifecycle and configuration; prioritised, cancellable job scheduling; deterministic seeded noise and generators (ADR-0008) |
| 2 | `engine-platform`, `engine-assets`, `engine-world` | Windowing and input (the only `winit` user); resource IDs, typed handles and residency; entities, transforms and the scene hierarchy |
| 3 | `engine-spatial`, `engine-render`, `engine-streaming` | Spatial queries (hash grid by default, ADR-0006); backend-agnostic render scene, extraction, render graph and the backend trait; generic keyed streaming with priorities, budgets and coarse-parent fallback |
| 4 | `engine-render-wgpu`, `debug-ui` | The wgpu backend (the only crate that may use `wgpu` or `egui-wgpu`, ADR-0005, ADR-0018); the debug panel built on pure `egui` |
| 5 | `engine-sdk` | The public facade: `App`, `Runtime`, `run_app`, resources and re-exports (ADR-0009) |
| 6 | `conventional-3d`, `planetary-core` | Extensions that depend only on the SDK |
| 7 | `planetary-terrain` | Planetary terrain streaming, GPU mesh lifecycle, effective LOD cut |
| 8 | `planetary-app` | The planetary reference application and its examples |
| tooling | `xtask`, `ecs-bench` (`spikes/ecs-bench`) | Dependency checker and policy tests; the ECS comparison spike, the only crate allowed to use `hecs` (ADR-0017) |

Rules the tooling enforces (`cargo xtask check-deps`, `cargo xtask check-canary`, `cargo deny check`):

- Nothing under `crates/` depends on `extensions/` (`core-depends-on-extension`).
- `wgpu` and `egui-wgpu` appear only in `engine-render-wgpu`; `winit` only in `engine-platform`.
- One extension family never depends on another (`extension-cross-dependency`), and every workspace crate must be listed in the layer table (`unlisted-crate`).
- The conventional app's full dependency graph must not reach any planetary crate (`canary-planetary-reachable`).

## Rendering split

Rendering is divided so that no GPU API type escapes the lowest layer (ADR-0005):

- `engine-render` holds the render scene, extraction from the world, the render graph and the `RenderBackend` trait, plus a `NullBackend` that records frames for headless tests. It has no GPU types.
- `engine-render-wgpu` implements the trait on wgpu 30. Its offscreen and software-adapter constructors drive the image-diff tests; its present-mode configuration (ADR-0016) uses engine-owned types.
- Positions are f64 in the world and rendered camera-relative in f32, with a reversed-Z depth buffer (ADR-0004, ADR-0001).
- The overlay painter (feature `egui-overlay`) is part of the wgpu backend; the application only returns a snapshot (ADR-0018).

## SDK and application loop

`engine-sdk` is the surface extensions and applications use. An application implements the `App` trait (`setup`, `input`, `update`, and the optional per-frame `gpu_frame` hook of ADR-0009); `run_app` owns the window and the surface, and `Runtime` drives frames against any backend, so the same app runs windowed or headless on the null backend. See [sdk-usage.md](sdk-usage.md).

## Extension model and the canary

Specialised capabilities live in `extensions/`. They consume only the SDK, and the engine never names them. The planetary family (`planetary-core`, `planetary-terrain`, `planetary-app`) contains the topologies, terrain generator, LOD selection and the app; `conventional-3d` contains glTF import, scene building and camera controllers. The `canary` job in CI deletes `extensions/planetary` and builds, tests and runs the conventional app, proving the core and the conventional extension do not secretly need the planet (ADR-0007). `planetary-app` has a test (`tests/sdk_only.rs`) that it only uses the public SDK.

## Streaming and level of detail

- `engine-streaming` is generic: keyed requests with priority, budget class and cancellation, counted per-frame caps, and a coarse-ancestor fallback so rendering never blocks on generation (ADR-0008).
- Procedural generation is deterministic and golden-tested (ADR-0008, ADR-0015 for generator v2).
- Terrain LOD combines screen-space-error selection with tight bounds, horizon and frustum culling and budgets (ADR-0012), skirts plus shared-border sampling for crack-free borders (ADR-0010), and per-vertex geomorphing (ADR-0014). Pieces are drawn as index ranges over shared index buffers (ADR-0013).
- The planet is a generic `Topology`: the aperture-4 hex hierarchy is the default and the cube-sphere is the reference and fallback (ADR-0011).
- Scale, relief lighting and distance haze are application policy over the generic render API (ADR-0015).

## Diagnostics and overlay

`engine-diagnostics` provides tracing spans, frame stats and `DiagnosticsSnapshot` (gauges, budgets, histograms). `debug-ui` draws a snapshot with pure `egui`. The SDK, behind the off-by-default `egui-overlay` feature, owns the egui context and hands the output to the wgpu painter just before submit; `planetary-app` exposes it as an F3 toggle behind its `debug-overlay` feature. The overlay is read-only (ADR-0018). Telemetry, CSV logs and headless benchmarks are described in the [planetary-app README](../extensions/planetary/planetary-app/README.md).

## Scene state: no ECS

Scene code uses `engine_world::World` (generational ids, transforms and a hierarchy). A spike measured it against `hecs` and the decision is to keep `World` (ADR-0017); the criteria for revisiting are recorded there.

## Decision index

| ADR | Topic |
|-----|-------|
| ADR-0001 | Engine conventions (units, axes, handedness, depth) |
| ADR-0002 | Workspace crate layout, render split and licence |
| ADR-0003 | Dependency-direction enforcement and CI |
| ADR-0004 | `WorldPos`, coordinate frames and origin rebasing |
| ADR-0005 | wgpu behind an engine-owned backend trait; software-adapter image tests |
| ADR-0006 | Spatial index: hash grid first, benchmarked |
| ADR-0007 | Lit rendering, glTF import and the planetary-absent canary |
| ADR-0008 | Generic streaming and deterministic procedural generation |
| ADR-0009 | SDK per-frame GPU hook and mesh destruction |
| ADR-0010 | Terrain LOD skirts and shared-border sampling |
| ADR-0011 | Hex-grid planet topology |
| ADR-0012 | Terrain LOD selection: tight bounds, culling, budgets |
| ADR-0013 | Shared index buffers and index-range draws |
| ADR-0014 | Vertex morph attribute (geomorphing) |
| ADR-0015 | Scale, contrast and distance haze |
| ADR-0016 | Present-mode setting and window readout |
| ADR-0017 | No ECS in P0 |
| ADR-0018 | Debug overlay placement and font licences |

The full records, with status, alternatives and evidence, are in the [ADR index](architecture/adr/README.md). Assumptions are in [assumptions.md](assumptions.md) and known limitations in [limitations.md](limitations.md).
