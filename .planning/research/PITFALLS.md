# Domain Pitfalls

**Domain:** General-purpose Rust 3D engine with planetary-scale reference app and conventional 3D reference scene
**Researched:** 2026-11-25
**Overall confidence:** MEDIUM. This draws on the project's PROJECT.md and HLD.md (sections 5, 7, 8, 12 read directly) and on well-established engine and terrain-rendering practice. I did no live web verification, so ecosystem-specific claims (wgpu, Rust ECS crates) are from training data and should be re-checked in the phase research.

## Critical Pitfalls

### Pitfall 1: Floating-point precision handled late (jitter, z-fighting, broken picking)
**What goes wrong:** Positions are stored as f32 or f64 in world space and converted for the GPU at the end. Jitter appears at planet radii: an f32 ULP at 6.4e6 m is about 0.5 m. Near-surface cameras shake, vertices swim, and shadows and picking disagree.
**Why it happens:** The conventional scene works fine in f32, so precision is treated as a later add-on. HLD 7.3 and 7.4 list the needs but give no mandatory mechanism.
**Consequences:** Transform, scene and render-extraction APIs all assume one precision. Fixing it later is a rewrite of the core data model.
**Prevention:**
- Define in the core phase a high-precision position type (f64 or i64 plus f32 offset, or a cell-based grid) and a camera-relative render transform. Do the f64-to-f32 subtraction on the CPU, never on the GPU.
- Make render extraction emit camera-relative f32 only.
- Keep an explicit conversion API per coordinate space (HLD 7.3).
- Add a precision test that places the camera 6.4e6 m from the origin with a 1 m object. Assert stable vertex positions and no visible jitter.
- Also check depth precision: use reverse-Z with a float depth buffer and an infinite far plane. Orbit-to-surface needs this because near and far span more than 1e7:1.
- Terrain patch vertices are stored relative to a patch-local origin.
**Detection:** Jitter that grows with distance from the origin. Tests that pass only near (0,0,0). Any `Mat4` of f32 holding a global translation.
**Phase:** Core/world/transform phase (earliest). Validate in the planetary phase.

### Pitfall 2: Planetary assumptions leaking into the core (violates Core Value)
**What goes wrong:** "Just this once" additions land in core crates: a geodetic type in the transform component, `Patch` or `Face` in the streaming key, planet-specific LOD fields in the renderer, or a cube-sphere assumption in the spatial trait.
**Why it happens:** Planetary is the first and most demanding consumer, so the core gets shaped around it. Agents under time pressure take the shortest path.
**Consequences:** The conventional reference scene needs dead planetary code. The core stops being general, which is the project's stated Core Value.
**Prevention:**
- Enforce dependency direction mechanically. Use separate crates, `cargo deny` or `cargo-depgraph` checks in CI, and a test that builds the core and the conventional app with the planet crate absent.
- Build the conventional reference scene early and in parallel, not last. It is the canary.
- Make core keys generic (opaque `ResourceId`, `SpatialKey`) and let extensions supply meaning.
- Review each core trait against the question "does a non-planet user need this?"
**Detection:** The `planet`, `geo`, `patch` or `lat` identifiers appear in core crates. The conventional app fails to compile without the planet crate. A core trait has exactly one implementor and its methods look planet-shaped.
**Phase:** Workspace bootstrap (CI gate), then every phase. The conventional reference app must exist by the rendering phase.

### Pitfall 3: Abstraction before evidence (premature traits, "two-way door" over-engineering)
**What goes wrong:** The HLD lists many replaceable backends (rendering, spatial, ECS, scheduler). Agents build trait hierarchies, generic parameters and plugin-style registries for all of them before any has two implementations.
**Why it happens:** The HLD is written in "should support" language. Section 22.4 itself warns against abstractions with no current use.
**Consequences:** Generics bloat compile times, error messages become unreadable, and the wrong abstraction is locked in. The Rust borrow checker makes a bad trait boundary expensive to change.
**Prevention:**
- Rule: a trait is introduced only when there are two real implementations, or a test double plus one real one, in the same phase.
- Resolve the HLD 23 open decisions with small benchmarked spikes, and record each in an ADR before building on it.
- Prefer concrete types behind a narrow module API first. Extract the trait on the second use.
- No dynamic plugin loading or external ABI (already out of scope).
**Detection:** Traits with a single impl and no test double. More than two layers of generics on a public type. A module registration system with one module.
**Phase:** Every phase. ADR spikes in the early phases (rendering API, spatial backend).

### Pitfall 4: Render backend leakage (wgpu types in public API)
**What goes wrong:** The renderer abstraction exists but `wgpu::Buffer`, `wgpu::TextureFormat` or backend-specific limits and bind-group layouts appear in asset, scene or SDK types. "Replaceable backend" becomes false.
**Why it happens:** The GPU resource handle is the easiest thing to store on an asset. The HLD resource states (CPU/GPU residency) invite storing GPU handles inside asset structs.
**Consequences:** A second backend, headless testing, or a mock for CI is impossible without touching the core. Residency logic is entangled with the GPU API.
**Prevention:**
- Core and SDK expose only engine-owned handles (`GpuMeshHandle`, `TextureId`) and plain-data descriptors.
- Only the backend crate depends on the GPU API crate. CI check on `cargo tree`.
- Build a null or headless backend early. It forces the boundary to be real and enables CI without a GPU.
- Render extraction produces plain data (`RenderPacket`) that is independent of geometry source (HLD 8.1).
- Do not design the boundary as the lowest common denominator of several APIs. Pick one backend (wgpu is the likely Rust choice, to be confirmed by the spike) and keep the seam thin.
**Detection:** `wgpu` (or other GPU crate) in the `[dependencies]` of core, scene, asset or sdk crates. Unit tests that need a GPU.
**Phase:** Rendering framework phase. Null backend in the same phase.

### Pitfall 5: Terrain LOD cracks, popping and neighbor inconsistencies
**What goes wrong:** Adjacent patches at different LOD levels leave T-junction cracks. Cube face edges and corners are special cases: the 4-neighbor lookup differs across faces and the face orientation (u/v axes) changes. Morphing is computed in f32 at the wrong origin. Popping appears when children replace parents.
**Why it happens:** Neighbor resolution across 6 face boundaries is fiddly. Treating faces as independent quadtrees works in the interior and fails at seams. Distance-only LOD is explicitly forbidden by the HLD (12.6), but a simple distance metric is the easy first version.
**Consequences:** Visible holes at face seams and at LOD borders. Late discovery, because test cameras tend to sit on a face interior.
**Prevention:**
- Define the face-edge adjacency table and orientation transforms first, with exhaustive unit tests: every patch at every level has 4 neighbors that are reciprocal.
- Enforce a 2:1 LOD balance (restricted quadtree) so stitching has only a finite number of cases.
- Choose one crack strategy (skirts, edge stitching or morphing with a shared vertex rule) in a prototype spike and record it in an ADR. Skirts are the cheapest and are a reasonable first step.
- Use the same deterministic height function for shared edge vertices. Sampling at the same patch-independent world position guarantees a match.
- Add test cameras on face edges and corners and at the poles, plus a screenshot or image-diff regression test.
- Implement screen-space error from the start: `error_world * viewport_height / (2 * distance * tan(fov/2))`. Add hysteresis to avoid LOD flicker.
**Detection:** Holes only at certain camera positions. Neighbor-lookup tests missing for cross-face cases. LOD that changes every frame at a threshold.
**Phase:** Planetary terrain LOD phase. Topology and neighbor tests belong in the earlier planetary-topology phase.

### Pitfall 6: Streaming that blocks, thrashes, or leaks
**What goes wrong:** The render loop waits on generation (explicit violation of 12.8). Or there is no cancellation, so stale requests for patches the camera has left still consume workers. Or GPU uploads happen on the render thread in unbounded batches and cause frame spikes. Or memory budgets are advisory and residency grows without bound.
**Why it happens:** The simplest implementation generates inline. Async is bolted on as a thread pool without priority, and priorities are computed once at request time and go stale as the camera moves.
**Consequences:** Hitches on fast descent, which is the planetary demo's main scenario. Out-of-memory on long sessions. Race conditions where a patch is released while an upload is in flight.
**Prevention:**
- Design a request lifecycle state machine (Requested, Generating, Ready-CPU, Uploading, Resident, Evictable) with generation-tagged handles so stale results are discarded safely.
- Re-prioritize each frame, or make priority a function evaluated at dequeue time. Support cancellation tokens.
- Cap the upload bytes and the count per frame.
- Keep the parent resident as a fallback until all needed children are ready (12.8 item 4), and pin it against eviction.
- Enforce hard budgets with LRU eviction that never evicts a resident fallback.
- Add a frame-time test with a scripted fast orbit-to-surface descent that reports the max frame time and fails on a threshold.
- Do not hold locks or wait on a join handle in the frame path. Use channels with `try_recv`.
**Detection:** Frame-time spikes correlated with new patches. Growing memory on a camera-orbit soak test. Worker queues that never drain.
**Phase:** Job scheduling and streaming phase, then integrated in the terrain LOD phase.

### Pitfall 7: Non-deterministic procedural generation
**What goes wrong:** Patches generated twice differ: seed derived from execution order, thread-local RNG, f32 vs f64 differences, parallel float reductions, hash maps with randomized iteration order, platform-dependent `sin` or `pow`. Neighbor edges then disagree and caches and persistence break.
**Why it happens:** Determinism is a stated requirement (HLD 11.3) but easy to violate incidentally. Rust `HashMap` uses a random hasher by default.
**Consequences:** Cracks along patch borders, cache keys that no longer match content, and non-reproducible bug reports.
**Prevention:**
- Seed each layer from `hash(world_seed, layer_id, stable_patch_id)` using a fixed, versioned hasher. Do not use `DefaultHasher`.
- Make noise a pure function of position (sampled in a patch-independent coordinate), never of generation order.
- Avoid libm differences. Pin the math library or use pure-Rust deterministic functions in the generation path, and record the platform caveat in an ADR.
- Golden-hash tests: generate patch X and compare the hash of the output across runs, thread counts and (in CI) OS targets.
- Version the generator so cached outputs are invalidated when it changes.
**Detection:** The golden hash differs between 1 and N threads. Differences between Windows and Linux CI.
**Phase:** Procedural generation framework phase (the golden tests), then terrain.

### Pitfall 8: Premature or hidden ECS and scene-model commitment
**What goes wrong:** The project says "no premature ECS commitment" but the scene or entity API ends up exposing the internals of Bevy ECS, hecs or similar, or hand-rolls a half ECS. Alternatively, entity identity is just an array index that is reused, so stale references appear.
**Why it happens:** Entities, components and transforms need some storage, and picking a crate is the quickest route.
**Consequences:** Either lock-in contrary to the stated constraint, or a custom ECS that costs months.
**Prevention:**
- Keep the scene API in terms of engine-owned `EntityId` (generational) and a small component interface. Hide the storage crate behind it.
- Use a simple existing storage for P0 and record the choice in an ADR with a documented exit.
- Stable identity (HLD 6.5) is a separate persistent ID from the runtime handle.
- Do not write a custom ECS.
**Detection:** The ECS crate's types appear in the public SDK. Systems scheduling that is entangled with engine lifecycle.
**Phase:** World/scene phase. ADR decision in the spike.

## Moderate Pitfalls

### Pitfall 9: Scope creep from the 2,257-line HLD
**What goes wrong:** The HLD covers atmosphere, day-night, persistence, physics, scripting and plugins. Agents implement stubs "to be ready", and Phase 1 grows without bound.
**Prevention:** Treat HLD sections 13-17 as non-goals for P0 (PROJECT.md already lists them). Each phase's plans cite specific requirements. Reject anything not traced to an Active requirement. A hard exit criterion per phase: both reference apps still run.
**Warning signs:** Empty crates named `physics`, `scripting`, `weather`. Plans without a requirement ID.
**Phase:** Roadmap-level, all phases.

### Pitfall 10: Reference apps built last (integration-late failure)
**What goes wrong:** Subsystems are developed against unit tests and integrated at the end. The two reference apps then expose that the SDK is awkward or that the pieces do not fit.
**Prevention:** A thin vertical slice first: window, clear screen, triangle through the extraction path and the SDK. Grow both apps every phase. Keep the SDK usable from the apps only (no private back doors).
**Phase:** The first phases (slice), continuous.

### Pitfall 11: Wrong thread-ownership model (Rust-specific)
**What goes wrong:** Shared mutable world state wrapped in `Arc<Mutex<_>>` everywhere. Lock contention between generation, streaming and render threads. Deadlocks. Or `Rc`/non-`Send` GPU objects used from worker threads.
**Prevention:** Message passing between subsystems and immutable snapshots for render extraction. Workers produce plain data and only the render thread touches GPU objects. Decide the frame-ownership model (extract, then render, with double buffering) in ADRs. Test with `loom` or at least with thread sanitizer runs for the few shared structures.
**Phase:** Core lifecycle and scheduling phases.

### Pitfall 12: Mixed-up units, axes and handedness
**What goes wrong:** Handedness, up-axis (Y-up vs Z-up), depth range (0..1 vs -1..1), winding, and cube face UV orientation differ among the math crate, the GPU API and the planet code. Units mix meters and kilometers.
**Prevention:** Fix conventions in one ADR (units: meters, f64 for world; axes; clip-space convention) and use newtypes or a dedicated math module. Visual test scene with axis gizmo and an asymmetric object.
**Phase:** Core math/transform phase.

### Pitfall 13: Performance claims without measurement
**What goes wrong:** Optimizations are applied by guess (HLD 3.3 says to avoid premature optimization). Or the opposite: no benchmarks, so regressions go unnoticed until the planetary demo.
**Prevention:** Set up `criterion` benches and a frame-time capture harness in the diagnostics phase. Define the budgets (frame time, memory, patch generation time) before the LOD work. Profile with Tracy or puffin hooks from the start.
**Phase:** Diagnostics phase (early), then each performance-sensitive phase.

### Pitfall 14: Tests that need a GPU, window or network
**What goes wrong:** CI cannot run the rendering tests, so they are skipped or flaky, and regressions pass through.
**Prevention:** Split tests: pure-CPU logic (topology, LOD selection, generation, scheduling) is fully unit-tested. The headless/null backend covers extraction. Image-diff regression tests on a software adapter (for example a Vulkan software rasterizer or WARP) with tolerances. Mark these separate in CI.
**Phase:** Testing/diagnostics, with the null backend.

### Pitfall 15: Patch ID design mistakes
**What goes wrong:** Patch IDs encode neighbors or children (HLD 12.4 lists them) rather than deriving them, so IDs become large, mutable or inconsistent. Or the ID is not bit-packed and compact, which makes hashing slow. Or the level range is too small for real planets.
**Prevention:** Make the ID a pure value `(planet, face, level, x, y)` packed in a u64 or u128, and derive parent, children and neighbors by function. Check the bit budget against the finest level needed (an Earth-size planet at 1 m resolution needs about 25 levels). Property-test round trips.
**Phase:** Planetary topology phase.

## Minor Pitfalls

### Pitfall 16: Workspace and compile-time bloat
**What goes wrong:** Too many tiny crates, or heavy generic code, makes iteration slow. Version drift between crates.
**Prevention:** Use `[workspace.dependencies]` and a shared lint set. Coarse crate granularity at first (core, render, spatial, assets, sdk, planet, apps). Split only for a dependency-direction reason.

### Pitfall 17: Undocumented decisions evaporate between agents
**What goes wrong:** Multi-agent, parallel execution loses context. Two plans make conflicting choices (for example different coordinate conventions).
**Prevention:** ADRs written in the same plan that makes the decision. Shared conventions doc read by every plan. Parallelize only plans with disjoint crates.

### Pitfall 18: Public API instability signaling
**What goes wrong:** Everything is `pub`, so internals become the de facto API.
**Prevention:** Use `pub(crate)` by default and make the SDK crate the single re-export surface. Use `#[non_exhaustive]` on config and enum types.

### Pitfall 19: Platform quirks on Windows development target
**What goes wrong:** The dev machine is Windows. GPU driver or backend selection differences (DX12 vs Vulkan) hide bugs that appear elsewhere.
**Prevention:** Run CI on at least Windows and Linux, and try more than one backend in the rendering spike.

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Workspace bootstrap | Core leakage (2), premature traits (3), bloat (16) | Dependency-direction CI check, coarse crates, shared conventions ADR |
| Core, lifecycle, math | Precision (1), conventions (12), threading model (11) | Precision type and camera-relative contract up front, conventions ADR |
| World/scene/entities | ECS lock-in (8) | Engine-owned EntityId, storage hidden, ADR |
| Rendering framework | Backend leakage (4), tests need GPU (14) | Null backend, plain-data extraction, backend-only GPU dependency |
| Spatial | Premature abstraction (3), precision (1) | One concrete backend first, second only if justified by the benchmark spike |
| Assets/residency | GPU handles in assets (4), eviction bugs (6) | Residency state machine, generation-tagged handles |
| Scheduling/streaming | Blocking, stale work, no budgets (6) | Cancellation, per-frame upload caps, fallback pinning, descent frame-time test |
| Procedural generation | Non-determinism (7) | Versioned hasher, position-pure noise, golden hashes |
| Planetary topology | Patch ID, face-edge adjacency (5, 15) | Reciprocity property tests across all 6 faces |
| Terrain LOD | Cracks, popping, depth precision (5, 1) | 2:1 balance, crack ADR, hysteresis, reverse-Z, seam test cameras |
| Reference apps/diagnostics | Late integration (10), unmeasured performance (13) | Vertical slice first, budgets and benchmarks defined before LOD |

## Phases Likely Needing Deeper Research

- Terrain LOD and crack strategy (high): the prototype spike must compare skirts, stitching and morphing.
- Streaming and residency (high): scheduler design and upload throttling.
- Rendering API choice and the render graph (medium): wgpu limits, such as bindless and compute availability, should be verified against current docs.
- Spatial backend (medium): benchmark-driven.
- Large-world precision (medium): choose between f64 world, i64 grid cells, or hybrid. Verify with a prototype.
- Procedural determinism across platforms (medium).

## Sources

- C:\development\Ar-c_engine\.planning\PROJECT.md (constraints, out-of-scope)
- C:\development\Ar-c_engine\HLD.md sections 7.3-7.4, 12.3-12.9 (read directly), plus structure of the other sections
- Established practice in large-world rendering and chunked-LOD terrain (floating origin, camera-relative rendering, restricted quadtrees, skirts and geomorphing). Training-data knowledge, not re-verified online (LOW to MEDIUM confidence).
- Rust-specific points (HashMap randomized hasher, `Send`/`Sync` constraints on GPU handles) from training data, which I consider high-certainty basics but did not re-verify.
