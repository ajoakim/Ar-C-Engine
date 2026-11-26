# Phase 4: World, Spatial & Assets - Context

**Gathered:** 2026-11-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Developers can build scenes of stable entities, query them spatially, and manage resources with residency states (WORLD-01, WORLD-05, ASSET-01, ASSET-02, ASSET-03). Crates: engine-world, engine-spatial, engine-assets (stubs from Phase 1; deps already wired: world->core,math; spatial->math,world; assets->core,jobs).

</domain>

<decisions>
## Implementation Decisions

### World / entities
- Engine-owned generational `EntityId` over a small `World` API; no ECS crate (storage hidden behind the ID so ECS stays a two-way door).
- Parent/child hierarchy with local `Transform` (f64 translation, f32 rotation/scale) in a `Frame`; world transforms recomputed top-down on demand with dirty flags.
- Despawning a parent recursively despawns children; stale IDs return None/Err, never panic.

### Spatial
- `SpatialIndex` trait: insert/update/remove, query_frustum / query_region / query_radius returning `EntityId`s.
- Default backend: uniform hash grid in f64 world space; BVH/rstar benchmark deferred to the Phase 8 ADR spike.
- Queries use world-space f64 volumes built from the camera `WorldPos` (no f32 loss at distance).

### Assets / residency
- `AssetId` (stable uuid-v5 from path/key) plus typed `Handle<T>` (index + generation); registry maps ID -> handle.
- Residency states Unloaded -> Requested -> Generating -> CpuReady -> Uploading -> GpuResident -> Evicting / Failed as an enforced transition table (illegal transitions are errors).
- Per-class CPU/GPU byte budgets plus a per-frame upload byte cap in the residency manager; LRU eviction candidates reported, eviction driven by the caller.
- Async CPU work uses the Phase 2 job system, exercised with a synthetic loader; the real streaming scheduler is Phase 6.

### Claude's Discretion
Names, error types, module layout, test structure, exact hash-grid cell sizing.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- engine-math: `WorldPos`, `FrameRegistry`, rebasing. engine-jobs: priority/cancellable scheduler. engine-core: Engine/Module. engine-render: `RenderScene` extraction (Phase 5 will connect world -> extraction).

### Established Patterns
- Rust 1.99.0, edition 2024, `unsafe_code = forbid`, clippy -D warnings (`result_large_err` allowed where EngineError is wrapped), explicit versions in [workspace.dependencies], xtask layer table/deny.toml must stay green, ADRs in docs/architecture/adr (next number 0006), CI on Linux/macOS/Windows.

### Integration Points
- Phase 5 conventional app consumes engine-world for scenes; Phase 6 streaming uses the residency state machine.

</code_context>

<specifics>
## Specific Ideas

ADR for spatial backend choice recorded as "hash grid first, benchmark later" (feeds WS-05).

</specifics>

<deferred>
## Deferred Ideas

ECS adoption, BVH/rstar/kiddo spatial backends (benchmark ADR in Phase 8); real streaming scheduler (Phase 6).

</deferred>
