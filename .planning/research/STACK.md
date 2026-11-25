# Technology Stack

**Project:** General-Purpose 3D Engine (Rust; planetary reference app + conventional 3D scene)
**Researched:** 2026-11-25
**Version source:** crates.io API (max_stable_version), queried 2026-11-25. Not cross-checked against Context7 or changelogs. API-level claims come from training knowledge and are marked accordingly.

## Headline Decisions

1. **wgpu 30.x behind an engine-owned render-backend trait.** It is the de facto Rust GPU layer and covers Vulkan, DX12 and Metal. Raw Vulkan (ash) is not justified in P0. HLD §8.5 and §23 call the backend a two-way door, so wgpu goes behind a narrow boundary (the `render-backend` crate). Only that crate depends on `wgpu`.
2. **winit 0.30.x for windowing**, using the `ApplicationHandler` model. Do not adopt 0.31 yet, because only betas exist.
3. **glam for f32 render math and `glam::DVec3`/`DMat4` for f64 world math.** Do not use nalgebra in the core. Use a custom `WorldPos` type (i64 sector or f64 origin plus f32 local offset) for large-world precision. Camera-relative rendering is the default.
4. **No ECS in the core API.** PROJECT.md forbids committing to one. Use a small engine-owned scene graph with generational IDs (slotmap-style). Hide storage behind an interface so `hecs` or `bevy_ecs` can be evaluated later.
5. **Jobs: rayon for CPU-parallel work, plus a custom priority and cancellable streaming scheduler built on crossbeam/flume channels.** Do not use tokio. Do not use rayon alone for streaming, because it has no priorities or cancellation.
6. **Data formats: RON for human-authored config, postcard for binary caches, glTF 2.0 for conventional-scene assets.** The planet itself is procedural and needs no asset format.
7. **Rust toolchain must be >= 1.95 (stable), edition 2024.** The local `rustc` is 1.69, which is far too old and cannot build the stack. Add `rust-toolchain.toml` in Phase 0 and treat it as a blocker.

## Recommended Stack

### Core Language / Toolchain
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| Rust stable | >=1.95 (pin in `rust-toolchain.toml`) | Language | egui-winit 0.37 declares MSRV 1.95, wgpu 30 declares 1.87, and the newest crates drive the floor. Local machine has 1.69, so it must be updated. | HIGH (crates.io rust_version) |
| Cargo workspace, edition 2024, resolver 3 | - | Crate boundaries (HLD §20.2) | Use `[workspace.dependencies]` to pin versions in one place. Use `cargo-deny` to enforce dependency direction and licenses. | HIGH |

### Rendering / GPU
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| wgpu | 30.0.2 | GPU abstraction (Vulkan/DX12/Metal, GL fallback, WebGPU) | Standard in Rust. It is the only mature cross-platform option that is actively maintained. Bevy, Rend3 and many others use it. Fits the "replaceable backend" requirement. | HIGH on choice; MEDIUM on API details (wgpu breaks its API roughly every 3 months, so pin exact versions and budget for upgrades) |
| naga (via wgpu) | 30.0.2 | Shader translation | Ships with wgpu. Write WGSL as the primary shader language. | HIGH |
| WGSL shaders | - | Shaders | Native to wgpu and has no extra toolchain. Defer `wesl` (0.6.0, pre-1.0) for imports and modules. If shader reuse becomes painful, evaluate it then. | MEDIUM |
| bytemuck | 1.25.2 | POD casting for vertex/uniform buffers | Universal for wgpu code. Use the `derive` feature. | HIGH |
| pollster | 1.0.3 | Block on wgpu async adapter/device request | Avoids pulling in an async runtime for the one-off startup calls. | HIGH |
| half | 2.7.1 | f16 for compact vertex/height data | Use when packing terrain attributes. | MEDIUM |

Planetary rendering notes (all MEDIUM, from training knowledge and the HLD, not verified against current wgpu docs):
- Use camera-relative rendering: subtract the f64 camera position on the CPU, then upload f32 offsets.
- Use a reversed-Z depth buffer with an infinite or very far plane, with `Depth32Float`.
- wgpu supports storage buffers, compute and indirect draws, which is enough for terrain patches. P0 should still stay on simple per-patch draws (ray tracing and GPU-driven rendering are out of scope).
- Terrain patches should share a few static index buffers and use skirts or stitching for crack-free LOD.

### Windowing / Input
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| winit | 0.30.13 (stable) | Window, event loop, input | Standard. wgpu creates surfaces directly from winit windows via `raw-window-handle`. 0.31 is only in beta (0.31.0-beta.4), so wait for stable. Isolate it in a `platform` crate so the migration is cheap. | HIGH (versions); MEDIUM (0.30 API: `ApplicationHandler`, `resumed` creates the window) |

Do not use `SDL2` or `glfw` bindings. They add native dependencies and do not fit "Rust-only" goals.
`gilrs` can be added later for gamepads. It is not needed in P0.

### Math / Precision
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| glam | 0.34.3 | Vec/Mat/Quat in f32 and f64 (`DVec3`, `DMat4`, `DQuat`) | Fast, small, SIMD, with f64 types in the same crate. It is the most common math crate in the wgpu and Bevy ecosystem. | HIGH (version); HIGH (f64 types exist in glam) |
| (custom) `WorldPos` / `Transform64` | - | High-precision positions (HLD §7.4) | Write this in-house. No crate gives you origin rebasing and coordinate-space semantics. Keep world-space positions in f64 (or i64 sector plus f32), and keep the render path f32 and camera-relative. | HIGH (design) |
| approx | 0.5.1 | Float comparisons in tests | Standard. | HIGH |

f64 gives about 1 mm of precision at planetary distances (Earth radius ~6.4e6 m, f64 spacing ~1e-9 m at that range). So f64 world positions are sufficient for an Earth-sized planet, and no 128-bit or fixed-point type is needed in P0. Make the position type opaque so this can be changed later (two-way door).

Avoid nalgebra (0.35.0). It is excellent for physics and geometry but heavier in compile time and API surface, and it is not the render-ecosystem default. Revisit it only if a physics integration needs it. The `glamx` crate (0.3.1) is a small bridge in this area. It is not needed.

### Scene / World / ECS
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| slotmap | 1.1.1 | Generational handles for entities, nodes, resources | Gives stable, safe IDs without committing to an ECS. Fits HLD §6.5 identity. | HIGH |
| (custom) scene graph | - | Hierarchy and transforms | Keep it small and data-oriented. | HIGH (design) |
| hecs | 0.11.2 (deferred) | Candidate ECS if the benchmarks justify it | Minimal and standalone, with no scheduler to buy into. It is the lowest-lock-in option. | MEDIUM |
| bevy_ecs | 0.20.1 (deferred) | Candidate ECS alternative | Feature-rich (schedules, change detection) but it pulls its own app/scheduling model and a fast release cadence. | MEDIUM |

Recommendation: P0 does not take an ECS dependency. Define `World` and `Component` access behind a trait or module boundary. Run an ADR-backed spike (hecs vs bevy_ecs vs custom) in a later phase, once the conventional reference scene exists to benchmark against.

### Concurrency / Streaming
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| rayon | 1.12.0 | Data-parallel CPU work (mesh and noise generation inside one job) | Standard. | HIGH |
| crossbeam-channel | 0.5.17 | Worker to main-thread result queues | Mature and fast. `flume` 0.12.0 is a fine alternative, so pick one and keep it. | HIGH |
| crossbeam-deque | 0.8.8 | Only if building a work-stealing priority pool | Optional. A simpler `BinaryHeap` behind a mutex plus a condvar is enough for P0. | MEDIUM |
| parking_lot | latest | Faster mutexes | Optional. | MEDIUM |
| (custom) `JobSystem` / `StreamScheduler` | - | Priority, cancellation tokens, budgets, coarse fallback (HLD §10) | The requirements (priority, cancel, memory budget) are not met by rayon or tokio off the shelf. Build it on std threads plus an atomic cancel flag per job. Poll results each frame. | HIGH (design) |

Do not use tokio or any async runtime for the generation and streaming path. The work is CPU-bound and needs priority and cancellation. Async I/O runtimes add complexity without benefit. If network or file I/O arrives in P2, add it then.

### Assets / Serialization
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| serde | 1.0.229 | Derive-based serialization | Universal. | HIGH |
| ron | 0.12.3 | Human-editable config (planet config, scene files, engine config) | Rust-native, supports enums and structs naturally. `toml` 1.2.3 is an acceptable alternative for plain config. | MEDIUM |
| postcard | 1.2.0 | Compact binary cache (generated patch data, asset cache) | Stable wire format, no_std friendly, and actively maintained. Prefer it to bincode (now 3.0.0 after a controversial history of the project; verify its status before using). | MEDIUM |
| uuid | 1.29.3 | Stable resource IDs where an external ID is needed | Use v4/v5. v5 (name-based) suits deterministic IDs derived from paths. | HIGH |
| gltf | 1.4.1 | glTF 2.0 loading for the conventional scene | The standard Rust glTF parser. Use it for the conventional reference app. | HIGH (version), MEDIUM (fit) |
| image | 0.25.10 | PNG/JPEG decoding for textures | Standard. Disable unused default features to cut compile time. | HIGH |
| ktx2 | 0.5.0 | Compressed GPU texture containers | Defer to a later phase. | LOW |
| meshopt | 0.7.0 | Mesh optimization | Defer. Not needed for a prototype. | LOW |

Resource IDs (HLD §9.4) should be an engine-owned `ResourceId` newtype. Use serde-derived metadata sidecars rather than a database.

### Procedural Generation
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| (custom) seeded noise | - | Deterministic layered terrain noise | HLD §11.3 requires determinism across platforms and runs, independently seeded layers. Write a small in-house OpenSimplex or Perlin on f64 with an integer hash (for example a PCG or splitmix-style hash of seed, layer ID and coordinates). This removes dependence on third-party output stability. | MEDIUM |
| rand + rand_pcg | 0.10.4 / 0.10.3 | Seeded RNG for non-noise needs | Pin the exact version if outputs are stored in golden tests, because stream stability across versions is not guaranteed. | MEDIUM |
| noise | 0.9.0 | Reference or prototype only | Stale and f64 support is limited. If used, wrap it behind your generator trait and do not rely on its output for golden data. | LOW |
| fastnoise-lite | 1.1.1 | Alternative noise | Fine for prototypes, with the same pinning caveat. | LOW |

Cross-platform determinism: avoid `f32::sin`/`powf` differences by preferring f64 and integer hashing. Libm results can differ between platforms. If bit-exact output across platforms matters, use the `libm` crate for transcendental functions. (MEDIUM, general knowledge.)

### Spatial Structures
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| (custom) cube-sphere quadtree | - | Patch hierarchy, patch IDs | Planet-specific and sits in the planetary extension crate only. | HIGH |
| rstar | 0.14.0 | R-tree for the general spatial query backend | Candidate for the pluggable general backend (HLD §7). | MEDIUM |
| kiddo | 6.3.2 | kd-tree | Alternative backend. Prototype and benchmark before choosing. | LOW |

Start with a simple uniform grid or BVH written in-house behind the spatial trait. Evaluate rstar and kiddo in a benchmark, as HLD §23 requires.

### Diagnostics / Tooling
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| tracing | 0.1.44 | Structured logging and spans | Standard. | HIGH |
| tracing-subscriber | 0.3.23 | Log output | Standard. | HIGH |
| profiling | 1.0.18 | Thin facade over Tracy/puffin/optick scopes | wgpu itself depends on it, so it integrates naturally. | HIGH |
| tracy-client | 0.19.0 | Tracy profiler backend | Best frame-level view for streaming and LOD work. Optional feature flag. | MEDIUM |
| puffin | 0.20.0 | In-process profiler | Alternative that works well with egui. | MEDIUM |
| egui + egui-wgpu + egui-winit | 0.37.0 | Debug overlay (stats, LOD toggles, planet parameters) | Immediate-mode UI is the fastest way to build diagnostics. Keep it in a `debug-ui` crate. It is not an editor, so this does not violate the P0 scope. Note it requires wgpu and winit version alignment (check egui 0.37 pins the wgpu and winit versions you use before locking). | MEDIUM (alignment unverified) |
| thiserror | 2.0.21 | Library error types | Standard. | HIGH |
| anyhow | 1.0.105 | Error handling in apps and binaries only | Do not use it in library crates. | HIGH |

### Testing / Benchmarks
| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| built-in `cargo test` plus `cargo nextest` | - | Unit and integration tests | nextest is the common faster runner. | HIGH |
| proptest | 1.12.0 | Property tests (patch ID round trips, cube-sphere mapping, determinism) | Fits the many invariants in the HLD. | HIGH |
| insta | 1.49.0 | Snapshot and golden regression tests | Good for generated data and config. | HIGH |
| criterion | 0.8.2 | Benchmarks and performance regression | Standard. `divan` 0.1.21 is lighter and also fine. Pick one. | HIGH |
| wgpu headless (software adapter, e.g. lavapipe or WARP) | - | GPU-free CI rendering tests | Render to texture and compare images. Needs CI setup. (MEDIUM, general knowledge, not verified.) | MEDIUM |

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| GPU API | wgpu 30 | ash (raw Vulkan) | Far more code and unsafe for a prototype, and Vulkan only. Consider a second backend later only if wgpu limits are hit. |
| GPU API | wgpu 30 | vulkano | Vulkan only, with a smaller community. |
| GPU API | wgpu 30 | rend3 or Bevy renderer | Rend3 is unmaintained or in low activity (LOW confidence). Bevy's renderer is tied to Bevy's ECS and app model, which breaks the "core independent" principle. |
| Whole engine | custom core | Bevy | Bevy is the obvious competitor, but this project's goal is to build an engine with its own architecture. Bevy would impose ECS, plugins and the render graph. Use it as a design reference only. |
| Windowing | winit 0.30 | winit 0.31 beta | Unstable API. Revisit when it is released. |
| Windowing | winit | SDL2 / glfw | Native C dependencies, weaker wgpu integration. |
| Math | glam | nalgebra / cgmath / ultraviolet | nalgebra is heavy. cgmath is unmaintained. ultraviolet is niche. |
| ECS | none in P0 | bevy_ecs / hecs / specs / flecs | Forbidden by PROJECT.md until proven by benchmark. specs is effectively legacy. |
| Async | threads plus channels | tokio / async-std | CPU-bound work needing priority and cancellation. |
| Binary format | postcard | bincode / rkyv | rkyv gives zero-copy but is a larger commitment and its format is stricter. Revisit for large cached data if postcard is a bottleneck. bincode has had maintenance turmoil. |
| Config | RON | JSON / YAML | RON matches Rust types best. JSON is acceptable for interchange. YAML has pitfalls. |
| Shaders | WGSL | GLSL / HLSL / Slang | Extra compilers and build steps. WGSL is native to wgpu. |
| Debug UI | egui | imgui-rs / iced | egui has first-class wgpu and winit integration. |

## Installation

```toml
# rust-toolchain.toml
[toolchain]
channel = "stable"   # must be >= 1.95; pin an exact version once chosen
```

```toml
# Workspace Cargo.toml, [workspace.dependencies]
wgpu = "30.0.2"
winit = "0.30.13"
glam = { version = "0.34.3", features = ["serde", "bytemuck"] }
bytemuck = { version = "1.25", features = ["derive"] }
pollster = "1.0"
serde = { version = "1", features = ["derive"] }
ron = "0.12"
postcard = { version = "1.2", features = ["use-std"] }
slotmap = "1.1"
rayon = "1.12"
crossbeam-channel = "0.5"
tracing = "0.1"
tracing-subscriber = "0.3"
thiserror = "2"
uuid = { version = "1", features = ["v5", "serde"] }
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
gltf = "1.4"

[workspace.dev-dependencies]  # per-crate dev-deps
# proptest = "1.12"; insta = "1.49"; criterion = "0.8"; approx = "0.5"
```

```bash
# Tools
cargo install cargo-nextest cargo-deny
```

Crate feature names (`serde`, `bytemuck`, `use-std`) are from training knowledge and should be checked at the time of adding.

## Suggested Crate Layout (stack-relevant)

- `engine-core` depends on glam, slotmap, serde, tracing, and thiserror only. It does not depend on wgpu or winit.
- `engine-platform` depends on winit.
- `engine-render-backend` is the only crate that depends on wgpu.
- `engine-render` holds the render graph and extraction, and has no GPU API types.
- `engine-jobs` and `engine-assets` hold the scheduler and resource lifecycle.
- `ext-planet` depends on core, jobs and spatial, and nothing depends on it.
- `debug-ui` depends on egui.
- `examples/scene3d` and `examples/planet` are the two reference apps.

## Open Items Needing Verification

- Whether egui 0.37 and wgpu 30 versions align (egui-wgpu normally pins a wgpu major, so check before the debug UI phase).
- Current maintenance status of bincode 3.0, rand 0.10 stream stability, and noise crate suitability.
- winit 0.31 stable release timing.
- wgpu 30 API specifics (surface configuration, `Limits`, features for indirect and multi-draw). Check the docs via Context7 during the rendering phase.
- Software-adapter CI for GPU tests on Windows, Linux and macOS.

## Sources

- crates.io API, version and MSRV data, queried 2026-11-25: wgpu, winit, glam, egui, egui-winit, hecs, bevy_ecs, rayon, serde, ron, postcard, tracing, criterion, proptest, insta, gltf, image, and others listed above. (HIGH for versions.)
- HLD.md sections 5, 7, 8.5, 9, 10, 11, 20, 22, 23 (project constraints).
- Design and API claims otherwise come from training knowledge and are marked MEDIUM or LOW. No Context7 or official docs lookups were performed in this pass.
