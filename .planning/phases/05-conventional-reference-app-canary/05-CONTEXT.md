# Phase 5: Conventional Reference App (Canary) - Context

**Gathered:** 2026-11-26
**Status:** Ready for planning

<domain>
## Phase Boundary

A conventional 3D scene runs on the SDK alone, proving the core is general-purpose (REND-04, REND-05, APP-01, PLAN-01). Touches engine-render (+ -wgpu) for lit/material rendering, extensions/conventional-3d for glTF import, scene builder and fly camera, and CI/xtask for the planetary-absent canary.

</domain>

<decisions>
## Implementation Decisions

### Rendering features
- Simple lit shading: Lambert diffuse + ambient with one directional light; WGSL consumes a per-frame light uniform and per-draw material. PBR deferred.
- `Material` = base color factor + optional base-color texture, referenced by engine-owned `MaterialId`/`TextureId` handles in `RenderScene`.
- One standard vertex layout: position + normal + uv (+ optional vertex color).

### glTF loading
- `gltf` 1.4.1 + `image` 0.25 in the extension layer; glTF converted to engine mesh/material/texture data and registered via the Phase 4 `AssetRegistry`, CPU decode in jobs.
- Scope: static meshes, base-color textures, node transforms only; no skins/animation/PBR extensions.
- Tiny checked-in glTF/glb test asset (textured cube) plus a procedural fallback for headless tests.

### App & camera
- `extensions/conventional-3d` crate: scene builder, fly-camera controller, and a `basic_scene` example depending only on `engine-sdk` + the extension.
- Fly camera: WASD + mouse look + shift boost from SDK `InputEvent`s, f64 `WorldPos` camera.

### Canary / CI
- Planetary-absent proof: build/test the conventional app with `planetary-core` excluded (CI step with `--exclude planetary-core` or equivalent), plus an xtask assertion that no dependency path from the app reaches planetary crates.
- Headless tests (scene extraction, full frame via NullBackend) run in CI; real window run is a manual item; software-adapter CI deferred to Phase 8.

### Claude's Discretion
Names, WGSL details, uniform layout, test structure, how the SDK exposes materials/lights.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- Phase 3: RenderBackend trait, NullBackend, WgpuBackend (triangle pipeline, reverse-Z Depth32Float), Camera + extract(), render graph, FramePacket, SDK Runtime/run_app/App/examples/triangle.rs.
- Phase 4: World (entities, hierarchy, transforms), SpatialIndex/HashGridIndex, AssetRegistry/ResidencyManager/AsyncLoader.

### Established Patterns
- Rust 1.99.0, edition 2024, unsafe forbid, explicit versions in [workspace.dependencies], cargo-deny (wildcards deny, license allow-list), xtask layer table must stay green, ADRs in docs/architecture/adr (next 0007), CI matrix on Linux/macOS/Windows (not yet running: GitHub billing issue).
- wgpu/winit only in engine-render-wgpu/engine-platform; apps use only engine-sdk.

### Integration Points
- conventional-3d depends on engine-sdk (stub). The SDK needs to expose whatever the app needs (materials, lights, world/asset access) without leaking wgpu/winit.

</code_context>

<specifics>
## Specific Ideas

The canary must fail loudly if any planetary concept leaks into core: the planetary-absent build is the proof for PLAN-01.

</specifics>

<deferred>
## Deferred Ideas

PBR, shadow maps, animation/skins, asset import pipeline (v2); software-adapter image-diff CI (Phase 8).

</deferred>
