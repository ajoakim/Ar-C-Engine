# Requirements: General-Purpose 3D Engine

**Defined:** 2026-11-25
**Core Value:** The engine core must remain general-purpose and independent of any specialized capability (planetary rendering, physics, scripting, etc.) — specialized implementations are validated through demanding reference applications but never become assumptions embedded in the core.

## v1 Requirements

### Workspace & Guardrails

- [x] **WS-01**: Developer can build the whole workspace on a pinned Rust toolchain (>= 1.95, edition 2024) via `rust-toolchain.toml` on Windows, Linux and macOS
- [x] **WS-02**: Workspace is split into coarse crates with documented dependency direction (core never depends on extensions)
- [x] **WS-03**: CI fails if any core crate depends on an extension crate, or if wgpu/winit appear outside their designated crates
- [x] **WS-04**: An ADR template exists and a conventions ADR fixes units (meters), axes, handedness, and depth range
- [x] **WS-05**: Open technical decisions (rendering API, ECS, spatial backend, LOD transition technique) are each recorded as ADRs backed by prototype or benchmark

### Engine Core

- [x] **CORE-01**: Application can drive the engine lifecycle Initialize → Load → Running → Suspend → Shutdown
- [x] **CORE-02**: Application can load configuration and register modules with the engine
- [x] **CORE-03**: Application can use the engine through a minimal public SDK facade without depending on internal crates
- [x] **CORE-04**: Developer can submit prioritized, cancellable jobs to a job scheduler
- [x] **CORE-05**: Developer gets structured logging, tracing spans, and frame timing from a diagnostics module

### World & Precision

- [x] **WORLD-01**: Developer can create entities with stable generational identity and transform hierarchy
- [x] **WORLD-02**: Positions are stored in high-precision world coordinates (opaque `WorldPos`) across multiple coordinate spaces
- [x] **WORLD-03**: Renderer receives camera-relative f32 transforms, and a test at 6.4e6 m from the origin shows no visible jitter
- [x] **WORLD-04**: Engine supports origin rebasing for large worlds
- [x] **WORLD-05**: Developer can run spatial queries (visibility, region lookup) through a pluggable spatial interface with one default backend

### Rendering

- [x] **REND-01**: Rendering goes through an engine-owned backend boundary, with wgpu as the first implementation and a null backend for headless tests
- [x] **REND-02**: Application can open a window and receive input events through a platform layer
- [x] **REND-03**: Frame is built by extracting a geometry-agnostic, camera-relative `RenderScene` and executing a render graph
- [x] **REND-04**: Application can render lit meshes with materials and a directional light using reverse-Z depth
- [x] **REND-05**: Application can load glTF meshes and textures

### Assets & Residency

- [x] **ASSET-01**: Resources have stable identifiers and typed handles
- [x] **ASSET-02**: Resources move through a CPU/GPU residency state machine (Unloaded → Requested → Generating → CpuReady → Uploading → GpuResident → Evicting / Failed)
- [x] **ASSET-03**: Resource caching and memory budgets are enforced, with per-frame upload caps

### Streaming & Procedural Generation

- [x] **STRM-01**: Generic streaming requests carry key, priority, budget class, and cancellation token
- [x] **STRM-02**: Rendering never blocks on streaming; coarse parent data is used as fallback until refinement is resident
- [x] **PROC-01**: Developer can define deterministic, independently seeded generator layers as pure functions of stable ID and seed
- [x] **PROC-02**: Golden-hash tests confirm generation output is identical across thread counts and platforms

### Planetary Extension

- [x] **PLAN-01**: Planetary code lives only in extension crates; the conventional app builds and runs with those crates absent
- [ ] **PLAN-02**: Planet surface is represented as an icosahedral hexagonal cell hierarchy (aperture 4, 12 pentagons) with stable packed cell IDs and a tested neighbour table; the cube-sphere is retained as the reference topology behind a `Topology` trait (Phase 7.1, ADR-0011)
- [ ] **PLAN-03**: Planet cells form a hierarchy with level-difference-1 balance and screen-space-error LOD selection with hysteresis, tight bounds and horizon/frustum culling (cube quadtree built in Phase 7; hex hierarchy in Phase 7.1)
- [ ] **PLAN-04**: Terrain LOD transitions are crack-free, including across icosahedron edges, vertices and pentagons, and across half-cell borders (cube-face edges and corners for the reference topology)
- [ ] **PLAN-05**: Terrain is generated from layered planet-space generators so patch borders agree
- [ ] **PLAN-06**: User can navigate with a planetary camera from orbit to surface
- [ ] **PLAN-07**: A scripted orbit-to-surface descent holds the target frame-time budget without stalls
- [ ] **PLAN-08**: Planet shows a minimal atmosphere (first item to cut if schedule slips)
- [ ] **HEX-01**: Planet can use an H3-inspired hex-grid topology on an icosahedron (half-cell scheme: each hex gives way to a centre hex plus six half-covered edge hexes, 12 pentagons) with stable packed cell IDs and tested parent/child/neighbor derivation, built with exact arithmetic only (ADR-0008)
- [x] **HEX-02**: The hex drawn cut covers the sphere exactly once with balanced neighbors, and terrain is crack-free at cell borders, pentagons and mixed levels (seam and coverage proofs with negative controls)
- [x] **HEX-03**: A `Topology` trait has the cube-sphere quadtree and the hex grid as implementations; the cube-sphere stays as test oracle and fallback until hex passes the full descent
- [ ] **PERF-01**: Telemetry reports frame time (avg/p95/max), FPS, altitude, distance to surface and selection/streaming stats, with a CSV log and a headless descent benchmark at the real default parameters
- [x] **PERF-02**: The scripted orbit-to-surface descent at the real default LOD parameters streams detail in as the camera descends, aggregates on ascent, holds the frame budget and completes without a crash in debug and release

### Reference Apps & Quality

- [x] **APP-01**: A conventional 3D reference app runs against the public SDK only
- [ ] **APP-02**: A planetary reference app runs against the public SDK only
- [ ] **QA-01**: Unit, integration, performance (criterion), and regression (image-diff on software adapter) tests exist and run in CI
- [ ] **QA-02**: Debug overlays show LOD and streaming/memory budget state
- [ ] **QA-03**: Architecture, assumptions, limitations, and SDK usage are documented

## v2 Requirements

### Rendering

- **REND-V2-01**: Shadow maps
- **REND-V2-02**: Geomorphing for LOD transitions
- **REND-V2-03**: Asset import pipeline

### Planetary

- **PLAN-V2-01**: Geospatial data ingestion (NASA/USGS/Copernicus)
- **PLAN-V2-02**: Persistent world editing / modification overlays
- **PLAN-V2-03**: Weather, day-night cycle, seasons, clouds

### Platform

- **TOOL-V2-01**: Editor / visual development environment
- **SIM-V2-01**: Physics integration
- **SIM-V2-02**: Gameplay scripting
- **SIM-V2-03**: Comprehensive animation system
- **ECS-V2-01**: ECS adoption, if the ADR spike justifies it

## Out of Scope

| Feature | Reason |
|---------|--------|
| Editor | Deferred until runtime and SDK stabilize |
| Physics engine integration | Optional future capability |
| Scripting VM | Not committed before evaluation |
| ECS commitment in P0 | Must not prematurely lock in one implementation |
| Geospatial ingestion pipeline | P2 |
| Plugin marketplace / dynamic plugin ABI | No real requirement yet |
| Ray tracing / GPU-driven rendering | Not needed for initial prototype |
| Planetary types in engine core | Violates Core Value |
| Post-processing stack | Not needed for P0 |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| WS-01 | Phase 1 | Complete |
| WS-02 | Phase 1 | Complete |
| WS-03 | Phase 1 | Complete |
| WS-04 | Phase 1 | Complete |
| WS-05 | Phase 8 | Complete |
| CORE-01 | Phase 2 | Complete |
| CORE-02 | Phase 2 | Complete |
| CORE-03 | Phase 3 | Complete |
| CORE-04 | Phase 2 | Complete |
| CORE-05 | Phase 2 | Complete |
| WORLD-01 | Phase 4 | Complete |
| WORLD-02 | Phase 2 | Complete |
| WORLD-03 | Phase 3 | Complete |
| WORLD-04 | Phase 2 | Complete |
| WORLD-05 | Phase 4 | Complete |
| REND-01 | Phase 3 | Complete |
| REND-02 | Phase 3 | Complete |
| REND-03 | Phase 3 | Complete |
| REND-04 | Phase 5 | Complete |
| REND-05 | Phase 5 | Complete |
| ASSET-01 | Phase 4 | Complete |
| ASSET-02 | Phase 4 | Complete |
| ASSET-03 | Phase 4 | Complete |
| STRM-01 | Phase 6 | Complete |
| STRM-02 | Phase 6 | Complete |
| PROC-01 | Phase 6 | Complete |
| PROC-02 | Phase 6 | Complete |
| PLAN-01 | Phase 5 | Complete |
| PLAN-02 | Phase 7 | Pending |
| PLAN-03 | Phase 7 | Pending |
| PLAN-04 | Phase 7 | Pending |
| PLAN-05 | Phase 7 | Pending |
| PLAN-06 | Phase 7 | Pending |
| PLAN-07 | Phase 7 | Pending |
| HEX-01 | Phase 7.1 | Pending |
| HEX-02 | Phase 7.1 | Complete |
| HEX-03 | Phase 7.1 | Complete |
| PERF-01 | Phase 7.1 | Pending |
| PERF-02 | Phase 7.1 | Complete |
| PLAN-08 | Phase 8 | Pending |
| APP-01 | Phase 5 | Complete |
| APP-02 | Phase 7 | Pending |
| QA-01 | Phase 8 | Pending |
| QA-02 | Phase 8 | Pending |
| QA-03 | Phase 8 | Pending |

**Coverage:**
- v1 requirements: 45 total (40 original + HEX-01..03 and PERF-01..02 added in Phase 7.1 on 2026-11-25)
- Mapped to phases: 45
- Unmapped: 0

---
*Requirements defined: 2026-11-25*
*Last updated: 2026-11-25 after roadmap creation*
