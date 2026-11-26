# ADR-0007: Lit Rendering, Materials, glTF Import and the Planetary-Absent Canary

- Status: accepted
- Date: 2026-11-26
- Requirement/Phase: REND-04, REND-05, APP-01, PLAN-01 / Phase 5

## Context

Phase 5 delivers the conventional reference app, which must render lit, textured glTF content using only the public SDK, and must prove that the engine and the conventional extension do not depend on the planetary extension. This needs a material and lighting contract in `engine-render`, a bounded glTF importer, SDK exposure for apps, and a machine-checked canary.

## Decision

1. Shading: Lambert diffuse + ambient, one `DirectionalLight` per frame (`direction` = unit vector the light travels; shader uses `max(dot(n, -direction), 0)`). PBR, shadows, skins and animation are deferred.
2. The data contract lives in engine-render with no GPU types: `Material { base_color, base_color_texture: Option<TextureId> }`, `MaterialId` (id 0 = default white material), `MaterialTable`, `DirectionalLight`, `TextureDesc`, `VertexPosNormalUv` (32 bytes: position, normal, uv) and `VertexLayout::PosNormalUv`; `ShaderId::LitTextured`; `MeshHandle` gains its `VertexLayout` and an optional `u32` index buffer.
3. Vertex color is deliberately omitted from the first lit layout (the "optional vertex color" in CONTEXT): the layout enum is `#[non_exhaustive]` so a `PosNormalUvColor` layout can be added later without breaking callers.
4. Sampled `TextureId`s start at 3 (1 = backbuffer, 2 = depth stay reserved surface targets) and are never reused; textures are RGBA8 uploaded as `Rgba8UnormSrgb`, single mip, max 8192 per side; `validate_packet` is shared by the Null and wgpu backends.
5. Per-frame light uniform (bind group 0), per-draw dynamic uniform of 192 bytes in the existing 256-byte stride (bind group 1: mvp, model, normal matrix, base color), per-texture bind group (group 2) created at upload time. Normal matrix = inverse-transpose of the camera-relative model's upper 3x3, computed in `extract`.
6. SDK exposure: engine-sdk re-exports world/assets/jobs/material types and adds `Resources::create_lit_mesh`, `Resources::create_texture`, `SceneState.materials`, `SceneState.light`; `App` trait signatures stay unchanged; apps and extensions name only `engine_sdk`.
7. glTF import lives only in `extensions/conventional-3d` (`gltf` 1.4.1 + `image` 0.25.10, confined by cargo-deny wrappers): `gltf::import_buffers` plus our own bounded `image` decode (dimension, allocation and file-size caps), URI confinement (no `..`, absolute, `file:` or percent-encoded URIs), static meshes + base-color textures + node TRS only. Decode runs on the `JobSystem`; `App::setup` blocks on the job before GPU upload because `Resources` is only available in `setup`.
8. Canary (PLAN-01): `cargo xtask check-canary` walks the resolved (transitive) dependency graph of `conventional-3d` and fails with rule `canary-planetary-reachable` if any `planetary-*` package or anything under `extensions/planetary` is reachable; a negative fixture proves it fails. A CI `canary` job deletes `extensions/planetary`, removes it from workspace members and builds, tests and runs the app headless. `--exclude planetary-core` is kept only as a cheap extra step because on its own it proves nothing.

## Alternatives Considered

- De-indexing meshes instead of adding index buffers: rejected, it inflates vertex data and loses sharing.
- `gltf::import` eager unbounded decode: rejected, no size or dimension caps on untrusted input.
- Direct engine-world/engine-assets dependencies from the extension: rejected, violates the layer table and APP-01.
- `cargo xtask check-deps` alone as the canary: rejected, `.no_deps()` sees only direct dependencies, not transitive reachability.
- Mip generation: deferred.

## Consequences

- `extract` gains a 7-argument signature; all call sites are updated.
- The workspace transiently does not compile between the engine-render and engine-sdk plans; plan 05-06 restores it.
- Single-mip textures may shimmer under minification.
- No vertex color in the first lit layout.
- Shear in glTF node matrices is dropped by TRS decomposition.
- Two new third-party crates (gltf, image) enter the tree, confined by cargo-deny wrappers.

## Evidence

- Research verification: scratch crate cargo-deny run and wgpu 30.0.2 source reads.
- Tests added in this phase: `crates/engine-render/tests/lit_packet.rs`, `crates/engine-render-wgpu/tests/gpu_lit.rs`, `extensions/conventional-3d/tests/gltf_import.rs`, `xtask/tests/canary.rs`.
