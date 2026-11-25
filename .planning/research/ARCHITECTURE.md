# Architecture Research

**Domain:** General-purpose Rust 3D engine (planetary-scale reference app + conventional 3D reference app)
**Researched:** 2026-11-25
**Confidence:** MEDIUM. Structure and patterns are HIGH (well-established engine practice, and the HLD agrees with them). Specific library versions and the planetary LOD technique are LOW and must be settled by prototyping (HLD s23). I did no web verification in this pass. Treat crate names (wgpu, glam, rayon) as hypotheses to confirm in STACK.md.

## Verdict on the HLD proposal

The HLD s4/s5/s20 architecture is sound. Layered, core-small, extensions-depend-on-core. I recommend keeping it with these refinements:

1. **Collapse the 15 illustrative crates to about 8 at start.** HLD s20.1 itself says not to create all of them on day one. Merge `engine-resources` into `engine-assets`. Merge `engine-render-graph` into `engine-render`. Merge `engine-module` into `engine-core`. Defer `engine-simulation` (empty crates are fake boundaries).
2. **Treat `engine-streaming` as generic residency and priority machinery only.** Planetary LOD policy (which patches, which priority) lives in the extension. Streaming only knows `ResourceKey`, priority, budget class, and cancellation token.
3. **Make the "render extraction" contract the central seam.** Rendering must consume a geometry-source-agnostic `RenderScene` (draw items plus camera-relative transforms). This one boundary lets terrain, meshes, and future sources share a renderer without the core knowing about planets.
4. **Add an explicit `engine-math` / precision layer** (f64 world positions, camera-relative f32 render transforms). Precision is the cross-cutting concern that most often forces a rewrite, so it must exist before world, spatial, and render.
5. **Add a "boundary enforcement" mechanism from day one.** This means a CI check (`cargo deny` / `cargo tree` script / `cargo-hakari`-free) that fails if `engine-*` depends on any `planetary-*` crate. Core Value depends on it.
6. **Do not build an ECS.** Provide a minimal world/entity/component storage trait or simple typed-storage implementation behind the `engine-world` API (PROJECT.md out-of-scope: no premature ECS commitment). Hiding it behind accessor APIs keeps it a two-way door.

## Standard Architecture

### System Overview

```
┌───────────────────────────────────────────────────────────────────────┐
│ APPLICATIONS    examples/basic-scene   examples/planetary-world       │
├───────────────────────────────────────────────────────────────────────┤
│ SDK FACADE      engine-sdk  (curated re-exports, Engine builder, App  │
│                 trait, versioned public API)                          │
├───────────────────────────────────────────────────────────────────────┤
│ EXTENSIONS      planetary-* (topology, terrain LOD, camera, gen,      │
│ (depend on      render glue)        conventional-3d (meshes, lights,  │
│  core, never           materials, camera controllers)                 │
│  reverse)                                                             │
├───────────────────────────────────────────────────────────────────────┤
│ ENGINE SERVICES                                                       │
│ ┌──────────┐ ┌──────────┐ ┌───────────┐ ┌───────────┐ ┌────────────┐ │
│ │  world   │ │ spatial  │ │  render   │ │ streaming │ │ procedural │ │
│ │ scene,   │ │ traits + │ │ extract,  │ │ residency,│ │ generator  │ │
│ │ entity,  │ │ coord    │ │ graph,    │ │ priority, │ │ contract,  │ │
│ │ transform│ │ spaces   │ │ backend   │ │ budgets   │ │ seeds      │ │
│ └────┬─────┘ └────┬─────┘ └─────┬─────┘ └─────┬─────┘ └─────┬──────┘ │
│      │            │             │             │             │        │
│ ┌────┴────────────┴─────────────┴─────────────┴─────────────┴──────┐ │
│ │ engine-assets (IDs, metadata, CPU/GPU residency states, cache)   │ │
│ └──────────────────────────────┬───────────────────────────────────┘ │
├────────────────────────────────┴──────────────────────────────────────┤
│ FOUNDATION   engine-core (lifecycle, config, module registry, events, │
│              service/capability registry, typed IDs)                  │
│              engine-jobs (thread pool, priorities, cancellation)      │
│              engine-math (f64 positions, camera-relative, frames)     │
│              engine-diagnostics (tracing, metrics, debug overlay hooks)│
│              engine-platform (window, input, GPU device/surface)      │
└───────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| engine-core | Lifecycle state machine (Initialize, Load, Running, Suspend, Shutdown), config, module registration and ordering, event bus, capability registry | `Module` trait with `init/load/update/suspend/shutdown`; `ServiceRegistry` keyed by `TypeId`; no dependencies on other engine crates except math/diagnostics |
| engine-math | `WorldPos` (f64 or i64 cells plus f32 offset), reference frames, camera-relative transform, origin rebasing | `glam` DVec3/DMat4 for world, Mat4 for GPU; a `Frame`/`CoordinateSpace` id type |
| engine-platform | Window, input events, GPU device and surface creation, timing | `winit` plus device handle passed to render backend; keep behind a trait so headless tests work |
| engine-jobs | Priority task queue, cancellation tokens, worker pool, main-thread-only task lane | Own thin layer over `rayon` or a custom pool; priority is needed, which rayon lacks natively, so a priority queue plus worker threads is more likely |
| engine-world | Scenes, entities, components, transform hierarchy, stable identity | Typed component storage behind `World` API; generational `EntityId` plus stable `Uuid`/u64 identity; transform = local (frame, f64 pos, rot, scale) |
| engine-spatial | Traits `SpatialIndex`, `VisibilityQuery`, `RegionLookup`; coordinate-space conversions; default simple backends | Traits plus brute-force/BVH default backend; planetary implements its own backend by quadtree |
| engine-assets | `AssetId`/`ResourceKey`, metadata, dependency graph, CPU/GPU residency state machine, caches (LRU with budget) | Handle types (`Handle<T>`) over a state table; states Unloaded, Loading, CpuReady, Uploading, GpuResident, Evicting, Failed |
| engine-streaming | Request, prioritize, cancel, retain-fallback, budget accounting; generic over resource type | Request queue keyed by `ResourceKey`; priority recomputed each frame by the requester; budget classes (CPU, GPU, in-flight) |
| engine-procedural | `Generator` trait (deterministic, seeded, pure), layer composition, seed derivation from stable IDs | `fn generate(&self, key: &GenKey, ctx: &GenCtx) -> Output`; seed = hash(world_seed, layer_id, key) |
| engine-render | Render extraction (`RenderScene`), render graph, pass system, GPU resource manager, backend trait | Extract on main thread into a frame packet; backend consumes the packet; passes declare resource reads and writes |
| engine-diagnostics | `tracing` spans, frame/GPU timers, counters, debug-draw queue | `tracing`/`tracy`-style hooks; metrics registry; stats readable by tests |
| engine-sdk | Stable facade, `Engine::builder()`, `App` trait, curated re-exports | Pure re-exports plus builder; contains no logic |
| planetary-* | Cube-sphere topology, `PatchId`, quadtree, SSE LOD selection, planetary camera, layered generation, terrain mesh and render passes | Implements core traits (SpatialIndex, Generator, render extraction producer) and registers as a `Module` |
| conventional-3d | Static mesh, material, light, simple camera controller; proves core without planets | Same extension mechanism as planetary; ships as a peer, not part of core |

## Recommended Project Structure

```
Cargo.toml                    # [workspace], resolver = "2"/"3", shared lints, workspace.dependencies
crates/
├── engine-math/              # f64 world pos, frames, camera-relative, rebasing (no deps)
├── engine-diagnostics/       # tracing setup, metrics, debug-draw queue
├── engine-core/              # lifecycle, config, Module trait, registry, events, typed IDs
├── engine-jobs/              # priority scheduler, cancellation
├── engine-platform/          # window/input/device behind traits
├── engine-assets/            # ids, handles, residency state machine, cache
├── engine-world/             # scene, entity, component storage, transform hierarchy
├── engine-spatial/           # spatial traits, coordinate spaces, default backend
├── engine-streaming/         # residency requests, priorities, budgets
├── engine-procedural/        # generator contract, seeding, layers
├── engine-render/            # extraction, render graph, backend trait, wgpu backend (feature-gated module or sibling crate)
└── engine-sdk/               # facade
extensions/
├── conventional-3d/          # meshes, materials, lights, fly camera
└── planetary/
    ├── planetary-core/       # topology, PatchId, hierarchy, geodetic math
    ├── planetary-terrain/    # SSE LOD, transitions, fallback, streaming integration
    ├── planetary-generation/ # layered generators (heightmap noise, etc.)
    └── planetary-render/     # terrain render pass, atmosphere (minimal)
examples/
├── basic-scene/              # conventional reference app
└── planetary-world/          # planetary reference app
tests/                        # cross-crate integration, golden/reference tests
benchmarks/                   # criterion benches: LOD select, gen throughput, extraction
tools/                        # dep-direction checker, ADR helpers
docs/architecture/adr/        # ADRs (rendering API, ECS, LOD technique, spatial backend)
```

### Structure Rationale

- **crates/ vs extensions/:** The directory split makes the dependency rule visible and mechanically checkable (`crates/*` must never reference `extensions/*`).
- **engine-math first:** Every other crate depends on precision-correct types. Retrofitting f64 positions is the classic rewrite trigger.
- **Facade crate:** Apps depend on `engine-sdk` plus extensions. Internal crates can be reshaped without breaking the public surface (HLD s5.2, s18).
- **Backend inside engine-render:** Keep the wgpu backend as a feature or module behind the `RenderBackend` trait at first. Split into `engine-render-wgpu` only if a second backend appears (two-way door, no abstraction without a user).

## Architectural Patterns

### Pattern 1: Dependency inversion at the core boundary (traits in core, impls in extensions)

**What:** The core defines `SpatialIndex`, `Generator`, `RenderExtractor` (a producer of draw items), `Module`. Extensions implement and register them.
**When to use:** Every point where planetary behavior touches engine services.
**Trade-offs:** Slight indirection and dyn dispatch cost, but this is what keeps Core Value intact. Use static generics in hot loops (LOD selection internal to planetary), and dyn only at module and service boundaries.

```rust
// engine-core
pub trait Module: Send {
    fn name(&self) -> &'static str;
    fn init(&mut self, ctx: &mut InitCtx) -> Result<()>;      // register services/capabilities
    fn on_load(&mut self, ctx: &mut LoadCtx) -> Result<()>;
    fn update(&mut self, ctx: &mut FrameCtx);                 // per-frame; must not block
    fn shutdown(&mut self);
}
// planetary-terrain registers itself:
// ctx.services.register::<dyn RenderExtractor>(Box::new(TerrainExtractor::new(..)));
```

### Pattern 2: Extract, then render (frame packet decoupling)

**What:** Each frame, a main-thread extraction step walks world and extension producers and writes an immutable `RenderScene` (camera, visible draw items, lights) in camera-relative f32 space. The render backend consumes only that packet.
**When to use:** Always. It is the only way for terrain and meshes to share a renderer without renderer knowledge of either.
**Trade-offs:** Extra copy per frame. It enables pipelined rendering later and makes headless render-data tests trivial.

### Pattern 3: Handle plus residency state machine, never blocking

**What:** Callers hold `Handle<T>` and query `state()`. They request refinement and keep drawing the current valid fallback. Upload happens only on the render thread's upload budget slice.
**When to use:** Terrain patches, textures, meshes alike.
**Trade-offs:** Callers must handle "not ready." That is the point (HLD s12.8, s22.4: render never waits).

```rust
pub enum Residency { Unloaded, Requested, Generating, CpuReady, Uploading, GpuResident, Evicting, Failed }
// LOD: desired = select(view); effective = nearest GpuResident ancestor-or-self; request(desired) if != effective
```

### Pattern 4: Stable-ID, pure-function generation

**What:** `generate(key, seed) -> Output` is a pure function of a stable ID (e.g., `PatchId`) plus layer seed. No global RNG, no iteration-order dependence.
**When to use:** All procedural layers. Gives caching, cancellation (just drop), parallelism, and reproducibility tests for free. Neighbor seams: generate with sampling functions defined in world/planet space (continuous noise evaluated at world position), never per-patch local noise, so adjacent patches agree on borders.
**Trade-offs:** Needs care with floating-point determinism across platforms (document the guarantee level: same-platform bit-exact versus cross-platform tolerance).

### Pattern 5: Precision-by-construction (large-world)

**What:** Authoritative positions in f64 (or integer cell plus f32 offset). Per frame, subtract camera position in f64, then cast to f32 for GPU. Rebase the world origin when the camera exceeds a threshold. Terrain patch vertices are stored relative to the patch center, and the patch center is camera-relative at draw time.
**When to use:** Everything that is rendered or compared spatially.
**Trade-offs:** Discipline needed (no raw f32 world coordinates in the API). Cheap if designed in, very expensive to retrofit.

### Pattern 6: Quadtree LOD with screen-space error and parent fallback

**What:** Per frame, traverse the patch quadtree from face roots. Split when projected error (geometric error divided by distance, scaled by viewport and FOV) exceeds the threshold, and cull by frustum/horizon. If a wanted child is not resident, draw the nearest resident ancestor and queue the child. Enforce a 2:1 neighbor level constraint (or use skirts or edge morphing) for crack-free seams. Pick the technique via benchmark (ADR).
**Trade-offs:** The traversal is CPU-side and cheap at P0 scale. Hysteresis is needed to avoid split/merge thrash.

## Data Flow

### Frame Flow

```
Platform events -> Input -> App/Module update (camera, sim)
        |
        v
Planetary camera update (f64 pos, rebasing check)
        |
        v
LOD selection (SSE, quadtree) -> desired patch set + effective (resident) set
        |                              |
        | missing -> StreamRequest(PatchId, priority, budget class)
        v                              v
Extraction -> RenderScene (camera-relative f32)     Streaming scheduler -> Jobs (workers)
        |                                                   |
        v                                                   v
Render graph -> backend submit                       Generator(PatchId, seed) -> CPU mesh/data
        ^                                                   |
        |   upload budget slice per frame                   v
        +------------ GPU upload <---- CpuReady <-- assets residency table
```

### State Management

```
World/Scene (authoritative state, main thread)
    -> extraction snapshot (immutable per frame) -> render
Assets/Residency table (shared, internally synchronized; state transitions only via scheduler)
Worker results -> channel -> main/render thread applies state change (no worker touches GPU or world)
```

### Key Data Flows

1. **Stream-and-refine:** LOD says "want patch P at level L." Streaming checks the residency table. If it is absent, enqueue with a priority from screen error and distance, and keep drawing the ancestor. A worker generates CPU data and the result returns via channel. The render thread uploads within a per-frame budget. LOD picks it up next frame and transitions via morph or fade. Out-of-view requests are cancelled by token, and eviction follows budgets (LRU weighted by priority).
2. **Camera-relative render:** World f64 position minus camera f64 position gives an f32 relative transform at extraction. No f64 reaches the GPU.
3. **Module registration:** The app builder lists modules, `engine-core` orders them by declared dependencies, `init` registers services and capabilities, and `update` runs in a fixed phase order (Input, Simulate, Camera, LOD/Stream, Extract, Render).
4. **Basic scene (non-planetary):** The `conventional-3d` extension feeds the same extraction and graph. If this works without importing any `planetary-*` crate, the core is validated.

## Suggested Build Order (dependency-driven)

| # | Layer | Contents | Why here | Unblocks |
|---|-------|----------|----------|----------|
| 1 | Workspace and guardrails | Cargo workspace, lints, CI, dep-direction check, ADR template | Boundary violations are cheapest to prevent at the start | everything |
| 2 | Foundation | engine-math, diagnostics, core (lifecycle, Module, registry, events), jobs | Everything depends on these; precision types must precede world and render | 3 to 6 |
| 3 | Platform and minimal render | platform (window), render backend with a triangle, then render graph skeleton | Early visual feedback; decides rendering API via spike (ADR) | 4, 6 |
| 4 | World and spatial | scene/entity/transform with frames, spatial traits, camera-relative extraction | Needs math and core; render needs extraction contract | 5, 7 |
| 5 | Assets and residency | IDs, handles, residency state machine, cache budgets | Needed before streaming and real meshes | 6, 7 |
| 6 | Conventional reference app v0 | conventional-3d (mesh, light, fly cam), basic-scene example via SDK | Validates core without planets early. Do this BEFORE heavy planetary work | validates 2 to 5 |
| 7 | Streaming and procedural | streaming scheduler (priority, cancel, budgets), generator contract and seeding | Generic machinery; testable with synthetic generators | 8 |
| 8 | Planetary core | topology, `PatchId`, quadtree, geodetic and local frames, planetary camera | Pure math and data, heavily unit-testable and no GPU needed | 9 |
| 9 | Terrain LOD plus generation plus render | SSE selection, fallback, transitions, layered generation, terrain pass | Integration crux; needs 3, 5, 7, 8 | 10 |
| 10 | Hardening | atmosphere (minimal), diagnostics overlays, perf and regression suites, SDK polish, docs | Needs working vertical slice to measure | release |

Parallelization opportunities (matches the "parallel where independent" setting): steps 4 and 5 are independent after 2. Within 8, topology math is independent of the render work in 3. Step 7 can use synthetic generators in parallel with step 6.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| P0 prototype (one planet, about 10^3 visible patches, one window) | CPU quadtree traversal, per-patch draw calls, simple priority queue, brute-force or BVH spatial index. Do not over-engineer. |
| P1 (richer data, more content) | GPU-driven culling and indirect draws, texture and virtual-texture streaming, mesh instancing. These are optional future capabilities (not in P0). |
| Multi-planet and heavy datasets | Spatial backend swap (hierarchical grid), persistent cache on disk, job-system tuning. The traits from P0 make these swaps local. |

### Scaling Priorities

1. **First bottleneck:** Main-thread frame cost (LOD traversal, extraction, and GPU upload spikes). Fix with a per-frame upload byte budget, hysteresis, and incremental traversal.
2. **Second bottleneck:** Generation throughput and memory churn. Fix with a priority and cancellation-aware scheduler, buffer pooling, and coarse-first generation. Then caching generated patches with budget-based eviction.

## Anti-Patterns

### Anti-Pattern 1: Planet concepts leaking into core

**What people do:** Add `Planet`, `lat/lon`, or `Patch` fields to `Transform`, `World`, or the renderer "just for convenience."
**Why it's wrong:** It breaks the Core Value and makes the conventional app carry dead concepts.
**Do this instead:** Generic `Frame`/`CoordinateSpace` ids and trait seams. Planetary types live only under `extensions/planetary`. Enforce with the CI dependency check, and run the conventional example in CI without planetary crates built.

### Anti-Pattern 2: Blocking on generation or I/O in the frame path

**What people do:** `get_patch()` falls back to synchronous generation when the patch is missing.
**Why it's wrong:** It causes hitches, and it violates the hard constraint (HLD s22.4).
**Do this instead:** Return the fallback plus a request. Add a test or debug assertion that fails if a frame exceeds a threshold or if generation is called on the render thread.

### Anti-Pattern 3: f32 world coordinates

**What people do:** Store positions as `Vec3` f32 in world space and "fix jitter later."
**Why it's wrong:** At planetary scale (about 6.4e6 m) f32 gives roughly half-meter precision, so visible jitter appears, along with broken physics and picking later.
**Do this instead:** f64 authoritative, camera-relative f32 for GPU, per-patch local origins, and a test that runs at the surface with tiny motions.

### Anti-Pattern 4: Premature ECS or generic everything

**What people do:** Pick an ECS and make every crate generic over it, or build a plugin ABI up front.
**Why it's wrong:** PROJECT.md explicitly excludes it. Lock-in raises the cost of every later change.
**Do this instead:** A small `World` API with simple storage. Revisit via ADR after the reference apps expose real access patterns.

### Anti-Pattern 5: Fragmented crates with no consumer

**What people do:** Create all 15+ crates and trait layers on day one.
**Why it's wrong:** Compile friction and abstraction with no user. Boundaries guessed wrong are costly to move.
**Do this instead:** Start with about 8 to 10 crates (above), and split when a second consumer or a real separation appears.

### Anti-Pattern 6: Per-patch independent noise and no neighbor model

**What people do:** Generate each patch with local-coordinate noise and fix cracks afterwards.
**Why it's wrong:** Border discontinuities and cracks. Neighbor consistency becomes a patch-by-patch hack.
**Do this instead:** Sample all layers in a shared planet-space function. Patches share border vertices deterministically. Add a neighbor-level constraint or skirts, and test seams numerically.

### Anti-Pattern 7: Rendering API leaking through the backend boundary

**What people do:** Expose wgpu types in `engine-render` public interfaces and in extensions.
**Why it's wrong:** The "replaceable backend" promise becomes fiction.
**Do this instead:** Engine-owned handle types (`BufferId`, `TextureId`, `PipelineDesc`) in the public render API. Only the backend module touches wgpu. But do not over-abstract: model the abstraction on what the one backend and the two reference apps need.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| GPU API (wgpu is the likely candidate, to be decided by ADR) | Behind `RenderBackend` trait, device created by platform | Validate WGSL buffer and precision limits for large meshes. Per-frame upload throttling is required. |
| Windowing and input (winit likely) | Platform trait, events into the engine event bus | Handle Suspend/Resume for the lifecycle. Headless mode for CI. |
| Logging and profiling (`tracing`, optional Tracy) | Diagnostics crate wraps; spans per phase | Make frame-phase timings queryable from tests. |
| Math (glam likely) | Used inside engine-math, re-exported | Keep f64 types first-class in the public API. |
| Filesystem and cache | Behind assets trait | Only needed for caching and later data ingestion (P2). |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| core to everything | Trait and registry (services, events) | Core never imports a service crate other than math, diagnostics, and jobs. |
| world to render | Extraction into `RenderScene` snapshot | One-way. The renderer never reads the world. |
| LOD (planetary) to streaming | `StreamRequest{key, priority, budget class, cancel token}` | Streaming is generic, and the planetary side recomputes priorities every frame. |
| streaming to jobs | Task submission with priority and cancellation | Results return via channel, and only the main thread mutates the residency table's GPU states. |
| streaming/assets to render | Residency state and upload queue | Upload happens on the render thread within a byte budget. |
| procedural to planetary | Generator trait implemented by planetary-generation | Seed derived from `PatchId`, so core never sees patches. |
| spatial to planetary | `SpatialIndex` / `RegionLookup` implemented by the planetary backend | Geodetic and local-tangent frames expressed through generic `CoordinateSpace`. |
| extensions to SDK | Re-exports plus `Module` registration | Apps compose the SDK with chosen extensions. |

## Open Decisions to Resolve by ADR (from HLD s23, with a recommended default)

| Decision | Recommended default | Resolve by |
|----------|--------------------|------------|
| Rendering API | wgpu (portable, safe Rust, pairs with winit). LOW confidence, needs a spike confirming limits (buffers, timestamp queries, indirect draws). | Phase 3 spike |
| ECS | None in P0. Simple typed storage behind `World` API. | Revisit after reference app 1 |
| Spatial backend | Brute-force or BVH default for conventional. Quadtree via the planetary hierarchy for planetary. | Phase 4 and 9 |
| LOD transition | Geomorphing in the vertex shader plus neighbor-level constraint. Alternatives: skirts, stitching. | Benchmark in Phase 9 |
| Job system | Custom priority queue plus workers (rayon has no priorities or cancellation). | Phase 2 |
| Determinism level | Same-machine reproducible, tolerance-bounded across platforms. | Phase 7 |

## Sources

- `HLD.md` s4, s5, s12, s20, s22, s23 (project source document; read directly). HIGH for project intent.
- `.planning/PROJECT.md` (constraints and scope). HIGH.
- General engine-architecture practice (extract/render split, handle-and-residency resource models, camera-relative rendering, quadtree SSE terrain, floating origin). From training knowledge, MEDIUM. Not re-verified against current sources in this pass. Recommended to confirm against current docs (wgpu, Bevy render-world extraction design, cesium-native/3D Tiles SSE LOD, Outerra and "Large World Coordinates" write-ups) during phase-level research.

---
*Architecture research for: general-purpose Rust 3D engine with planetary reference implementation*
*Researched: 2026-11-25*
