# Phase 7: Planetary Extension & Terrain LOD - Context

**Gathered:** 2026-11-26
**Status:** Ready for planning

<domain>
## Phase Boundary

A user can fly from orbit to the surface of a procedurally generated planet with crack-free, streamed terrain (PLAN-02..PLAN-07, APP-02). All planetary code lives only in `extensions/planetary/*`; the core must not learn about planets (Core Value). Builds on Phase 5 (renderer, SDK, conventional-app canary) and Phase 6 (generic streaming, procedural layers).

</domain>

<decisions>
## Implementation Decisions

### Topology
- Planetary code stays in `extensions/planetary/*`: `planetary-core` (math/topology/LOD, GPU-free) plus new crates for the terrain render pass/camera and the planetary app/example. New crates need xtask layer-table, dependency-direction doc, workspace and canary-job consistency (planetary crates must remain removable from the workspace without breaking the conventional build).
- Cube-sphere with the tangent-adjusted (equal-angle) mapping, f64 throughout; per-face patch grids with a tested adjacency table across all 12 edges and 8 corners (reciprocity tests).
- Patch ID: one packed `u64` = face (3 bits) + level (5 bits) + interleaved x/y morton bits; parent, children and neighbors derived from the ID, not stored.

### LOD
- Quadtree refined by screen-space error with a pixel threshold, hysteresis band (no popping), and 2:1 balancing (neighbors differ by at most 1 level).
- Planetary patches implement Phase 6 generic `StreamKey` and `Generator`; the coarse parent shows until the child is resident (pinned fallback).
- Terrain from layered planet-space generators (base shape + detail + mask) sampled in unit-sphere space so shared borders agree exactly (no per-patch local noise).

### Crack-free technique (user decision)
- **Skirts + shared-border sampling**: border vertices come from the same planet-space function so same-level neighbors match exactly; a small downward skirt hides T-junction gaps between different levels. 2:1 balance keeps skirts small. Geomorphing is a possible later improvement. Record as an ADR (next free number) with prototype evidence (WS-05 LOD technique decision).

### Rendering & app
- Each patch has its own f64 origin; vertices stored as f32 offsets from it; uses existing camera-relative extraction.
- Planetary camera: orbit → surface navigation with altitude-scaled speed, SDK input only; no gravity/walking.
- Atmosphere is deferred to Phase 8 (PLAN-08, first to cut).

### Verification
- CPU-side seam proof: sample every shared boundary vertex from both patches and assert equality (zero gaps) incl. cube-face edges and corners; GPU readback optional.
- Scripted orbit-to-surface descent on a simulated clock asserts counted per-frame work caps (patches started/applied/uploaded), no wall-clock.
- Planetary app uses the public SDK only (like Phase 5's `sdk_only` policy test); manual window run is a human-verification item.

### Claude's Discretion
Crate names, patch grid resolution, skirt depth, error thresholds, ADR number, module layout, test structure, planet radius/seeds for the demo.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- Phase 5: lit/textured renderer with indexed meshes, MaterialTable, DirectionalLight, SDK Runtime/App/run_app, FlyCamera, `sdk_only` policy test, `check-canary` and the planetary-absent CI job (it removes `extensions/planetary` — new planetary crates must live under that directory so removal works).
- Phase 6: engine-streaming (StreamKey, StreamManager, pins/resolve), engine-procedural (hash, noise, LayerStack, goldens).
- Phase 4: world, spatial, assets; Phase 2: jobs, WorldPos/Frames/rebase.

### Established Patterns
- Rust 1.99.0, edition 2024, unsafe forbid, explicit versions, cargo-deny, xtask layer table + docs policy tests, ADRs (next after 0008), CI matrix on 3 OSes (not running: GitHub billing), disk tight (~18 GB), cross-target `cargo check` for Linux/macOS from Windows.

### Integration Points
- `extensions/planetary/planetary-core` currently a stub. Planetary crates must not be depended on by any core/crate or by conventional-3d (check-canary).

</code_context>

<specifics>
## Specific Ideas

Seam test cameras placed at LOD boundaries, cube-face edges and corners. Golden hashes for terrain patches (same exact-op discipline as Phase 6: no libm in hashed paths).

</specifics>

<deferred>
## Deferred Ideas

Geomorphing, atmosphere (Phase 8), software-adapter image-diff tests (Phase 8), gravity/walking, geospatial data ingestion, persistent edits.

</deferred>
