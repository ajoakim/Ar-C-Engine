# Roadmap: General-Purpose 3D Engine

## Overview

Build a small, stable Rust engine core and validate it with two reference apps. Precision and guardrails come first, then the render vertical slice, then world/assets, then a conventional app as a canary for core leakage. Generic streaming and procedural generation come next, so the planetary extension (cube-sphere, quadtree LOD, planetary camera) is built on them. A final hardening phase covers atmosphere, overlays, regression tests, docs and decision ADRs.

## Phases

**Phase Numbering:** Integer phases (1-8) are planned work. Decimal phases (e.g. 2.1) are urgent insertions.

- [x] **Phase 1: Workspace, Toolchain & Guardrails** - Pinned toolchain, coarse crate layout, CI boundary checks, ADR template and conventions (completed 2026-11-25)
- [x] **Phase 2: Foundation (Core, Jobs, Diagnostics, Precision)** - Lifecycle, modules, job scheduler, diagnostics, WorldPos and origin rebasing (completed 2026-11-25)
- [x] **Phase 3: Platform, Render Backend & Vertical Slice** - Window/input, engine-owned render boundary with wgpu and null backends, RenderScene extraction, triangle via SDK (completed 2026-11-26)
- [x] **Phase 4: World, Spatial & Assets** - Entities and transforms, spatial queries, typed handles and residency state machine
 (completed 2026-11-26)
- [x] **Phase 5: Conventional Reference App (Canary)** - Lit meshes, glTF, basic-scene app on SDK only, building without planetary crates
 (completed 2026-11-26)
- [x] **Phase 6: Streaming & Procedural Framework** - Generic streaming with fallback, deterministic seeded generator layers
 (completed 2026-11-26)
- [x] **Phase 7: Planetary Extension & Terrain LOD** - Cube-sphere, quadtree SSE LOD, crack-free transitions, planetary camera, planetary app (plans complete 2026-11-25; gaps G1-G3 closed by Phase 7.1)
- [x] **Phase 07.1: Hex-Grid Planet Topology & Descent Performance (INSERTED)** - Hex topology, descent performance, flight feedback fixes (completed 2026-11-26, human flight approved with caveats)
- [ ] **Phase 8: Hardening & Decisions** - Atmosphere, debug overlays, regression/perf tests, docs, decision ADRs

## Phase Details

### Phase 1: Workspace, Toolchain & Guardrails
**Goal**: Developers can build a workspace whose architectural boundaries are enforced automatically from day one
**Depends on**: Nothing (first phase)
**Requirements**: WS-01, WS-02, WS-03, WS-04
**Success Criteria** (what must be TRUE):
  1. A fresh checkout builds on Windows, Linux and macOS with the pinned toolchain (Rust >= 1.95, edition 2024) via `rust-toolchain.toml`
  2. The workspace has coarse crates with a documented dependency direction
  3. CI fails when a core crate depends on an extension crate, or when wgpu/winit appear outside their designated crates (demonstrated by a failing negative test)
  4. An ADR template exists, and a conventions ADR fixes meters, axes, handedness and depth range
**Plans**: 4 plans

Plans:
- [x] 01-01-PLAN.md — Pinned toolchain (1.99.0, MSRV 1.95, edition 2024) and 13-crate workspace with tiered path deps
- [x] 01-02-PLAN.md — Dependency-direction doc, ADR template, ADR-0001 conventions, ADR-0002 layout/render split/license, ADR-0003 enforcement/CI
- [x] 01-03-PLAN.md — xtask check-deps (cargo_metadata, rules R1-R6), negative fixtures, metadata/docs policy tests
- [x] 01-04-PLAN.md — cargo-deny (wgpu/winit wrappers, licenses, advisories) and GitHub Actions ubuntu+windows CI

### Phase 2: Foundation (Core, Jobs, Diagnostics, Precision)
**Goal**: Applications can drive an engine lifecycle with modules, jobs and diagnostics, on a precision-safe position model
**Depends on**: Phase 1
**Requirements**: CORE-01, CORE-02, CORE-04, CORE-05, WORLD-02, WORLD-04
**Success Criteria** (what must be TRUE):
  1. An app can drive Initialize, Load, Running, Suspend and Shutdown, loading configuration and registering modules
  2. A developer can submit prioritized jobs, cancel them, and see higher-priority jobs run first
  3. Logs, tracing spans and per-frame timing are emitted by the diagnostics module
  4. Positions are stored as opaque `WorldPos` across multiple coordinate spaces, and origin rebasing preserves relative positions in tests
**Plans**: 6 plans
Plans:
- [x] 02-01-PLAN.md — Pin Phase 2 third-party deps (glam, tracing, crossbeam, serde, ron, thiserror); guardrails stay green (wave 1)
- [x] 02-02-PLAN.md — engine-math: opaque WorldPos, coordinate frames, origin rebasing, ADR-0004 (wave 2)
- [x] 02-03-PLAN.md — engine-diagnostics: subscriber install, span-timing layer, FrameClock/FrameStats (wave 2)
- [x] 02-04-PLAN.md — engine-jobs: priority scheduler, cancel tokens, deterministic tests (wave 2)
- [x] 02-05-PLAN.md — engine-core: lifecycle state machine, RON EngineConfig, Module trait, registry (wave 2)
- [x] 02-06-PLAN.md — engine-core: Engine driver, end-to-end lifecycle tests, example app (wave 3)

### Phase 3: Platform, Render Backend & Vertical Slice
**Goal**: A window shows geometry rendered through an engine-owned backend boundary, with camera-relative precision proven
**Depends on**: Phase 2
**Requirements**: REND-01, REND-02, REND-03, CORE-03, WORLD-03
**Success Criteria** (what must be TRUE):
  1. An app opens a window, receives input events, and renders a triangle through the public SDK facade
  2. The same frame runs headless on the null backend in tests, and wgpu types do not appear outside the backend crate
  3. Each frame is built by extracting a geometry-agnostic camera-relative `RenderScene` and executing a render graph
  4. A test with the camera at 6.4e6 m from the origin shows no visible jitter
**Plans**: 7 plans
Plans:
- [x] 03-01-PLAN.md — Pin wgpu/winit/rwh/pollster/bytemuck, deny+check-deps green, ADR-0005 rendering API (wave 1)
- [x] 03-02-PLAN.md — engine-render boundary: handles, RenderBackend, FramePacket, ResourceTable, NullBackend (wave 2)
- [x] 03-03-PLAN.md — engine-platform: InputEvent translation, PlatformWindow, winit ApplicationHandler run loop (wave 2)
- [x] 03-04-PLAN.md — engine-render: Camera, camera-relative extract, render graph, 6.4e6 m precision tests (wave 3)
- [x] 03-05-PLAN.md — engine-render-wgpu: WgpuBackend, WGSL pipeline, reverse-Z, surface handling (wave 3)
- [x] 03-06-PLAN.md — engine-sdk: App, Runtime, run_app, headless tests, triangle example (wave 4)
- [ ] 03-07-PLAN.md — Phase gate + MANUAL triangle window smoke run (wave 5, human verify)
**UI hint**: yes

### Phase 4: World, Spatial & Assets
**Goal**: Developers can build scenes of stable entities, query them spatially, and manage resources with residency states
**Depends on**: Phase 3
**Requirements**: WORLD-01, WORLD-05, ASSET-01, ASSET-02, ASSET-03
**Success Criteria** (what must be TRUE):
  1. Entities have stable generational IDs, stale IDs are rejected, and child transforms follow parents
  2. Visibility and region-lookup queries work through the pluggable spatial interface with one default backend
  3. Resources are referenced by stable IDs and typed handles
  4. Resources move through the residency state machine, with budgets and per-frame upload caps enforced and observable in tests
**Plans**: 8 plans

Plans:
- [x] 04-01-PLAN.md — Pin slotmap/uuid, wire world/spatial/assets manifests, ADR-0006 spatial index hash grid first (wave 1)
- [x] 04-02-PLAN.md — engine-world: EntityId, Transform, World spawn/despawn/hierarchy with stale/cycle rejection (wave 2)
- [x] 04-03-PLAN.md — engine-assets: AssetId (uuid v5, golden-pinned), Handle<T>, AssetRegistry<T> (wave 2)
- [x] 04-04-PLAN.md — engine-world: WorldTransform, dirty-flag top-down update, rebase, 6.4e6 m precision tests (wave 3)
- [x] 04-05-PLAN.md — engine-spatial: f64 volumes, SpatialIndex trait, LinearIndex oracle backend (wave 3)
- [x] 04-06-PLAN.md — engine-assets: residency transition table, ResidencyManager budgets/upload cap/LRU/stats (wave 3)
- [x] 04-07-PLAN.md — engine-spatial: HashGridIndex default backend + oracle and trait-object tests (wave 4)
- [x] 04-08-PLAN.md — engine-assets: AsyncLoader on JobSystem (load, cancel, panic, stale generation) (wave 4)

### Phase 5: Conventional Reference App (Canary)
**Goal**: A conventional 3D scene runs on the SDK alone, proving the core is general-purpose
**Depends on**: Phase 4
**Requirements**: REND-04, REND-05, APP-01, PLAN-01
**Success Criteria** (what must be TRUE):
  1. The app renders a scene of lit, textured, materialed meshes with a directional light and reverse-Z depth
  2. The app loads glTF meshes and textures from disk
  3. A user can fly a camera through the scene using only SDK APIs
  4. A CI job builds and runs the app with all planetary crates absent
**Plans**: 10 plans

Plans:
- [x] 05-01-PLAN.md — Pin gltf/image, conventional-3d manifest, cargo-deny wrappers, ADR-0007
- [x] 05-02-PLAN.md — xtask check-canary (transitive closure) + canary-leak negative fixture + docs
- [x] 05-03-PLAN.md — engine-render contract: Material/MaterialTable/DirectionalLight/TextureDesc, PosNormalUv, index buffers, texture table, NullBackend
- [x] 05-04-PLAN.md — engine-render frame flow: material/light-aware extract with normal matrix, PipelineSet, lit packet validation
- [x] 05-05-PLAN.md — engine-render-wgpu lit textured pipeline, textures, index draws, readback, GPU tests (Arc B580)
- [x] 05-06-PLAN.md — engine-sdk surface: re-exports, create_lit_mesh/create_texture, materials/light, two-pipeline Runtime
- [x] 05-07-PLAN.md — conventional-3d bounded glTF/image import, procedural fallback, textured_cube.glb fixture
- [x] 05-08-PLAN.md — conventional-3d ModelLoader on JobSystem + AssetRegistry, FlyCamera
- [x] 05-09-PLAN.md — conventional-3d SceneBuilder, basic_scene example (windowed + --headless), SDK-only policy test
- [x] 05-10-PLAN.md — CI canary job + steps, full local CI run, GPU tests, human window check
**UI hint**: yes

### Phase 6: Streaming & Procedural Framework
**Goal**: Data can be generated and streamed asynchronously and deterministically without ever stalling rendering
**Depends on**: Phase 4
**Requirements**: STRM-01, STRM-02, PROC-01, PROC-02
**Success Criteria** (what must be TRUE):
  1. Stream requests carry key, priority, budget class and cancellation token, and cancelled requests free their budget
  2. Under artificial slow loading, frames keep their time budget and coarse parent data is shown until refinement is resident
  3. Generator layers are pure functions of stable ID and seed, with independent layer seeds
  4. Golden-hash tests show identical output across thread counts and platforms (Windows, Linux and macOS CI)
**Plans**: 6 plans

Plans:
- [x] 06-01-PLAN.md — Scaffold engine-streaming (tier 3) and engine-procedural (tier 1): workspace, Cargo.lock, xtask layers, dependency doc, SDK re-exports, ADR-0008
- [x] 06-02-PLAN.md — Procedural primitives: splitmix64/fnv1a64 KATs, layer seeds, exact-arithmetic value noise/fBm, banned-ops scan
- [x] 06-03-PLAN.md — Streaming pipeline: StreamKey/StreamRequest/StreamSource, reject-on-full priority queue, budget-freeing cancel, capped non-blocking update
- [x] 06-04-PLAN.md — Generator trait, LayerStack, canonical output hashing, golden hashes across 1/2/4/8 threads and layer compositions
- [x] 06-05-PLAN.md — Pinned coarse-ancestor fallback, resolve(), pin-aware LRU eviction, gate-driven slow-load and pin terminal-path tests
- [x] 06-06-PLAN.md — Streamed procedural determinism at 1 vs 4 JobSystem workers, goldens recorded in ADR-0008, full phase gate

### Phase 7: Planetary Extension & Terrain LOD
**Goal**: A user can fly from orbit to the surface of a procedurally generated planet with crack-free, streamed terrain
**Depends on**: Phase 5, Phase 6
**Requirements**: PLAN-02, PLAN-03, PLAN-04, PLAN-05, PLAN-06, PLAN-07, APP-02
**Success Criteria** (what must be TRUE):
  1. The planet is a cube-sphere with stable packed patch IDs, and adjacency tests pass, including reciprocity across all face edges
  2. Terrain refines and coarsens through a 2:1-balanced quadtree by screen-space error, with no popping flicker at hysteresis boundaries
  3. No cracks are visible at LOD boundaries, cube-face edges or corners from the seam test cameras
  4. Neighboring patch borders agree because terrain comes from layered planet-space generators
  5. A user can navigate with the planetary camera from orbit to surface in the planetary app, which uses the public SDK only
  6. A scripted orbit-to-surface descent stays within the frame-time budget without stalls
**Plans:** 10 plans
Plans:
- [x] 07-01-PLAN.md — SDK gpu_frame hook + destroy_mesh, scaffold planetary-terrain/planetary-app, xtask/docs/CI wiring, ADR-0009 and ADR-0010 (proposed)
- [x] 07-02-PLAN.md — planetary-core: packed PatchId, face bases, cross-face adjacency with exhaustive reciprocity + oracle tests
- [x] 07-03-PLAN.md — planetary-core: exact-op equal-angle mapping, locate, layered planet-space generators, goldens, banned-ops scan
- [x] 07-04-PLAN.md — planetary-core: SSE LOD selection with hysteresis, cross-face 2:1 balance, leaf cap, validators
- [x] 07-05-PLAN.md — planetary-core: patch mesh with skirts and per-patch origin, validated payload, CPU seam proofs, ADR-0010 evidence
- [x] 07-06-PLAN.md — planetary-terrain: planetary camera (altitude-scaled speed, parallel transport, clearance, near plane)
- [x] 07-07-PLAN.md — planetary-terrain: effective (residency-limited) cut with coarsen-to-balance, request planning
- [x] 07-08-PLAN.md — planetary-terrain: PatchKey/PatchSource/GpuUploader/TerrainSystem streaming glue, headless integration test
- [x] 07-09-PLAN.md — planetary-app: PlanetaryApp, DescentScript, planetary_flight example, sdk_only policy test, CI headless step
- [x] 07-10-PLAN.md — scripted descent test (PLAN-07), ADR-0010 accepted, validation sign-off, human window-flight checkpoint
**UI hint**: yes

### Phase 07.1: Hex-Grid Planet Topology & Descent Performance (INSERTED)

**Goal**: Descent works at real parameters (detail streams in, frames hold budget, no crash, measurable) on a hex-grid planet in the spirit of H3: each hex gives way to its constituent parts as you descend and aggregates on ascent
**Why inserted**: Phase 7 human flight (2026-11-25) found detail not streaming in during descent, a stuttering/crashing scripted descent and no frame-rate-vs-altitude measurement; the user chose an H3-inspired hex topology. See `.planning/research/HEX-TOPOLOGY.md` and `07-VERIFICATION.md` (gaps G1-G3).
**Depends on:** Phase 7
**Requirements**: HEX-01, HEX-02, HEX-03, PERF-01, PERF-02 (closes Phase 7 gaps G1-G3; PLAN-02/03 are re-scoped from cube-sphere/quadtree to the `Topology` trait with both implementations)
**Success Criteria** (what must be TRUE):
  1. Telemetry reports frame time (avg/p95/max), FPS, altitude, distance to surface and selection/streaming stats per frame, with a CSV log and a headless descent benchmark at the real default parameters (no reduced level)
  2. The cause of the descent crash/stutter is diagnosed from evidence and fixed; the scripted descent completes in debug and release without a crash
  3. Selection uses tight per-cell height bounds, horizon and frustum culling and a node-visit budget, so detail converges as the camera descends at the real default max level, within the 1024-draw cap
  4. Sphere spikes (cell nesting/coverage, pentagons, IDs/adjacency, skirt gap bound) pass, or the documented fallback is chosen, at an explicit go/no-go with the user before the full hex build
  5. A `Topology` trait has cube-sphere and half-cell hex implementations; the hex cut covers the sphere exactly once with balanced neighbors, and its seam/coverage proofs (with negative controls) pass
  6. The hex planet runs the full scripted orbit-to-surface descent at real defaults in the planetary app (SDK only), and the user's window flight confirms detail rendering in on descent and aggregating on ascent
  7. The flight feedback items are fixed or measured with evidence and confirmed in the human flight: flicker cause established (plan 20), seamless level changes via geomorphing (plans 21-24), scale and contrast with mountain ranges, colour bands, relief light and distance haze (plan 25; the haze is a generic precursor that Phase 8 atmosphere PLAN-08 supersedes), present mode and window readout (plan 26), hex streaming speed (plan 27)
**Plans:** 27/27 plans executed

Plans:
- [x] 07.1-01-PLAN.md — Telemetry (Clock/SimClock/WallClock, CSV, --bench-descent at real defaults) + evidence-based crash/stutter diagnosis on the baseline (G3, G2 diagnosis)
- [x] 07.1-02-PLAN.md — planetary-core selection fixes: extruded-footprint distance, resident-ancestor ranges + residual, horizon/frustum culling, node/leaf budgets, no tau reruns, max_level 16 (G1)
- [x] 07.1-03-PLAN.md — planetary-terrain: measured ranges into selection, explicit 1024-draw budget, frustum planes, telemetry columns, crash root-cause fixes (G1/G2)
- [x] 07.1-04-PLAN.md — Spikes S1 (diamond IDs, 12 pentagons, adjacency) + S2 (exact-op projection bit identity), test-only
- [x] 07.1-05-PLAN.md — App frustum wiring, descent + ascent suite at REAL defaults, debug/release bench evidence, ADR-0012, ADR-0010 amendment (closes G1/G2 for the cube-sphere)
- [x] 07.1-06-PLAN.md — Spikes S3 (half-cell coverage on the sphere) + S4 (skirt gap on edges and half diagonals), test-only
- [x] 07.1-07-PLAN.md — CHECKPOINT: go/no-go on the half-cell hex topology (GO / FALLBACK triangles / STOP), ADR-0011 proposed
- [x] 07.1-08-PLAN.md — Topology trait + CubeSphere + generic select/balance + v3 cell payload (go-dependent)
- [x] 07.1-09-PLAN.md — Generic shared index buffers + index-range draws in engine-render/wgpu/sdk, ADR-0013 (go-dependent)
- [x] 07.1-10-PLAN.md — Hex CellId + icosahedron/diamond tables + parents/children/neighbours (go-dependent)
- [x] 07.1-11-PLAN.md — planetary-terrain generic keys/requests/effective cut (go-dependent)
- [x] 07.1-12-PLAN.md — Hex exact-op projection, sample lattice, locate, footprints, goldens (go-dependent)
- [x] 07.1-13-PLAN.md — Generic TerrainSystem<T>, v3 payloads, index-range draws, debug-cells palette (go-dependent)
- [x] 07.1-14-PLAN.md — Hex balance closure, half-cell pieces, coverage proof with negative controls (go-dependent)
- [x] 07.1-15-PLAN.md — Hex sector meshes, 8 index variants, skirts incl. diagonals, seam proofs with negative control (go-dependent)
- [x] 07.1-16-PLAN.md — HexSphere: Topology, hex selection at real parameters, legacy cube paths removed (go-dependent)
- [x] 07.1-17-PLAN.md — App on hex (default) + cube fallback, hex descent/ascent at real defaults, benches, ADR-0011 accepted, docs (go-dependent)
- [x] 07.1-16b (summary only, no plan file) — parents-only hex balance closure, hex_selection_reaches_max_level un-ignored
- [x] 07.1-19-PLAN.md — Streaming throughput: windowed-realistic harness, adaptive caps, nearest-first ordering, coarse fallback partition, lag bounds (added after the Phase 7.1 flight feedback)
- [x] 07.1-20-PLAN.md — Flicker diagnosis with counters (level_changes, appear/disappear, cull_toggles, overlap_pairs, flipflops), absolute invariants, targeted fix or documented finding (reconciled with the existing 0.2 selection hysteresis)
- [x] 07.1-21-PLAN.md — Generic render path: morph vertex layout (stride 52), per-draw morph, per-frame distance fog, identity tests (morph 0 and density 0 bit-identical)
- [x] 07.1-22-PLAN.md — Morph targets: parent-mesh surface at the vertex direction for cube and hex (oracle, seam proof with negative controls)
- [x] 07.1-23-PLAN.md — Cell payload v4, morph upload and byte budgets, ADR-0014
- [x] 07.1-24-PLAN.md — Morph driving from the selector refine distance, child appears at morph 1, handover continuity proofs and benches
- [x] 07.1-25-PLAN.md — Scale and contrast: generator v2 ridge layer, bounds, colour ramp and slope rock, relief light, distance haze (precursor to Phase 8 atmosphere), ADR-0015
- [x] 07.1-26-PLAN.md — Present mode setting, startup info and window title readout, strict flags, measured table, ADR-0016
- [x] 07.1-27-PLAN.md — Hex prepare speed: profile first, orientation-safe re-selection skip, equality proofs, stated targets
- [x] 07.1-18-PLAN.md — CHECKPOINT (last wave, depends on 17 and 19-27): full gate + human window flight with telemetry; the flight covers every Pending (human) item of plans 20-27


### Phase 8: Hardening & Decisions
**Goal**: The prototype is observable, regression-tested, documented, and its open technical decisions are recorded
**Depends on**: Phase 7
**Requirements**: PLAN-08, QA-01, QA-02, QA-03, WS-05
**Success Criteria** (what must be TRUE):
  1. The planet shows a minimal atmosphere (first item cut if schedule slips)
  2. Debug overlays show LOD state and streaming/memory budget state
  3. CI runs unit, integration, criterion performance, and image-diff regression tests on a software adapter
  4. Architecture, assumptions, limitations and SDK usage docs exist
  5. ADRs for rendering API, ECS, spatial backend and LOD transition technique each cite a prototype or benchmark
**Plans**: 12 plans
Plans:
- [x] 08-01-PLAN.md — Confirm hosted CI green on pushed 07.1 commits, record cross-OS goldens, fix failures
- [x] 08-02-PLAN.md — Carry-overs (07-VERIFICATION, STATE) and workspace dependency pins
- [x] 08-03-PLAN.md — Software-adapter constructor, image-diff harness, goldens, CI image job
- [x] 08-04-PLAN.md — Criterion benches (spatial, selection, mesh, streaming)
- [ ] 08-05-PLAN.md — ecs-bench spike, ADR-0017, amendments to ADR 0005/0006/0010
- [ ] 08-06-PLAN.md — DiagnosticsSnapshot, debug-ui crate, planetary snapshot builder
- [ ] 08-07-PLAN.md — Overlay policy: font licences, egui-wgpu wrapper, xtask R2 fixture, ADR-0018
- [ ] 08-08-PLAN.md — egui-wgpu painter, SDK hand-off, F3 overlay
- [ ] 08-09-PLAN.md — Architecture, assumptions, limitations, SDK usage docs
- [ ] 08-10-PLAN.md — Minimal atmosphere limb ring, ADR-0019 (cuttable)
- [ ] 08-11-PLAN.md — Non-blocking bench job, hosted goldens, CI evidence
- [ ] 08-12-PLAN.md — Final gate, human flight, HLD.md decision, bookkeeping
**UI hint**: yes

## Progress

**Execution Order:** 1 → 2 → 3 → 4 → 5 and 6 (parallelizable) → 7 → 8

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Workspace, Toolchain & Guardrails | 4/4 | Complete    | 2026-11-25 |
| 2. Foundation | 6/6 | Complete    | 2026-11-25 |
| 3. Platform, Render Backend & Vertical Slice | 6/7 | Complete    | 2026-11-26 |
| 4. World, Spatial & Assets | 8/8 | Complete    | 2026-11-26 |
| 5. Conventional Reference App | 10/10 | Complete    | 2026-11-26 |
| 6. Streaming & Procedural Framework | 6/6 | Complete    | 2026-11-26 |
| 7. Planetary Extension & Terrain LOD | 10/10 | Complete | 2026-11-25 |
| 07.1. Hex-Grid Planet Topology & Descent Performance | 27/27 | Complete (human flight approved with caveats) | 2026-11-26 |
| 8. Hardening & Decisions | 4/12 | In Progress|  |
