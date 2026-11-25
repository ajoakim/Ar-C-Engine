# Roadmap: General-Purpose 3D Engine

## Overview

Build a small, stable Rust engine core and validate it with two reference apps. Precision and guardrails come first, then the render vertical slice, then world/assets, then a conventional app as a canary for core leakage. Generic streaming and procedural generation come next, so the planetary extension (cube-sphere, quadtree LOD, planetary camera) is built on them. A final hardening phase covers atmosphere, overlays, regression tests, docs and decision ADRs.

## Phases

**Phase Numbering:** Integer phases (1-8) are planned work. Decimal phases (e.g. 2.1) are urgent insertions.

- [ ] **Phase 1: Workspace, Toolchain & Guardrails** - Pinned toolchain, coarse crate layout, CI boundary checks, ADR template and conventions
- [ ] **Phase 2: Foundation (Core, Jobs, Diagnostics, Precision)** - Lifecycle, modules, job scheduler, diagnostics, WorldPos and origin rebasing
- [ ] **Phase 3: Platform, Render Backend & Vertical Slice** - Window/input, engine-owned render boundary with wgpu and null backends, RenderScene extraction, triangle via SDK
- [ ] **Phase 4: World, Spatial & Assets** - Entities and transforms, spatial queries, typed handles and residency state machine
- [ ] **Phase 5: Conventional Reference App (Canary)** - Lit meshes, glTF, basic-scene app on SDK only, building without planetary crates
- [ ] **Phase 6: Streaming & Procedural Framework** - Generic streaming with fallback, deterministic seeded generator layers
- [ ] **Phase 7: Planetary Extension & Terrain LOD** - Cube-sphere, quadtree SSE LOD, crack-free transitions, planetary camera, planetary app
- [ ] **Phase 8: Hardening & Decisions** - Atmosphere, debug overlays, regression/perf tests, docs, decision ADRs

## Phase Details

### Phase 1: Workspace, Toolchain & Guardrails
**Goal**: Developers can build a workspace whose architectural boundaries are enforced automatically from day one
**Depends on**: Nothing (first phase)
**Requirements**: WS-01, WS-02, WS-03, WS-04
**Success Criteria** (what must be TRUE):
  1. A fresh checkout builds on Windows and Linux with the pinned toolchain (Rust >= 1.95, edition 2024) via `rust-toolchain.toml`
  2. The workspace has coarse crates with a documented dependency direction
  3. CI fails when a core crate depends on an extension crate, or when wgpu/winit appear outside their designated crates (demonstrated by a failing negative test)
  4. An ADR template exists, and a conventions ADR fixes meters, axes, handedness and depth range
**Plans**: 4 plans

Plans:
- [ ] 01-01-PLAN.md — Pinned toolchain (1.99.0, MSRV 1.95, edition 2024) and 13-crate workspace with tiered path deps
- [ ] 01-02-PLAN.md — Dependency-direction doc, ADR template, ADR-0001 conventions, ADR-0002 layout/render split/license, ADR-0003 enforcement/CI
- [ ] 01-03-PLAN.md — xtask check-deps (cargo_metadata, rules R1-R6), negative fixtures, metadata/docs policy tests
- [ ] 01-04-PLAN.md — cargo-deny (wgpu/winit wrappers, licenses, advisories) and GitHub Actions ubuntu+windows CI

### Phase 2: Foundation (Core, Jobs, Diagnostics, Precision)
**Goal**: Applications can drive an engine lifecycle with modules, jobs and diagnostics, on a precision-safe position model
**Depends on**: Phase 1
**Requirements**: CORE-01, CORE-02, CORE-04, CORE-05, WORLD-02, WORLD-04
**Success Criteria** (what must be TRUE):
  1. An app can drive Initialize, Load, Running, Suspend and Shutdown, loading configuration and registering modules
  2. A developer can submit prioritized jobs, cancel them, and see higher-priority jobs run first
  3. Logs, tracing spans and per-frame timing are emitted by the diagnostics module
  4. Positions are stored as opaque `WorldPos` across multiple coordinate spaces, and origin rebasing preserves relative positions in tests
**Plans**: TBD

### Phase 3: Platform, Render Backend & Vertical Slice
**Goal**: A window shows geometry rendered through an engine-owned backend boundary, with camera-relative precision proven
**Depends on**: Phase 2
**Requirements**: REND-01, REND-02, REND-03, CORE-03, WORLD-03
**Success Criteria** (what must be TRUE):
  1. An app opens a window, receives input events, and renders a triangle through the public SDK facade
  2. The same frame runs headless on the null backend in tests, and wgpu types do not appear outside the backend crate
  3. Each frame is built by extracting a geometry-agnostic camera-relative `RenderScene` and executing a render graph
  4. A test with the camera at 6.4e6 m from the origin shows no visible jitter
**Plans**: TBD
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
**Plans**: TBD

### Phase 5: Conventional Reference App (Canary)
**Goal**: A conventional 3D scene runs on the SDK alone, proving the core is general-purpose
**Depends on**: Phase 4
**Requirements**: REND-04, REND-05, APP-01, PLAN-01
**Success Criteria** (what must be TRUE):
  1. The app renders a scene of lit, textured, materialed meshes with a directional light and reverse-Z depth
  2. The app loads glTF meshes and textures from disk
  3. A user can fly a camera through the scene using only SDK APIs
  4. A CI job builds and runs the app with all planetary crates absent
**Plans**: TBD
**UI hint**: yes

### Phase 6: Streaming & Procedural Framework
**Goal**: Data can be generated and streamed asynchronously and deterministically without ever stalling rendering
**Depends on**: Phase 4
**Requirements**: STRM-01, STRM-02, PROC-01, PROC-02
**Success Criteria** (what must be TRUE):
  1. Stream requests carry key, priority, budget class and cancellation token, and cancelled requests free their budget
  2. Under artificial slow loading, frames keep their time budget and coarse parent data is shown until refinement is resident
  3. Generator layers are pure functions of stable ID and seed, with independent layer seeds
  4. Golden-hash tests show identical output across thread counts and platforms (Windows and Linux CI)
**Plans**: TBD

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
**Plans**: TBD
**UI hint**: yes

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
**Plans**: TBD
**UI hint**: yes

## Progress

**Execution Order:** 1 → 2 → 3 → 4 → 5 and 6 (parallelizable) → 7 → 8

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Workspace, Toolchain & Guardrails | 0/4 | Not started | - |
| 2. Foundation | 0/TBD | Not started | - |
| 3. Platform, Render Backend & Vertical Slice | 0/TBD | Not started | - |
| 4. World, Spatial & Assets | 0/TBD | Not started | - |
| 5. Conventional Reference App | 0/TBD | Not started | - |
| 6. Streaming & Procedural Framework | 0/TBD | Not started | - |
| 7. Planetary Extension & Terrain LOD | 0/TBD | Not started | - |
| 8. Hardening & Decisions | 0/TBD | Not started | - |
