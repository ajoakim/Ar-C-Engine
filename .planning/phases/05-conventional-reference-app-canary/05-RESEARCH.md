# Phase 5: Conventional Reference App (Canary) - Research

**Researched:** 2026-11-25
**Domain:** Rust game-engine rendering (wgpu 30 behind a trait), glTF import, SDK-only app, architecture-canary CI
**Confidence:** HIGH on stack/versions/APIs (verified in a scratch crate, cargo-deny and the wgpu 30.0.2 source); MEDIUM on exact internal API shapes (design recommendations the planner may adjust)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Rendering features
- Simple lit shading: Lambert diffuse + ambient with one directional light; WGSL consumes a per-frame light uniform and per-draw material. PBR deferred.
- `Material` = base color factor + optional base-color texture, referenced by engine-owned `MaterialId`/`TextureId` handles in `RenderScene`.
- One standard vertex layout: position + normal + uv (+ optional vertex color).

#### glTF loading
- `gltf` 1.4.1 + `image` 0.25 in the extension layer; glTF converted to engine mesh/material/texture data and registered via the Phase 4 `AssetRegistry`, CPU decode in jobs.
- Scope: static meshes, base-color textures, node transforms only; no skins/animation/PBR extensions.
- Tiny checked-in glTF/glb test asset (textured cube) plus a procedural fallback for headless tests.

#### App & camera
- `extensions/conventional-3d` crate: scene builder, fly-camera controller, and a `basic_scene` example depending only on `engine-sdk` + the extension.
- Fly camera: WASD + mouse look + shift boost from SDK `InputEvent`s, f64 `WorldPos` camera.

#### Canary / CI
- Planetary-absent proof: build/test the conventional app with `planetary-core` excluded (CI step with `--exclude planetary-core` or equivalent), plus an xtask assertion that no dependency path from the app reaches planetary crates.
- Headless tests (scene extraction, full frame via NullBackend) run in CI; real window run is a manual item; software-adapter CI deferred to Phase 8.

### Claude's Discretion
Names, WGSL details, uniform layout, test structure, how the SDK exposes materials/lights.

### Deferred Ideas (OUT OF SCOPE)
PBR, shadow maps, animation/skins, asset import pipeline (v2); software-adapter image-diff CI (Phase 8).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REND-04 | Lit meshes with materials and a directional light using reverse-Z depth | New `PosNormalUv` layout + `LitTextured` shader; per-frame light uniform, per-draw material; reuses existing reverse-Z (clear 0.0, `Greater`) depth state (Standard Stack, Patterns 1-3) |
| REND-05 | Load glTF meshes and textures | `gltf` 1.4.1 + `image` 0.25.10 in `conventional-3d`; import via `gltf::import_buffers` + own bounded `image` decode (Patterns 5-6) |
| APP-01 | Conventional reference app runs against the public SDK only | SDK re-exports + new `Resources` methods; `basic_scene` example imports only `engine_sdk` + `conventional_3d` (Pattern 4) |
| PLAN-01 | Planetary code only in extension crates; conventional app builds/runs with them absent | `xtask check-canary` (transitive closure) + CI job that physically removes `extensions/planetary` and builds/tests (Pattern 7) |
</phase_requirements>

## Summary

The Phase 3 render path is deliberately minimal: `MeshHandle {vertex_buffer, vertex_count}`, one `PosColor` layout, one `UnlitVertexColor` shader, `DrawCommand {pipeline, mesh, mvp}`, one dynamic-offset uniform slot (64 B used of a 256 B stride) per draw, and `TextureId` that only names the two surface-owned targets (1 = backbuffer, 2 = depth). Phase 5 must extend this in four directions: (1) index buffers, a second vertex layout, texture creation and sampled materials in `RenderBackend`/`ResourceTable`/`NullBackend`/`WgpuBackend`; (2) per-frame lighting plus per-draw material and normal matrix flowing `RenderItem -> extract -> RenderScene -> build_frame_packet -> FramePacket`; (3) SDK surface (re-exports plus `Resources`/`SceneState` additions) so the extension and app never name wgpu/winit/engine-* crates; (4) glTF import and fly camera in `conventional-3d`, plus an architecture canary.

All third-party choices are verified. `gltf 1.4.1` and `image 0.25.10` (png+jpeg only) resolve to a small dependency tree. The full license set passes the existing `deny.toml` allow-list unchanged (every crate offers MIT and/or Apache-2.0, BSD-3-Clause, Zlib or Unlicense OR MIT; 0BSD only appears as `0BSD OR MIT OR Apache-2.0` on adler2), `bans` and `advisories` pass, so `deny.toml` needs no license edits. wgpu 30.0.2 texture/sampler/bind-group APIs were read from source. The canary has one trap: the existing `xtask check-deps` uses `cargo metadata --no-deps`, which sees only direct dependencies, so a transitive planetary leak would be invisible. A new transitive subcommand is required, and `--exclude planetary-core` alone proves nothing (it only skips building a crate that is not in the app's closure anyway). The strong proof is a CI job that deletes `extensions/planetary` and removes it from the workspace members, then builds and tests everything.

**Primary recommendation:** Extend the existing pipeline with index buffers + `PosNormalUv` + sampled textures; carry resolved material/light data in `DrawCommand`/`FramePacket`; expose everything through new SDK re-exports and `Resources` methods; keep glTF parse/decode in `conventional-3d` as pure functions plus a thin job wrapper; prove PLAN-01 with `xtask check-canary` and a CI job that removes the planetary directory.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Material/light/vertex data types (`Material`, `MaterialId`, `DirectionalLight`, `VertexPosNormalUv`, `TextureDesc`) | engine-render (tier 3) | engine-sdk re-export | Backend-agnostic data contract; must contain no GPU types |
| Texture/index-buffer creation, handle validation | engine-render `ResourceTable` + backends | engine-sdk `Resources` | Shared validation for Null and wgpu backends (existing pattern) |
| Lighting math (Lambert), sampling, depth test | engine-render-wgpu (WGSL) | — | Only crate allowed to touch wgpu/WGSL |
| Camera-relative extraction incl. normal matrix | engine-render `extract` | — | f64 world -> f32 camera-relative already lives here |
| Scene state (materials table, light, items) | engine-sdk `SceneState` | engine-render types | App-facing mutable state owned by runtime |
| glTF parse, image decode, procedural meshes | conventional-3d (tier 6) | engine-jobs via SDK | PLAN-01/WS rule: format importers are extensions; third-party crates confined here |
| Scene builder (nodes -> `World` -> `RenderItem`s) | conventional-3d | engine-world via SDK | Uses Phase 4 World hierarchy without the core knowing about glTF |
| Fly camera controller | conventional-3d | SDK `InputEvent`, `Camera`, `WorldPos` | Pure logic over SDK types; headlessly testable |
| Planetary-absent proof | xtask + CI | cargo-deny | Architectural rule enforcement already lives in xtask |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| gltf | 1.4.1 (crates.io, published 2024-05-10, newest) [VERIFIED: crates.io API + `cargo search`] | glTF 2.0 parse + buffer import | De-facto Rust glTF crate; MIT OR Apache-2.0 |
| image | 0.25.10 [VERIFIED: `cargo search image`; scratch `cargo tree`] | PNG/JPEG decode to RGBA8 | Already a transitive dep of gltf `import`; use directly for bounded decoding |
| wgpu | 30.0.2 (already pinned) | Texture/sampler/bind groups | Existing |
| bytemuck | 1.25.2 (already pinned) | Uniform/vertex byte casting | Existing |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| engine-jobs (via SDK re-export) | workspace | Run decode off the main thread | glTF import job |
| engine-assets `AssetRegistry`/`AssetId` (via SDK re-export) | workspace | Typed handles for imported CPU assets | Register imported meshes/textures/materials |
| engine-world `World` (via SDK re-export) | workspace | Node hierarchy and transform propagation | Scene builder |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `gltf::import` (eager decode, default) | `import_buffers` + own `image` decode | Own decode lets us cap dimensions/alloc and reject 16-bit/float formats; chosen |
| Index buffers | De-index in importer | De-indexing is simpler (no trait change) but bloats vertices; index support is a small, standard addition. Fallback if scope slips |
| mip generation | single mip + linear filter | Minification shimmer on distant textures; acceptable for canary, flagged as known limitation |

**Dependency declaration (root `Cargo.toml` `[workspace.dependencies]`, explicit versions per project convention):**
```toml
gltf = { version = "1.4.1", default-features = false, features = ["import", "utils", "names"] }
image = { version = "0.25.10", default-features = false, features = ["png", "jpeg"] }
```
`conventional-3d/Cargo.toml` adds `gltf.workspace = true`, `image.workspace = true`, and `bytemuck.workspace = true` only if needed. gltf's `import` feature pulls `image` with only `jpeg`+`png` and `base64`/`urlencoding` for data URIs; this exact feature set was resolved in a scratch crate (40 packages, builds clean under 1.99.0).

**Verified license/ban/advisory result:** `cargo deny check licenses bans advisories` against a scratch crate with this dependency set and the repo's `deny.toml`: licenses OK for every dependency (only the unlicensed scratch crate itself failed, irrelevant), bans OK, advisories OK. Duplicate `syn` (2.x/3.x) is a warning only (`multiple-versions = "warn"`). [VERIFIED: cargo-deny 0.22.1 run]

**Recommended extra bans in `deny.toml` (verified to work in scratch):**
```toml
{ crate = "gltf",  wrappers = ["conventional-3d"],          reason = "glTF importer confined to the conventional-3d extension" },
{ crate = "image", wrappers = ["conventional-3d", "gltf"],  reason = "image decoding confined to extensions" },
```
Without the `gltf` wrapper for `image`, cargo-deny rejects gltf's own use of image. [VERIFIED: scratch run flagged a direct parent not listed as wrapper]

## Architecture Patterns

### System Architecture Diagram

```
 disk (.gltf/.glb + png)                         keyboard / mouse (SDK InputEvent)
        |                                                    |
  [conventional-3d: import job on JobSystem]        [FlyCamera::handle_input/update]
   gltf::import_buffers + image decode (bounded)             |
        |  ImportedModel (CPU: meshes, materials, textures, nodes)
        v                                                    v
  [App::setup / update: poll job result]               scene.camera (f64 WorldPos, Quat)
        |  Resources::create_lit_mesh / create_texture / create_material
        v                                                    |
  MeshHandle, TextureId, MaterialId  ----> SceneBuilder: World entities (+hierarchy)
        |                                   world.update_transforms() -> RenderItem{mesh, material, pos, rot, scale}
        v                                                    |
  SceneState { items, materials, light, camera, frames } <---+
        |
        v   Runtime::frame
  extract(frames, camera, aspect, items, materials, light)
        -> RenderScene { view_proj, light, draws[{mesh, model, normal_mat, material}] }   (camera-relative f32)
        v
  RenderGraph.build_frame_packet(scene, pipelines, clear)
        -> FramePacket { light, passes[{ draws[{pipeline, mesh, mvp, model, normal_mat, base_color, texture}] }] }
        v
  RenderBackend::submit  -- validate_packet (shared) --+--> NullBackend (records packet; CI)
                                                       +--> WgpuBackend: group0 light UBO, group1 per-draw dyn UBO,
                                                            group2 texture+sampler, draw_indexed, Depth32Float Greater
```

### Recommended Project Structure
```
crates/engine-render/src/
  material.rs     # Material, MaterialId, MaterialTable, DirectionalLight, TextureDesc  (new)
  backend.rs      # + BufferUsage::Index, VertexLayout::PosNormalUv, ShaderId::LitTextured,
                  #   VertexPosNormalUv + encode, DrawCommand fields, FramePacket.light, create_texture
  handles.rs      # TextureId::from_raw, MeshHandle + optional index buffer/count + layout
  resources.rs    # texture table (ids start at 3), index/texture/layout validation
  scene.rs        # RenderItem.material, DrawItem material + normal matrix, RenderScene.light
crates/engine-render-wgpu/src/
  shaders/lit_textured.wgsl
  pipeline.rs     # new layout attrs, group layouts, lit pipeline, back-face cull
  backend.rs      # texture/sampler/bind-group creation, index draw, read_offscreen_rgba8()
crates/engine-sdk/src/
  lib.rs / app.rs # re-exports; Resources::{create_lit_mesh, create_texture, create_material}; SceneState.{materials, light}
extensions/conventional-3d/
  src/{lib,import(gltf),procedural,scene_builder,fly_camera,job}.rs
  assets/textured_cube.gltf (+ .png if external)   examples/basic_scene.rs   tests/*.rs
xtask/src/canary.rs  # transitive closure check, subcommand `check-canary`
```

### Pattern 1: Data contract in engine-render, no GPU types
**What:** Add to engine-render: `VertexPosNormalUv {position, normal, uv}` (`repr(C)`, 32 B), `VertexLayout::PosNormalUv` (stride 32, offsets 0/12/24), `ShaderId::LitTextured`, `BufferUsage::Index`, `TextureDesc {label, width, height}` (RGBA8 sRGB data supplied separately), `Material {base_color: [f32;4], base_color_texture: Option<TextureId>}` (`Copy`), `MaterialId`, `MaterialTable` (create/get; ids never reused, mirroring `ResourceTable`), `DirectionalLight {direction, color, intensity, ambient}`.
**Light semantics (define in docs and test):** `direction` = unit vector the light travels (from light toward scene); shader uses `max(dot(n, -direction), 0)`.
**Vertex color:** CONTEXT says "(+ optional vertex color)". Recommend omitting it in the layout for this phase (glTF `COLOR_0` is rare; multiply-by-white default) and recording that as a deliberate narrowing; the layout enum is `#[non_exhaustive]`, so a `PosNormalUvColor` can be added later without breaking callers.
**Mesh handle:** extend `MeshHandle` with `layout: VertexLayout` and `index: Option<(BufferId, u32 /*count*/)>`. Note 13 struct-literal `MeshHandle {` sites exist (tests, gpu_smoke, `Resources::create_mesh`); add a `MeshHandle::unindexed(buffer, count)` constructor and update them.
**Index format:** `u32` only (glTF may supply u8/u16/u32; `ReadIndices::into_u32()` normalises). Validation at `create_mesh`: all indices `< vertex_count`, `index_count % 3 == 0`, non-empty.

### Pattern 2: Materials, lights and textures through the pipeline (no wgpu leak)
- `RenderItem` gains `material: MaterialId` (keep `RenderItem::new(mesh, pos)` using a `MaterialId::DEFAULT`-style default so existing triangle code keeps working; PosColor draws ignore it).
- `extract(...)` gains `materials: &MaterialTable` and `light: DirectionalLight` (15 call sites in crates; update them) or take a small `ExtractInputs` struct. It resolves `MaterialId -> Material` and computes the normal matrix: `Mat3::from_mat4(model).inverse().transpose()` (reject non-finite -> `RenderError::NonFiniteTransform`; handles non-uniform scale). `RenderScene` gets `light`; `DrawItem` gets `material: Material` (resolved) and `normal: Mat3`.
- `DrawCommand` gains `model: Mat4`, `normal: Mat3`, `base_color: [f32;4]`, `texture: Option<TextureId>`; `FramePacket` gains `light: DirectionalLight` (frame-level, uploaded once). Keep `mvp` (precomputed `view_proj * model`).
- Pipeline selection: `Runtime::new` currently makes one pipeline. Create two (`UnlitVertexColor/PosColor`, `LitTextured/PosNormalUv`) and choose per draw from `mesh.layout`; change `build_frame_packet` to take a small `PipelineSet`/lookup rather than one `PipelineId`.
- `RenderBackend` gains `create_texture(&mut self, desc: &TextureDesc, rgba8: &[u8]) -> Result<TextureId, RenderError>` (+ `destroy_texture`). `create_buffer` already takes `BufferDesc.usage`; add `Index` and have wgpu map `Vertex -> VERTEX`, `Index -> INDEX`.
- `ResourceTable`: `alloc_texture` with ids starting at **3** (1 and 2 are reserved surface targets; keep `validate_packet`'s rule that pass color/depth targets must be 1/2 so a sampled texture can never be used as a pass target). Validate: draw texture exists, mesh layout matches pipeline layout, index buffer usage/size (`index_count*4 <= size`), `index_count % 3 == 0`, finiteness of new matrices/colors.
- Texture limits: reject `width|height == 0` or `> 8192` (wgpu default `max_texture_dimension_2d`), require `data.len() == w*h*4` using `checked_mul`; total size bounded by `MAX_BUFFER_BYTES`.

### Pattern 3: wgpu 30 lit backend (verified API shapes)
Bind-group plan (default limit is 4 groups):
- group 0: per-frame light UBO (FRAGMENT), written once per `submit` before encoding.
- group 1: per-draw dynamic-offset UBO (VERTEX|FRAGMENT), existing 256 B stride; payload `mvp mat4 (64) + model mat4 (64) + normal mat3x3 (48, three 16 B columns) + base_color vec4 (16) = 192 B`. Update `min_binding_size`, `MVP_BYTES` and the stride check (`min_uniform_buffer_offset_alignment <= 256`). Keep the unlit shader working (it reads only `mvp` at offset 0, so it can bind the same group if `min_binding_size` is relaxed to the 64 B prefix, or use a second layout).
- group 2: per-texture bind group {`texture_2d<f32>` FRAGMENT, `sampler` FRAGMENT}, created inside `create_texture`; backend holds a default 1x1 white texture/bind group for `texture: None`.

Verified wgpu 30.0.2 facts (read from `wgpu-30.0.2` / `wgpu-types-30.0.2` sources):
- `queue.write_texture(texture.as_image_copy(), data, wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) }, wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 })`. `Texture::as_image_copy()` exists. `write_texture` has no 256-byte row-alignment requirement (only buffer<->texture encoder copies do).
- `SamplerDescriptor<L>` fields: `address_mode_{u,v,w}`, `mag_filter`, `min_filter`, **`mipmap_filter: MipmapFilterMode`** (new type, was `FilterMode`; default Nearest), `lod_min_clamp`, `lod_max_clamp`, `compare`, `anisotropy_clamp: u16` (>=1), `border_color`. Use `..Default::default()` with Linear mag/min and Repeat address modes.
- `PipelineLayoutDescriptor.bind_group_layouts: &[Option<&BindGroupLayout>]`, `immediate_size: 0` (already used).
- `render_pass.set_index_buffer(buf.slice(..), wgpu::IndexFormat::Uint32)` and `draw_indexed(0..index_count, 0, 0..1)`; `buffer.slice(..)` on an empty buffer panics ("size_expect_nonzero"), so empty meshes must be rejected earlier (already `EmptyBuffer`/`EmptyMesh`).
- Existing depth state is correct for reverse-Z: `Depth32Float`, `depth_compare Some(Greater)`, clear 0.0 (validated by `validate_packet`); projection is `glam::camera::rh::proj::directx::perspective_infinite_reverse`. The lit pipeline reuses `depth_stencil_state()`.
- Culling: set `cull_mode: Some(wgpu::Face::Back)`; `front_face` default `Ccw` matches glTF (counter-clockwise front faces, right-handed, Y up, -Z forward as in ADR-0001). The current PosColor pipeline has `cull_mode: None` (triangle is viewed from both sides); keep that for the unlit pipeline.
- Texture format: upload base-color textures as `Rgba8UnormSrgb` (sampling returns linear). The surface prefers an sRGB format and offscreen is `Rgba8UnormSrgb`, so write linear colors in the shader. `base_color` factor is linear per glTF.
- `Depth32Float` and surface configuration are unchanged from Phase 3.
Phase 3's lesson stands: wgpu 30 differs from older knowledge. Any API not listed above must be checked against `wgpu-30.0.2/src` (present in `~/.cargo/registry/src`) or compiled in a scratch crate before it goes into a plan.

**WGSL sketch (Lambert + ambient):**
```wgsl
struct Frame { light_dir: vec4<f32>, light_color: vec4<f32>, ambient: vec4<f32> };   // xyz used, w padding
struct Draw  { mvp: mat4x4<f32>, model: mat4x4<f32>, normal: mat3x3<f32>, base_color: vec4<f32> };
@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<uniform> draw: Draw;
@group(2) @binding(0) var base_tex: texture_2d<f32>;
@group(2) @binding(1) var base_smp: sampler;
struct VsIn  { @location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) uv: vec2<f32> };
struct VsOut { @builtin(position) clip: vec4<f32>, @location(0) nrm: vec3<f32>, @location(1) uv: vec2<f32> };
@vertex fn vs_main(i: VsIn) -> VsOut { var o: VsOut; o.clip = draw.mvp * vec4(i.pos, 1.0);
  o.nrm = draw.normal * i.nrm; o.uv = i.uv; return o; }
@fragment fn fs_main(i: VsOut) -> @location(0) vec4<f32> {
  let n = normalize(i.nrm);
  let ndl = max(dot(n, -frame.light_dir.xyz), 0.0);
  let albedo = textureSample(base_tex, base_smp, i.uv) * draw.base_color;
  return vec4(albedo.rgb * (frame.ambient.xyz + frame.light_color.xyz * ndl), albedo.a); }
```
(Cross-check uniform layout with `bytemuck`-based `#[repr(C)]` structs and a size/offset unit test.)

### Pattern 4: SDK surface so the app uses only `engine-sdk` + the extension
Layer table check: `conventional-3d` (tier 6) may depend on `engine-sdk` (tier 5) only; R5 requires strictly lower tiers and R4 forbids cross-family. Depending on `engine-world`/`engine-assets` directly from the extension would be a tier-legal-by-number but contradicts the documented table ("May depend on: `engine-sdk`") and APP-01. So: **re-export through the SDK; do not add engine-* deps to the extension.** `engine-sdk` already depends on engine-world/assets/jobs/spatial, so only `lib.rs` changes:
```rust
pub use engine_world::{EntityId, Transform, World, WorldError, WorldTransform};
pub use engine_assets::{AssetError, AssetId, AssetRegistry, Handle};
pub use engine_jobs::{JobSystem, Priority};            // CancelToken if needed
pub use engine_render::{DirectionalLight, Material, MaterialId, MaterialTable, TextureId,
    VertexPosNormalUv, TextureDesc, /* DVec3 etc. already */};
pub use engine_math::{DAffine3, ...}                    // only if the importer needs them
```
`Resources` additions: `create_lit_mesh(label, &[VertexPosNormalUv], &[u32]) -> Result<MeshHandle, SdkError>` (validates indices, bounds), `create_texture(label, w, h, &[u8]) -> Result<TextureId, SdkError>`; materials need no backend resource, so `SceneState` owns `materials: MaterialTable` (+ `fn add_material`) and `light: DirectionalLight` (sane default). Keep `Resources::create_mesh` (PosColor) for the triangle example/tests.
`SceneState` currently has no `World`. Recommended: the extension's `SceneBuilder` owns a `World` and a `HashMap<EntityId, (MeshHandle, MaterialId)>`; each `update`, call `world.update_transforms()` then flush `world.world_transform(id)` (position `WorldPos` already f64, rotation `Quat`, scale `Vec3`) into `scene.items`. This needs no core `World`-in-`SceneState` change, and rendering already extracts camera-relative from `RenderItem`. Note `world_transform` is `Err(DirtyTransform)` until `update_transforms()` runs.
Also add `Runtime`/`SceneState` convenience only as needed; `run_app`, `App`, `AppControl` stay unchanged. Add `pub use` for `KeyCode::ShiftLeft` already exported via `KeyCode`.

### Pattern 5: glTF import (pure function, testable without GPU)
Location: `extensions/conventional-3d/src/import.rs`. Public API: `import_gltf_path(path) -> Result<ImportedModel, ImportError>`, `import_gltf_slice(bytes) -> ...`, and `ImportedModel { meshes: Vec<ImportedMesh {vertices: Vec<VertexPosNormalUv>, indices: Vec<u32>, material: Option<usize>}>, materials: Vec<ImportedMaterial {base_color: [f32;4], texture: Option<usize>}>, textures: Vec<ImportedTexture {width, height, rgba8}>, nodes: Vec<ImportedNode {parent, local: Transform, mesh: Option<usize>}> }`.
Verified gltf 1.4.1 API facts: `gltf::import(path) -> Result<(Document, Vec<buffer::Data>, Vec<image::Data>)>`; `gltf::import_slice(bytes)`; advanced `gltf::import_buffers(&document, base, blob)` and `gltf::import_images(...)` exist. `image::Data { pixels, format: Format, width, height }` where `Format` includes `R8, R8G8, R8G8B8, R8G8B8A8, R16..., R32...FLOAT`.
Recommended flow: `gltf::Gltf::open`/`from_slice` -> `import_buffers` -> walk default scene nodes -> `primitive.reader(|b| Some(&buffers[b.index()]))` with `read_positions()`, `read_normals()` (generate flat normals when absent), `read_tex_coords(0).map(|t| t.into_f32())` (default `[0,0]`), `read_indices().map(|i| i.into_u32())` (generate sequential indices when absent). Materials: `material.pbr_metallic_roughness().base_color_factor()` and `.base_color_texture()` (use `texture().source().index()`; honour `tex_coord() == 0` only, ignore samplers). Node transforms via `node.transform().decomposed()` -> `Transform::new(DVec3 from translation, Quat, Vec3)` (matrix form decomposes to TRS; reject shear/non-finite). Skip non-triangle primitive modes, skip primitives without positions, error on `animation/skin` ignored (documented, not failed).
Decode images yourself rather than `gltf::import_images`, using `image::ImageReader` + `image::Limits` (set `max_image_width/height <= 8192`, `max_alloc` ~ 256 MiB) and convert to RGBA8 (`to_rgba8()`); reject 16-bit/float. This is the main untrusted-input hardening point (decompression bombs). Also cap file size before reading (e.g. 256 MiB), vertex/index counts (u32 range, `MAX_BUFFER_BYTES`), node count and hierarchy depth (guard cycles/recursion; gltf's validator already rejects invalid indices but recursion over `children()` should be iterative or depth-limited).
`.gltf` external URIs: `import_buffers` with `base = Some(parent_dir)` resolves relative paths; reject `..`/absolute URIs escaping the base dir if cheap to do (path traversal), and data URIs are supported by `import` feature (base64).

### Pattern 6: Jobs and AssetRegistry
`AsyncLoader::start` takes a producer returning `Option<Vec<u8>>` and `LoadEvent::Loaded { data: Vec<u8> }`: byte-oriented, not suitable for typed `ImportedModel` without serialising. Recommended: extension-owned `ModelLoader` using `JobSystem::submit(Priority, FnOnce(&CancelToken))` (engine-jobs, re-exported by the SDK) sending `Result<ImportedModel, ImportError>` over `std::sync::mpsc`; `poll()` is called from `App::update`. `JobSystem` is not owned by `Engine` (verified: no `JobSystem` reference in engine-core/engine-sdk), so the app or `ModelLoader::new()` builds one via `JobSystem::builder().workers(n).build()`. On completion, register each imported item in `AssetRegistry<ImportedMesh/…>` with `AssetId::from_key("<normalised path>#mesh/<i>")` (`AssetId::from_key` normalises keys, fallible), keeping `Handle<T>`s for lookup; GPU upload happens on the main thread through `Resources` (resources are available in `setup` only today: **`Resources` is only passed to `App::setup`**). Gap/decision: async upload after setup needs `Resources` in `update`. Simplest correct approach: in the example, `setup` blocks on the job (`JobHandle::wait()` / channel `recv`) before creating GPU resources; the async path (poll in `update`) is possible only if `App::update` is given `&mut Resources` (a small SDK signature change — avoid unless needed; if made, it breaks `triangle.rs` and the headless tests). Recommend blocking in `setup`, with decode still executed on a job (satisfies "CPU decode in jobs") and the import API remaining non-blocking-capable for Phase 6.

### Pattern 7: Fly camera
`FlyCamera { yaw: f32, pitch: f32, speed: f64, boost: f64, sensitivity: f32, keys: held-key bitset, last_cursor: Option<(f64,f64)>, looking: bool }` in conventional-3d with `handle_input(&InputEvent)` and `update(dt, &mut Camera)`. Facts from `engine-platform/src/input.rs`: events are `KeyPressed{key, repeat}`, `KeyReleased{key}`, `MouseButtonPressed/Released(MouseButton)`, `CursorMoved{x,y}` (absolute pixels, **no raw mouse delta, no cursor grab API**), `MouseWheel`, `Focused(bool)`, `Resized`. `KeyCode` has `W A S D Q E Up Down Left Right Space Escape Enter ShiftLeft ControlLeft Other` (`#[non_exhaustive]`; use `_ =>` arm). Therefore: mouse look = cursor delta while the right mouse button is held; reset `last_cursor` on press/`Focused(false)`; clamp pitch (+-89 deg); orientation `Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch)` (-Z forward, Y up); move along `orientation * Vec3::NEG_Z` etc. in f64 (`DVec3`, `WorldPos::translated(DVec3) -> Result`); Shift multiplies speed; Q/E or Space/Ctrl for vertical (Ctrl is `ControlLeft`). Clear held keys on `Focused(false)` (prevents stuck keys). Note the Phase 3 `triangle.rs` sets `velocity` on press and zeroes on release; the same pattern works but a held-key set is more robust for chords.

### Pattern 8: Canary (PLAN-01)
Three layers, strongest first:
1. **`cargo xtask check-canary [--manifest-path P] [--package conventional-3d]`** (new `xtask/src/canary.rs`): run `cargo_metadata::MetadataCommand` **with** dependency resolution (not `.no_deps()`), take `resolve` node for the package, compute the transitive closure over normal+build+dev edges for the chosen target set, and fail if any package in the closure has `manifest_path` under `extensions/planetary/` or name starting `planetary-`. Output a rule ID like `canary-planetary-reachable`. Add a negative fixture (`xtask/fixtures/canary-leak/`, excluded from workspace like the others — root `exclude = ["xtask/fixtures"]` already covers it) where an app transitively depends on a stub `planetary-*` crate; CI asserts the checker fails on it (same pattern as existing negative steps), plus a unit test on a pure graph function. `cargo_metadata 0.23.1` is already an xtask dependency. Also have the check assert `examples/basic_scene` source imports only `engine_sdk`/`conventional_3d`/std (a cheap grep-style test over `extensions/conventional-3d/examples/*.rs`) to cover APP-01 "SDK only".
2. **CI job `canary` (ubuntu)**: physically remove the planetary crate and build/test the rest:
   ```bash
   rm -rf extensions/planetary
   sed -i '/extensions\/planetary\/planetary-core/d' Cargo.toml
   cargo build --workspace            # no --locked: Cargo.lock legitimately changes (planetary-core entry removed)
   cargo test --workspace
   cargo clippy -p conventional-3d --all-targets -- -D warnings
   cargo build -p conventional-3d --example basic_scene
   ```
   Caveats: `xtask/tests/workspace_policy.rs` / `docs_policy.rs` may assert membership of planetary-core or docs rows — check them before this step (likely run `cargo test --workspace --exclude xtask` or use `-p engine-sdk -p conventional-3d ...`, or make the policy test skip when the dir is absent). This is the real "runs with those crates absent" proof; it also fails if any `Cargo.toml` path-references the planetary directory.
3. **Cheap fast step in the main matrix**: `cargo test --workspace --exclude planetary-core --locked` (the literal CONTEXT item). Be aware this proves little alone: `--exclude` skips building a package, it does not prove no dependency path reaches it, hence (1) and (2).
Also add the new subcommand to `xtask/src/main.rs` (currently accepts only `check-deps`; unknown args exit 2) and to docs (`docs/architecture/dependency-direction.md` "Checked rules" + an ADR 0007 for the lit-render/material/SDK-exposure decisions and canary strategy — next ADR number is 0007).

### Anti-Patterns to Avoid
- **Giving `conventional-3d` direct deps on engine-world/engine-assets/engine-jobs:** violates the documented table and APP-01; re-export via SDK.
- **Leaking wgpu types:** no `wgpu::` in any signature outside engine-render-wgpu; readback helper returns `Vec<u8>`.
- **Using `TextureId` 1/2 for sampled textures:** collision with surface targets; allocate from 3.
- **Per-draw material bind-group churn:** one bind group per texture created at upload time; draws only select it.
- **Normal = model matrix directly:** wrong under non-uniform scale; use inverse-transpose.
- **Decoding images via unbounded `gltf::import`:** use limits.
- **Treating `--exclude planetary-core` as the canary:** insufficient (see Pattern 8).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| glTF parsing, accessor decoding (strides, normalized ints, sparse) | Custom JSON/binary parser | `gltf` crate reader API | Accessor/stride/sparse edge cases |
| PNG/JPEG decode | Custom decoder | `image` (png, jpeg) with `Limits` | Security and correctness |
| Base64 data URIs | Custom decoder | gltf `import` feature (base64) | Already included |
| Matrix inverse/transposes, quats | Custom math | glam (`Mat3::inverse`, `Quat`) | Already pinned |
| Byte casting of uniforms | manual byte pushing for new structs | `bytemuck` `Pod` structs | Already used in backend |
| Dependency graph walking | Custom Cargo.toml parser | `cargo_metadata` with resolve | Already an xtask dep |
| Mipmap generation | wgpu blit chain | (deferred) CPU box filter in importer if wanted | Out of scope for canary |

**Key insight:** the lighting/material model is intentionally tiny; the risk is plumbing (handles, validation, SDK surface, layering), not shading math.

## Runtime State Inventory
Not a rename/refactor/migration phase. Omitted.

## Common Pitfalls

### Pitfall 1: Uniform layout mismatch between Rust and WGSL
**What goes wrong:** garbled lighting or validation error on `min_binding_size`.
**Why:** `mat3x3<f32>` occupies 48 bytes (3 columns padded to 16 B); `vec3` aligns to 16.
**How to avoid:** `#[repr(C)] Pod` uniform structs with explicit padding; unit test `size_of` and field offsets; keep per-draw payload <= 256 B stride.
**Warning signs:** wgpu validation message about buffer binding size; lit surfaces black or flickering.

### Pitfall 2: Reserved texture ids and validation holes
**What goes wrong:** `TextureId` 1/2 reused, or a stale/unknown texture id reaches the backend and panics on `HashMap` lookup.
**How to avoid:** ids start at 3, never reused (same as buffers); `validate_packet` rejects unknown textures; backend maps missing entries to `RenderError::Backend` (existing style), never `unwrap`.

### Pitfall 3: Winding/culling and handedness
**What goes wrong:** cube renders inside-out or invisible.
**Why:** glTF is CCW front, right-handed; wrong `front_face`/`cull_mode` or flipped Y.
**How to avoid:** `front_face: Ccw`, `cull_mode: Back` for lit pipeline; no Y flip needed in wgpu with the `directx` glam projection; GPU readback test asserts a known pixel.

### Pitfall 4: Light direction sign
**What goes wrong:** faces facing the light are dark.
**How to avoid:** document "direction light travels"; unit-test Lambert on CPU reference (`ndl = max(dot(n, -dir), 0)`) and one GPU readback test.

### Pitfall 5: Camera-relative lighting space
**What goes wrong:** treating normals/light as relative to a rotated frame.
**Why:** `extract` makes translation camera-relative but keeps world rotation axes (view is rotation-only and only appears in `view_proj`). Model/normal matrices and the light direction are therefore all in world-axis-aligned space; this is consistent. Do not multiply light by the view matrix.

### Pitfall 6: Index validity and empty slices
**What goes wrong:** out-of-range indices (wgpu clamps/robust-accesses but results are garbage) or `slice(..)` panic on empty buffer.
**How to avoid:** validate `max(index) < vertex_count`, non-empty, multiple of 3 at `create_lit_mesh`; keep `EmptyBuffer` check.

### Pitfall 7: Image format surprises
**What goes wrong:** JPEG yields RGB (no alpha), PNG may be 16-bit/palette/grayscale.
**How to avoid:** decode with `image` then `to_rgba8()`; reject exotic formats; flip nothing (glTF uv origin is top-left, matches wgpu textures; do not flip V).

### Pitfall 8: Missing normals/uvs/indices in glTF
**How to avoid:** generate flat normals and zero uvs and sequential indices; test with a primitive lacking each attribute (procedural builder can emit those variants).

### Pitfall 9: `world_transform()` dirty errors and f64 precision
**What goes wrong:** `DirtyTransform` if `update_transforms()` not called; precision loss if converting to f32 early.
**How to avoid:** call `update_transforms()` before flush; keep `RenderItem.position` as `WorldPos` (f64), place the demo scene away from the origin (e.g. x=1e6) in at least one test to confirm camera-relative precision (existing `precision.rs` pattern).

### Pitfall 10: The canary only checking direct deps
**What goes wrong:** a transitive planetary dependency passes CI.
**Why:** existing `check-deps` uses `.no_deps()`.
**How to avoid:** Pattern 8 (resolved graph walk plus negative fixture).

### Pitfall 11: Clippy/RUSTFLAGS `-D warnings` and `result_large_err`
**What goes wrong:** CI fails on warnings (existing `#![allow(clippy::result_large_err)]` exists in sdk and in `triangle.rs`/tests that return `SdkError`).
**How to avoid:** add the same allow to the extension crate, example and tests that return `SdkError`.

### Pitfall 12: GPU readback row alignment
**What goes wrong:** `copy_texture_to_buffer` requires `bytes_per_row` to be a multiple of 256.
**How to avoid:** pad rows when implementing `read_offscreen_rgba8()`; map the buffer with `pollster`/`device.poll`; keep it behind the ignored GPU test only.

## Code Examples

### Texture upload (wgpu 30.0.2, verified signatures)
```rust
// Source: wgpu-30.0.2/src/api/queue.rs, wgpu-types-30.0.2/src/texture.rs
let texture = device.create_texture(&wgpu::TextureDescriptor {
    label: Some(desc.label), size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
    format: wgpu::TextureFormat::Rgba8UnormSrgb,
    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
});
queue.write_texture(texture.as_image_copy(), rgba8,
    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
    wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 });
let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
    mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear,
    mipmap_filter: wgpu::MipmapFilterMode::Linear, address_mode_u: wgpu::AddressMode::Repeat,
    address_mode_v: wgpu::AddressMode::Repeat, ..Default::default() });
```
Bind group layout entries: `wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }` and `wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)` (type names from training knowledge; confirm by compiling — they match the 30.x layout in `wgpu-types`).

### glTF primitive read (gltf 1.4.1)
```rust
// Source: gltf crate reader API (docs.rs/gltf/1.4.1)
let reader = primitive.reader(|b| Some(&buffers[b.index()]));
let positions: Vec<[f32;3]> = reader.read_positions().ok_or(ImportError::NoPositions)?.collect();
let normals = reader.read_normals().map(|n| n.collect::<Vec<_>>());
let uvs = reader.read_tex_coords(0).map(|t| t.into_f32().collect::<Vec<_>>());
let indices = reader.read_indices().map(|i| i.into_u32().collect::<Vec<u32>>());
```
`buffers` here come from `gltf::import_buffers(&document, base_dir, blob)` where `blob = gltf.blob.take()` for `.glb`.

### Canary closure check skeleton
```rust
// Source: cargo_metadata resolve graph (already a dependency of xtask)
let md = MetadataCommand::new().manifest_path(p).exec()?;       // NOT .no_deps()
let resolve = md.resolve.as_ref().ok_or(...)?;
// BFS from the root package id over resolve.nodes[].deps[].pkg; flag any package whose
// manifest_path contains "extensions/planetary" or whose name starts with "planetary-".
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `FilterMode` for `mipmap_filter` | `MipmapFilterMode` | wgpu 25+/30 | Sampler descriptor must use the new enum |
| push constants range | `immediate_size` + `var<immediate>` | wgpu 28+/30 | Not used here; keep `immediate_size: 0` |
| `Mat4::perspective_infinite_reverse_rh` | `glam::camera::rh::proj::directx::perspective_infinite_reverse` | glam 0.34 | Already in camera.rs |
| `gltf::import` decoding everything | `import_buffers` + own `image` decode | gltf 1.x | Bounded decoding |

**Deprecated/outdated:** none relevant; gltf 1.4.1 is the newest published (May 2024) and is stable for this scope.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `wgpu::BindingType::Texture{..}`/`SamplerBindingType::Filtering`/`TextureSampleType::Float{filterable}` names unchanged in 30 | Code Examples | Compile error only; check in first backend task |
| A2 | Default `max_texture_dimension_2d` is 8192 with `DeviceDescriptor::default()` (downlevel-friendly limits) | Pattern 2 | Texture cap slightly off; harmless |
| A3 | `gltf` `Reader::read_tex_coords(0).into_f32()` and `Node::transform().decomposed()` signatures as in gltf 1.x | Pattern 5 | Compile error only |
| A4 | Removing `extensions/planetary` leaves `xtask` policy tests failing unless adjusted | Pattern 8 | Planner must inspect `workspace_policy.rs`/`docs_policy.rs` first (not read in detail) |
| A5 | `image::ImageReader` + `Limits` API as in 0.25 (`set_limits`, `max_image_width`, `max_alloc`) | Pattern 5 | Compile error only |
| A6 | Omitting vertex color from the first layout is acceptable under "optional vertex color" | Pattern 1 | User may want it; cheap to add later |

## Open Questions (RESOLVED)

1. **How do materials/textures/lights reach backends without leaking wgpu?** RESOLVED: engine-render data types; resolved `Material` + normal matrix per `DrawCommand`, `DirectionalLight` on `FramePacket`; `RenderBackend::create_texture`; backend owns bind groups (Patterns 1-3).
2. **Can the extension use world/assets via the SDK?** RESOLVED: yes, via new `pub use` re-exports in `engine-sdk` (which already depends on them); extension keeps `engine-sdk` as its only workspace dependency (Pattern 4).
3. **Where does glTF import live; which crates may depend on gltf/image?** RESOLVED: `conventional-3d` only; enforced by new cargo-deny `bans` wrappers; licenses already allowed, no `deny.toml` license change.
4. **How to prove planetary-absent?** RESOLVED: `xtask check-canary` (transitive, with negative fixture) + CI job that deletes `extensions/planetary` + `--exclude` quick step (Pattern 8).
5. **Async load vs `Resources` only in `setup`?** RESOLVED: decode on a job, block in `setup` to upload; do not change `App::update` signature this phase.
6. **Index buffers or de-index?** RESOLVED: add u32 index support; de-indexing is the documented fallback if plan size must shrink.
7. **Do the existing xtask policy tests break when planetary is removed?** Not fully inspected; flagged as plan-time check (A4), mitigation described.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain 1.99.0 | all | yes | 1.99.0 (rustup toolchain installed; default `cargo` outside repo is 1.69 so use `cargo +1.99.0` in scratch dirs) | — |
| cargo-deny | bans/licenses | yes | 0.22.1 | CI installs via taiki-e |
| Local GPU Intel Arc B580 / Vulkan | ignored GPU tests | yes (per task statement) | — | NullBackend tests in CI |
| crates.io network | fetch gltf/image | yes | — | — |
| bash on Windows runner | CI `rm`/`sed` canary step | yes (`defaults.run.shell: bash`) | — | — |

**Missing dependencies:** none blocking. GitHub CI is currently not running (billing); verify all CI commands locally.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | built-in `cargo test` (libtest), integration tests in `tests/`, no external framework; xtask tests via `cargo test -p xtask` |
| Config file | none (workspace `Cargo.toml`; `RUSTFLAGS=-D warnings` in CI) |
| Quick run command | `cargo test -p engine-render -p engine-sdk -p conventional-3d --locked` |
| Full suite command | `cargo test --workspace --locked && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo run --locked -p xtask -- check-deps && cargo run --locked -p xtask -- check-canary` |
| GPU (manual, local) | `cargo test -p engine-render-wgpu -- --ignored` (Arc B580/Vulkan) |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REND-04 | Lit packet carries light, material, normal matrix; reverse-Z invariants kept; unknown texture/mismatched layout rejected by `validate_packet` | unit/integration (NullBackend) | `cargo test -p engine-render` | Wave 0 (extend `tests/null_backend.rs`) |
| REND-04 | Uniform struct sizes/offsets; WGSL has entry points/bindings; lit pipeline depth state Greater | unit | `cargo test -p engine-render-wgpu --lib` | Wave 0 (extend `pipeline.rs` tests) |
| REND-04 | Lit/textured cube renders lit side brighter than shaded side, pixels != clear | GPU, ignored | `cargo test -p engine-render-wgpu -- --ignored` | Wave 0 (new `tests/gpu_lit.rs`, needs `read_offscreen_rgba8`) |
| REND-05 | glTF fixture loads: mesh/vertex/index counts, texture dims, material factor, node transform | unit/integration | `cargo test -p conventional-3d --test gltf_import` | Wave 0 |
| REND-05 | Malformed/oversized glTF/image rejected without panic (truncated file, bad index, huge dims) | unit | `cargo test -p conventional-3d --test gltf_import` | Wave 0 |
| REND-05 | Import runs on `JobSystem` and result registered in `AssetRegistry` | integration | `cargo test -p conventional-3d --test model_loader` | Wave 0 |
| APP-01 | Headless full frame of the scene via `Runtime<NullBackend>` yields draws with textures/material/light; fly camera moves/rotates from synthetic `InputEvent`s | integration | `cargo test -p conventional-3d --test headless_scene` | Wave 0 |
| APP-01 | Example sources import only `engine_sdk`/`conventional_3d` | policy test | `cargo test -p conventional-3d --test sdk_only` (or xtask test) | Wave 0 |
| APP-01 | `basic_scene` builds | build | `cargo build -p conventional-3d --example basic_scene` | Wave 0 |
| PLAN-01 | No planetary crate in `conventional-3d` transitive closure; negative fixture fails | xtask | `cargo run -p xtask -- check-canary` and `! cargo run -p xtask -- check-canary --manifest-path xtask/fixtures/canary-leak/Cargo.toml` | Wave 0 |
| PLAN-01 | Workspace builds/tests with `extensions/planetary` removed | CI job | script in Pattern 8 | Wave 0 (`ci.yml`) |
| WS rules | gltf/image confined (cargo-deny) | tooling | `cargo deny check bans licenses sources advisories` | extend `deny.toml` |

### Sampling Rate
- **Per task commit:** `cargo test -p <touched crates> --locked` + `cargo clippy -p <crate> --all-targets -- -D warnings`
- **Per wave merge:** full suite command above
- **Phase gate:** full suite green, `cargo deny check`, ignored GPU test run locally on Arc B580, manual `cargo run -p conventional-3d --example basic_scene` window check (WASD, mouse look with RMB, shift boost), and local run of the canary CI script on a throwaway copy of the repo before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `crates/engine-render/tests/lit_packet.rs` (or extend `null_backend.rs`) covering REND-04 validation
- [ ] `crates/engine-render-wgpu/tests/gpu_lit.rs` + `WgpuBackend::read_offscreen_rgba8`
- [ ] `extensions/conventional-3d/tests/{gltf_import,model_loader,headless_scene,sdk_only}.rs` and the checked-in asset `assets/textured_cube.gltf` (generate once with an `#[ignore]` generator test/example using `image`'s PNG encoder and hand-built accessors; commit the output, keep it < ~10 KB)
- [ ] `xtask/src/canary.rs`, `xtask/fixtures/canary-leak/`, `xtask/tests` additions; `main.rs` subcommand
- [ ] `ci.yml`: add canary job and negative fixture step; `deny.toml` bans for gltf/image
- [ ] Framework install: none

## Security Domain

`security_enforcement` is absent from config, so treated as enabled.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | Bounds on file size, texture dims/alloc (`image::Limits`), vertex/index counts, node depth; `gltf` validation; reject non-finite transforms; validate handles in `ResourceTable` |
| V6 Cryptography | no | — (AssetId uses uuid v5 hashing for identity only, not security) |
| V12 Files/Resources | yes | Relative-URI base-dir confinement for external `.bin`/image URIs; no network fetching |

### Known Threat Patterns
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Decompression bomb / giant texture | DoS | `image::Limits`, 8192 cap, total alloc cap before upload |
| Malformed glTF (bad accessor/index) | DoS / Tampering | `gltf` validation, no panics (`unsafe_code = forbid`), explicit index range check before GPU upload |
| Path traversal via URI (`../../`) | Info disclosure | Resolve URIs relative to model dir, reject escapes |
| Unbounded node recursion/cycles | DoS | Iterative traversal with depth/node caps |
| Supply chain (new crates) | Tampering | `cargo deny` licenses/advisories/sources; explicit pinned versions; `--locked` |
| GPU validation panics from bad packets | DoS | Shared `validate_packet` for both backends before any wgpu call |

## Sources

### Primary (HIGH confidence)
- Local repo code read in full: `engine-render`, `engine-render-wgpu`, `engine-sdk`, `engine-platform/input.rs`, `engine-assets` (`registry`, `loader`), `engine-world` (`transform`, `world`), `xtask`, `deny.toml`, `ci.yml`, `docs/architecture/dependency-direction.md`
- `wgpu-30.0.2` and `wgpu-types-30.0.2` source in the local cargo registry (`write_texture`, `TexelCopyBufferLayout`, `SamplerDescriptor`, `MipmapFilterMode`, `PipelineLayoutDescriptor`, `set_index_buffer`, `draw_indexed`, `as_image_copy`)
- Scratch crate with `gltf 1.4.1` (`default-features = false`, `import,utils,names`) + `image 0.25.10` (`png,jpeg`): `cargo tree`, `cargo deny check licenses bans advisories` against the repo `deny.toml`, including wrapper-ban behaviour
- `gltf-1.4.1/src/import.rs`, `image.rs` (import API, `image::Data`/`Format`) and gltf `Cargo.toml` (features)
- crates.io API / `cargo search` for gltf 1.4.1 and image 0.25.10 current versions

### Secondary (MEDIUM confidence)
- Phase 3 summary 03-05 (wgpu 30 deviations list), Phase 3/4 context

### Tertiary (LOW confidence)
- None beyond items listed in the Assumptions Log (training knowledge for some type names)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH, resolved/licensed/deny-checked in scratch crate
- Architecture: MEDIUM-HIGH, derived from reading actual code; exact signatures are recommendations
- Pitfalls: HIGH for items grounded in read code/APIs, MEDIUM for GPU runtime behaviour (not executed this session)

**Research date:** 2026-11-25
**Valid until:** 2026-12-25 (stable stack; revisit if wgpu or glam bumps)
