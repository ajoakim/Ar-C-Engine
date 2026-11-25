# Feature Research

**Domain:** General-purpose Rust 3D engine (planetary-scale reference app + conventional 3D reference app)
**Researched:** 2026-11-25
**Confidence:** MEDIUM. Feature taxonomy comes from the HLD plus the author's knowledge of Bevy, Fyrox, Godot, Unreal, Unity and Cesium/OpenSpace-style planetary renderers. No live web verification was done this pass. Competitor statements are LOW to MEDIUM and should be checked before they drive scope.

## Framing

The audience is engine users: application developers and the two in-repo reference apps. Table stakes are what lets a developer build a conventional 3D scene. Differentiators are what the Core Value and the planetary stress test justify. The Core Value is a core that stays independent of any specialized capability. That makes boundary discipline itself the main differentiator. It also shapes the anti-features: many "expected" engine features are deliberately excluded from P0 (HLD §22.4).

## Feature Landscape

### Table Stakes (Users Expect These)

Without these, the conventional 3D reference scene cannot exist and the engine will not read as an engine.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Cargo workspace with clear crate boundaries and enforced dependency direction | Every serious Rust engine is multi-crate. The Core Value depends on it. | LOW | HLD §20. Enforce with `cargo-deny` or a CI dependency check so core never depends on planetary. |
| Engine lifecycle (Init, Load, Running, Suspend, Shutdown), config, module registration | Basic runtime structure. | MEDIUM | HLD §5.3, §17.3. Suspend matters for mobile and web later. |
| Windowing, input, and the main loop | Nothing runs without them. | LOW | Platform layer should be a replaceable boundary (winit is the Rust default). |
| Scene, entity, and transform hierarchy | The conventional scene needs parent-child transforms. | MEDIUM | Do not commit to an ECS (out of scope). Hide storage behind interfaces and decide by benchmark (§23). |
| Stable entity and resource identity | Needed for streaming, diagnostics, and later persistence. | LOW to MEDIUM | HLD §6.5, §9.4. |
| Camera system (perspective, view and projection, frustum) | Required for any 3D view. | LOW | Camera-relative transforms must be designed in from the start, not retrofitted. |
| Mesh rendering with materials and a basic PBR or lit shading model | Users expect a lit 3D scene. | MEDIUM | An unlit-only engine reads as a toy. Keep to one simple lit model in P0. |
| Textures, samplers, and mipmaps | Required for materials. | MEDIUM | |
| Depth buffering, frustum culling, and visibility | Baseline performance and correctness. | MEDIUM | Reverse-Z with a float depth buffer is effectively mandatory for planetary ranges. It is cheap to adopt early and costly later. |
| Directional light, with a basic shadow option | Lighting is expected. | MEDIUM | Shadows can be P1. At minimum, a directional light is needed for terrain shading. |
| Rendering backend abstraction (wgpu is the likely candidate) | Replaceable backend boundary is an explicit requirement. | MEDIUM | The API choice is deferred to benchmarking (§23). Keep the boundary thin. wgpu is the de facto Rust choice, so verify the decision in an ADR. |
| Render graph or pass framework with render-data extraction | Explicit requirement. Decouples scene from GPU submission. | HIGH | HLD §8. Extraction must be geometry-source-agnostic. This is what lets terrain plug in. |
| Asset loading (glTF mesh and texture import) | Users need to bring content into the conventional scene. | MEDIUM | glTF 2.0 is the only format to support initially. A full importer pipeline is P1 ("Asset importing foundation"). |
| Resource lifecycle (CPU and GPU residency states) and caching | Explicit requirement and prerequisite for streaming. | MEDIUM to HIGH | HLD §9. |
| Job system and async task execution | Needed for non-blocking generation and loading. | MEDIUM | Priority and cancellation are required. They are not optional because the streaming requirements depend on them. |
| Logging, tracing, and basic diagnostics (frame time, memory, counters) | Debugging an engine is impossible without it. | LOW to MEDIUM | Use the `tracing` crate. Add a debug overlay and optional Tracy-style profiler hooks. |
| Test harness (unit, integration, reference, performance, regression) | Explicit requirement. | MEDIUM | Headless rendering or a golden-image path is hard but valuable. |
| Public SDK crate with documented, versioned API surface | Explicit requirement. Reference apps must use only the SDK. | MEDIUM | Reference apps are the SDK's acceptance test. |
| ADRs and documentation | Explicit requirement. | LOW | |
| Cross-platform build (Windows, Linux, macOS) | Rust users expect it. | LOW to MEDIUM | Free if wgpu and winit are used. Web and mobile are out of P0. |

### Differentiators (Competitive Advantage)

These align with the Core Value and the planetary stress test. Most Rust engines (Bevy, Fyrox) do not ship them.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Large-world precision as a core capability (multiple coordinate spaces, f64 world positions, camera-relative rendering, origin rebasing) | Most engines break at f32 limits (jitter) beyond tens of kilometers. This is the core enabler for planetary scale. | HIGH | HLD §7.3, §7.4. Generic in core: coordinate-space types and a high-precision position type. It must not mention planets. |
| Pluggable spatial query, visibility, and region-lookup interface | Lets the same engine use a flat grid, an octree, or a planetary patch tree. | MEDIUM to HIGH | HLD §7. Backends are chosen by benchmark. |
| Streaming framework with priority, cancellation, coarse fallback, and memory budgets | Most engines treat streaming as app-level. Here it is a first-class core service. | HIGH | HLD §10. Progressive availability (coarse then fine) is the user-visible feature. |
| General procedural generation framework (deterministic, independently seeded layers) | Reusable for terrain, vegetation, and anything else. Determinism makes tests and regression feasible. | MEDIUM to HIGH | HLD §11. Seed per layer, so adding a layer does not perturb the others. Pin the RNG and float behavior. Cross-platform float determinism is a pitfall. |
| Planetary extension: cube-sphere topology, stable patch IDs, quadtree hierarchy | The reference differentiator. Stable patch IDs enable caching, diagnostics, and later persistence. | HIGH | HLD §12. Lives only in the extension crate. |
| Screen-space-error LOD with crack-free transitions and parent-child fallback | Visually correct, scalable terrain. | HIGH | Techniques: skirts, stitching, geomorphing, or CDLOD-style morphing. The choice is open (§23). Compare by prototype. |
| Planetary camera (orbit to surface navigation) | Showcases the engine, since orbit to ground exercises every large-world feature at once. | MEDIUM to HIGH | Needs a smooth speed and altitude relationship, and a near/far plane strategy. |
| Memory and streaming budget observability, with LOD and patch diagnostics | Few engines expose this. It makes the engine tunable and testable. | MEDIUM | HLD §12.14 requires it. Keep generic in core (budget counters) and specific in the extension (patch state). |
| Capability-optional architecture (core runs with no planetary, physics, or scripting modules) | A verifiable architectural property, not a UI feature. Differentiates from monoliths. | MEDIUM | Prove it in CI: build and run the conventional reference app with the planetary crate absent. |
| Basic atmosphere (planetary extension) | Without it, a planet from orbit looks unconvincing. | MEDIUM to HIGH | HLD §13.2. Keep it in the extension. Start with a simple analytic scattering approximation. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Editor and visual development environment | Every engine "needs" one. | Large effort. It would pull the SDK toward UI needs before the runtime is stable. | Defer to P2.5+. Provide debug overlays and config files. |
| Committing to a specific ECS now | ECS is the Rust engine fashion. | Locks storage and scheduling before benchmarks. Violates two-way-door decisions. | Entity and component interfaces, plus an ADR after benchmarking. |
| Physics integration (Rapier etc.) | Games need collision. | Pulls physics assumptions into core. It is an optional capability. | Future optional module behind a trait boundary. |
| Scripting VM | Designers want it. | Premature, with a large API-stability burden. | Rust-only SDK. Evaluate scripting later. |
| Dynamic plugin loading and external ABI | "Extensibility." | Rust has no stable ABI. Heavy cost with no current requirement. | Compile-time Rust crates as modules. |
| Ray tracing and GPU-driven rendering | Impressive. | Not needed to validate architecture. High complexity. | Keep the render graph open to it, but do not build it. |
| Full geospatial data ingestion (DEM, imagery) | Real Earth is compelling. | A large project in itself (projections, tiling, licensing). | Procedural terrain in P0. Define a data-source interface only. |
| Persistent world editing, weather, day-night cycle, seasons, clouds | Appealing for the demo. | Each is a subsystem that needs the foundation first. | Keep time and environment hooks open (HLD §14.2) and defer implementation. |
| Comprehensive animation and skeletal system | Standard for games. | Large. Not needed to validate P0. | Transform animation only if the demo needs it. Defer skeletal. |
| Generic "everything in the core" convenience (planet types or terrain in core) | Convenient to build. | Directly violates the Core Value. | Extension crates only. Enforce via the dependency check. |
| Premature abstraction layers (traits with one implementation and no use) | Looks flexible. | HLD §22.4 simplicity rule. Adds cost and obscures behavior. | Abstract only at boundaries the HLD names (backend, spatial, resources, generator). |
| Post-processing stack (bloom, SSAO, TAA, etc.) | Visual polish. | Not needed for the architecture proof. | Tonemapping only, if needed. |

## Feature Dependencies

```
Workspace + dependency rules
    └──requires──> Engine lifecycle + module registration
                       └──requires──> Public SDK
                                          └──requires──> Both reference apps

Windowing/input ──requires──> Main loop ──requires──> Rendering backend boundary
                                                           └──requires──> Render graph + extraction
                                                                              └──requires──> Mesh/material/lighting
                                                                                                 └──requires──> Conventional ref app

Scene/entity/identity ──requires──> Transform + coordinate spaces
    └──requires──> High-precision position + camera-relative + origin rebasing
                       └──requires──> Planetary camera, large-world terrain

Job system (priority, cancel) ──requires──> Resource lifecycle (CPU/GPU residency)
    └──requires──> Streaming + budgets ──requires──> Async terrain generation
                                                         └──requires──> Coarse fallback

Procedural framework (deterministic layers) ──requires──> Planetary terrain generation
Cube-sphere topology ──requires──> Stable patch IDs ──requires──> Quadtree ──requires──> SSE LOD
SSE LOD ──requires──> Crack-free transitions + parent-child fallback
Spatial interface ──enhances──> Visibility/culling and patch selection
Diagnostics ──enhances──> Everything (build early)
Atmosphere ──enhances──> Planetary visual result (needs lighting)

Planetary extension ──conflicts──> Core assumptions (planetary types must never enter core)
ECS commitment ──conflicts──> Two-way-door principle (defer)
```

### Dependency Notes

- **Camera-relative rendering requires high-precision positions:** Transforms must carry f64 world positions from the first scene implementation. Retrofitting is a rewrite.
- **Render extraction must not know the geometry source:** Both meshes and terrain patches feed it. This is the seam that proves the Core Value.
- **Streaming requires the job system and residency states:** Cancellation and priority belong in the job layer, not bolted onto streaming.
- **Crack-free LOD requires patch adjacency:** Adjacency tests and neighbor-level rules come with the quadtree and cube-sphere seams. Cube-face edges are the hard case.
- **Diagnostics should arrive early:** LOD, budgets, and patch state are needed to debug everything above.
- **Terrain generation needs determinism:** This is a precondition for regression tests, so pin RNG and layer seeding before building the terrain.

## MVP Definition

### Launch With (v1 = HLD P0)

- [ ] Workspace, lifecycle, module registration, and SDK foundation, because everything else depends on them.
- [ ] Window, input, wgpu-class backend boundary, and render graph with extraction, to get pixels on screen through the replaceable seam.
- [ ] Scene, transform hierarchy, and coordinate spaces with f64 positions, camera-relative rendering, and origin rebasing.
- [ ] Mesh, material, and directional light, with glTF load of one model, for the conventional reference app.
- [ ] Resource lifecycle, job system with priority and cancellation, and basic streaming with budgets.
- [ ] Spatial query interface with one simple backend.
- [ ] Deterministic procedural framework.
- [ ] Planetary extension: cube-sphere, patch IDs, quadtree, SSE LOD, crack-free transitions, parent-child fallback, async generation, planetary camera, and basic atmosphere or sky.
- [ ] Diagnostics (LOD overlay, budgets, frame timing), tests (transforms, topology, determinism, adjacency), benchmarks, and ADRs.
- [ ] Conventional app runs with the planetary crate absent.

### Add After Validation (v1.x = HLD P1)

- [ ] Render graph refinement and shadow maps, once the first backend works.
- [ ] CPU/GPU residency management and caching improvements, once the budget data shows pressure.
- [ ] Better LOD transitions (geomorphing), once cracks and popping are measured.
- [ ] Asset importing foundation, after glTF-only loading shows its limits.

### Future Consideration (v2+)

- [ ] Geospatial data ingestion and persistence or world modification overlays (P2).
- [ ] Day-night, weather, and clouds.
- [ ] Physics, animation, and scripting as optional modules.
- [ ] Editor (P2.5+).
- [ ] Ray tracing and GPU-driven rendering.
- [ ] Plugin ABI and marketplace.

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Workspace, lifecycle, SDK | HIGH | LOW to MEDIUM | P1 |
| Backend boundary + render graph + extraction | HIGH | HIGH | P1 |
| Scene, transforms, coordinate spaces, large-world precision | HIGH | HIGH | P1 |
| Mesh, material, light, glTF load | HIGH | MEDIUM | P1 |
| Job system, resources, streaming, budgets | HIGH | HIGH | P1 |
| Procedural framework (deterministic) | HIGH | MEDIUM | P1 |
| Planetary topology, quadtree, SSE LOD, crack-free | HIGH | HIGH | P1 |
| Planetary camera | HIGH | MEDIUM | P1 |
| Diagnostics and tests | HIGH | MEDIUM | P1 |
| Spatial interface (one backend) | MEDIUM | MEDIUM | P1 |
| Atmosphere (basic) | MEDIUM | MEDIUM | P2 |
| Shadow maps | MEDIUM | MEDIUM | P2 |
| Geomorphing and advanced transitions | MEDIUM | MEDIUM | P2 |
| Asset import pipeline | MEDIUM | HIGH | P2 |
| Geospatial ingestion | MEDIUM | HIGH | P3 |
| Editor, physics, scripting, ray tracing | LOW (now) | HIGH | P3 |

**Priority key:**
- P1: Must have for launch (HLD P0)
- P2: Should have, add when possible
- P3: Future consideration

## Competitor Feature Analysis

Confidence LOW to MEDIUM. Based on prior knowledge, not re-verified this pass.

| Feature | Bevy / Fyrox (Rust) | Unreal / Godot / Unity | Cesium / OpenSpace / Outerra (planetary) | Our Approach |
|---------|---------------------|------------------------|------------------------------------------|--------------|
| ECS or scene model | Bevy ECS-centric. Fyrox scene graph. | Scene graph or component model | Application specific | Interface-first, no commitment, decide by benchmark |
| Large-world precision | Typically f32, with community crates | Unreal LWC (f64). Others are limited | Native f64 and camera-relative | Core capability, planet-agnostic |
| Terrain and planet LOD | Community crates, not core | Terrain tools for bounded worlds | Core strength, but not general-purpose | Extension crate on a general core |
| Streaming | App-level asset loading | Level streaming, world partition | Tile streaming | First-class core service with budgets and fallback |
| Editor | Bevy none (in progress). Fyrox has one. | Full editors | Limited | Deferred |
| Rendering backend | wgpu | Native APIs | Varies | Replaceable boundary, API chosen by ADR |
| Scripting, physics, animation | Present or via crates | Built in | Limited | Deferred, optional modules |

## Sources

- C:\development\Ar-c_engine\HLD.md (v2.0): §5-§12, §20, §21 (P0 deliverables and success criteria), §22.4
- C:\development\Ar-c_engine\.planning\PROJECT.md (requirements, constraints, out of scope)
- Prior knowledge of Bevy, Fyrox, Godot, Unreal, Unity, Cesium, and OpenSpace (LOW to MEDIUM, not re-verified)

---
*Feature research for: general-purpose Rust 3D engine*
*Researched: 2026-11-25*
