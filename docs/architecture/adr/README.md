# Architecture Decision Records

To add an ADR, copy `0000-template.md` to the next free number (4-digit zero-padded) with a kebab-case title, for example `0004-my-decision.md`, and fill in every section. Never rewrite the decision of an accepted ADR: supersede it with a new ADR and set the old one's status to `superseded by ADR-XXXX`, so decision history stays auditable in git.

| ADR | Title | Status |
|-----|-------|--------|
| [0001](0001-conventions.md) | Engine Conventions | accepted |
| [0002](0002-workspace-crate-layout.md) | Workspace Crate Layout, Render Split and License | accepted |
| [0003](0003-dependency-enforcement.md) | Dependency-Direction Enforcement and CI | accepted |
| [0004](0004-worldpos-frames-rebasing.md) | WorldPos, Coordinate Frames and Origin Rebasing | accepted |
| [0005](0005-rendering-api-wgpu-behind-trait.md) | Rendering API - wgpu Behind an Engine-Owned Backend Trait | accepted |
| [0006](0006-spatial-index-hash-grid-first.md) | Spatial Index - Uniform Hash Grid First, Benchmark Later | accepted |
| [0007](0007-lit-rendering-gltf-import-and-canary.md) | Lit Rendering, Materials, glTF Import and the Planetary-Absent Canary | accepted |
| [0008](0008-deterministic-streaming-and-procedural-generation.md) | Generic Streaming and Deterministic Procedural Generation | accepted |
| [0009](0009-sdk-per-frame-gpu-hook.md) | SDK Per-Frame GPU Hook and Mesh Destruction | accepted |
| [0010](0010-terrain-lod-skirts-and-shared-border-sampling.md) | Terrain LOD - Skirts, Shared-Border Sampling and 2:1 Balance | accepted (selection superseded by 0012) |
| [0011](0011-hex-grid-planet-topology.md) | Hex-Grid Planet Topology (Aperture-4 Half-Cell Hierarchy) | accepted |
| [0012](0012-terrain-lod-selection-tight-bounds-culling-budgets.md) | Terrain LOD Selection - Tight Bounds, Culling and Budgets | accepted |
| [0013](0013-shared-index-buffers-and-index-range-draws.md) | Shared Index Buffers and Index-Range Draws | accepted |
| [0014](0014-vertex-morph-attribute-geomorphing.md) | Vertex Morph Attribute for Geomorphing (Cell Payload v4) | accepted |
| [0015](0015-scale-contrast-and-distance-haze.md) | Scale, Contrast and Distance Haze (Generator v2) | accepted |
