# Phase 4: World, Spatial & Assets - Research

**Researched:** 2026-11-25
**Domain:** Rust engine data structures (generational IDs, transform hierarchy, spatial hash grid, asset residency state machine)
**Confidence:** HIGH (small dependency surface, all crate edges verified against the repo)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
**World / entities**
- Engine-owned generational `EntityId` over a small `World` API; no ECS crate (storage hidden behind the ID so ECS stays a two-way door).
- Parent/child hierarchy with local `Transform` (f64 translation, f32 rotation/scale) in a `Frame`; world transforms recomputed top-down on demand with dirty flags.
- Despawning a parent recursively despawns children; stale IDs return None/Err, never panic.

**Spatial**
- `SpatialIndex` trait: insert/update/remove, query_frustum / query_region / query_radius returning `EntityId`s.
- Default backend: uniform hash grid in f64 world space; BVH/rstar benchmark deferred to the Phase 8 ADR spike.
- Queries use world-space f64 volumes built from the camera `WorldPos` (no f32 loss at distance).

**Assets / residency**
- `AssetId` (stable uuid-v5 from path/key) plus typed `Handle<T>` (index + generation); registry maps ID -> handle.
- Residency states Unloaded -> Requested -> Generating -> CpuReady -> Uploading -> GpuResident -> Evicting / Failed as an enforced transition table (illegal transitions are errors).
- Per-class CPU/GPU byte budgets plus a per-frame upload byte cap in the residency manager; LRU eviction candidates reported, eviction driven by the caller.
- Async CPU work uses the Phase 2 job system, exercised with a synthetic loader; the real streaming scheduler is Phase 6.

### Claude's Discretion
Names, error types, module layout, test structure, exact hash-grid cell sizing.

### Deferred Ideas (OUT OF SCOPE)
ECS adoption, BVH/rstar/kiddo spatial backends (benchmark ADR in Phase 8); real streaming scheduler (Phase 6).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| WORLD-01 | Entities with stable generational identity and transform hierarchy | slotmap-backed `EntityId`, parent/child links, dirty-flag world transforms (sections Standard Stack, Pattern 1, 2) |
| WORLD-05 | Spatial queries (visibility, region lookup) via pluggable interface, one default backend | `SpatialIndex` trait + `HashGridIndex`, f64 `Frustum`/`Aabb`/sphere volumes (Pattern 3) |
| ASSET-01 | Stable identifiers and typed handles | `AssetId` = uuid v5, `Handle<T>` over slotmap `DefaultKey` (Pattern 4) |
| ASSET-02 | CPU/GPU residency state machine | Pure transition table + `ResidencyManager` (Pattern 5) |
| ASSET-03 | Caching, memory budgets, per-frame upload caps | Per-class budgets, per-frame cap, LRU candidates (Pattern 5) |
</phase_requirements>

## Summary

Phase 4 fills three stub crates with pure, GPU-free, mostly single-threaded data structures. The required dependency surface is tiny: `slotmap 1.1.1` (Zlib) for generational keys and `uuid 1.29.3` with feature `v5` (Apache-2.0 OR MIT, pulls `sha1_smol 1.0.1`, BSD-3-Clause). All licenses are already in the `deny.toml` allow list, and neither crate introduces wildcards. [VERIFIED: cargo info]

The layer table already permits every planned edge: world(2)->core(1), math(0); spatial(3)->math(0), world(2); assets(2)->core(1), jobs(1). `engine-render` (tier 3) already depends on `engine-world` and `engine-assets` (tier 2), so Phase 5 can write world->`RenderScene` extraction inside `engine-render` with no new edge. No `xtask/src/layers.rs` or `deny.toml` change is needed. [VERIFIED: xtask/src/layers.rs, crates/*/Cargo.toml]

Transform math lives in `engine-world` (it owns `Transform`, `WorldTransform`, hierarchy composition), using `engine-math`'s `WorldPos`/`FrameRegistry`/`DVec3`/`DQuat`. Spatial volumes (`Aabb`, `Sphere`, `Frustum`) live in `engine-spatial` and are built from primitive/`WorldPos` inputs, not from `engine-render`'s `Camera` (spatial and render are both tier 3 and may not depend on each other).

**Primary recommendation:** Add `slotmap` and `uuid` to `[workspace.dependencies]`; implement `EntityId` as `slotmap::new_key_type!`, `Handle<T>` as a PhantomData-wrapped `DefaultKey`, keep all state-machine and budget logic in pure deterministic code (frame counters passed in, no `Instant`), and test every success criterion with plain `cargo test`.

## Architectural Responsibility Map

| Capability | Primary Tier (crate) | Secondary | Rationale |
|------------|---------------------|-----------|-----------|
| EntityId, World, hierarchy, Transform math | engine-world (2) | engine-math (0) | Owns entity storage; consumes WorldPos/FrameRegistry |
| Origin rebasing of entity positions | engine-world | engine-math `rebase` | World holds a `PositionStore` so rebase reaches entity roots |
| Spatial volumes + SpatialIndex + HashGridIndex | engine-spatial (3) | engine-world (EntityId only) | Needs EntityId; must not touch render |
| AssetId, Handle<T>, registry | engine-assets (2) | — | No dependency on world or render |
| Residency state machine, budgets, upload cap | engine-assets | engine-jobs (1) | CPU async work via JobSystem; GPU upload is only a state/accounting step here |
| world -> RenderScene extraction (Phase 5) | engine-render (3) | engine-world, engine-assets | Edges already exist |
| Culling glue (Frustum from Camera) (Phase 5) | engine-sdk (5) / extension (6) | spatial + render | spatial and render are same tier, cannot depend on each other |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| slotmap | 1.1.1 (Zlib) | Generational arena; `EntityId`, asset `Handle<T>` backing | Canonical generational key crate; stale-key `get`/`remove` return `None`, never panic [VERIFIED: cargo search/info] |
| uuid | 1.29.3, `features = ["v5"]` (Apache-2.0 OR MIT) | `AssetId` via `Uuid::new_v5(&NAMESPACE, key.as_bytes())` | Deterministic name-based IDs; `v5 = ["sha1"]` feature confirmed in the crate manifest [VERIFIED: registry Cargo.toml] |
| glam (existing 0.34.3) | — | `DVec3`, `DQuat`, `Quat`, `Vec3` | Already workspace dependency; add `glam.workspace = true` to engine-world and engine-spatial (as engine-render does) |
| thiserror (existing 2.0.21) | — | Error enums | Project convention |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tracing (existing) | 0.1.44 | Residency/eviction events | Optional, assets only |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| slotmap | hand-rolled generation arena | ~100 lines plus edge cases; slotmap is Zlib, tiny, no transitive deps |
| uuid v5 | hand-rolled 128-bit hash | Locked decision says uuid-v5; stability across versions matters |
| proptest 1.12.0 | deterministic LCG loops | proptest is optional; avoid for Phase 4 to keep deny surface minimal (it pulls many transitive crates) |

**Installation (root Cargo.toml `[workspace.dependencies]`, explicit versions per convention):**
```toml
slotmap = "1.1.1"
uuid = { version = "1.29.3", default-features = false, features = ["v5"] }
```
Then `slotmap.workspace = true` in engine-world, engine-spatial (only if it needs `SecondaryMap`; otherwise plain `HashMap<EntityId,_>`), engine-assets; `uuid.workspace = true` and `thiserror.workspace = true` in engine-assets/world/spatial; `glam.workspace = true` in world/spatial.

**Version verification:** `cargo search`/`cargo info` on 2026-11-25: slotmap 1.1.1 (rust-version 1.58), uuid 1.29.3 (rust-version 1.89), sha1_smol 1.0.1 (BSD-3-Clause). Workspace MSRV is 1.95, toolchain 1.99.0, so all fine. [VERIFIED: cargo info]

## Architecture Patterns

### System Architecture Diagram

```
 App / test
    |
    | spawn/despawn/set_parent/set_local      insert/update/remove
    v                                               |
 engine-world::World ---- EntityId ---------------> engine-spatial::SpatialIndex (HashGridIndex)
   | SlotMap<EntityId, Node{parent, children,                ^
   |   local Transform, dirty, cached WorldTransform}>       | query_frustum / query_region / query_radius
   | PositionStore (for rebase)                              | -> Vec<EntityId> (sorted, deterministic)
   | world_transform(id, &FrameRegistry) --top-down--> WorldTransform{WorldPos, rot, scale}
   |
   +--(Phase 5)--> engine-render extract (iterates world; own HashMap<EntityId, MeshHandle>)

 engine-assets
   AssetRegistry: AssetId(uuid v5) <-> Handle<T>(slotmap key)
   ResidencyManager: per-asset state (transition table) + per-class cpu/gpu budgets
        begin_frame(frame_index) -> resets upload cap, records frame for LRU
        request(handle) -> Unloaded->Requested
        JobSystem::submit(synthetic loader) -> Generating -> CpuReady (via mpsc/atomic result polled by manager)
        try_begin_upload(handle, bytes) -> Uploading | Err(UploadCapExceeded / OverBudget)
        complete_upload -> GpuResident ; eviction_candidates(class, bytes_needed) -> LRU Vec<Handle>
        caller: begin_evict(handle) -> Evicting -> finish_evict -> Unloaded
```

### Recommended Project Structure
```
crates/engine-world/src/   lib.rs, entity.rs (EntityId), transform.rs (Transform, WorldTransform, compose), world.rs (World, hierarchy), error.rs
crates/engine-spatial/src/ lib.rs, volume.rs (Aabb, Sphere, Frustum), index.rs (trait), hash_grid.rs, error.rs
crates/engine-assets/src/  lib.rs, id.rs (AssetId), handle.rs, registry.rs, residency.rs (state + table), budget.rs (manager), loader.rs (synthetic job loader), error.rs
docs/architecture/adr/0006-spatial-index-hash-grid-first.md   (and optionally 0007 entity/handle storage)
```

### Pattern 1: EntityId and World
```rust
// Source: slotmap docs (docs.rs/slotmap/1.1.1) - new_key_type! generates a Copy+Eq+Hash+Ord key
slotmap::new_key_type! { pub struct EntityId; }   // storage hidden: do not re-export slotmap types in the API
struct Node { parent: Option<EntityId>, children: Vec<EntityId>, local: Transform, dirty: bool, cached: Option<WorldTransform> }
pub struct World { nodes: SlotMap<EntityId, Node>, positions: PositionStore /* for rebase */ }
```
- All accessors return `Option`/`Result<_, WorldError>` for stale IDs (`nodes.get(id)`); never index.
- `despawn(id)`: iterative (explicit stack), not recursive, to avoid stack overflow on deep chains; detach from parent's `children` first; return the number or list of despawned ids.
- `set_parent` must reject cycles (walk ancestors of the new parent; error `CycleDetected`) and self-parenting, and reject stale parent.
- Hide slotmap: the `new_key_type!` type is the public ID; `EntityId::default()` is a null key in slotmap, so document it as never valid (get returns None).

### Pattern 2: Transform and WorldTransform (where the math lives)
- `Transform { translation: DVec3, rotation: Quat (f32), scale: Vec3 (f32) }` local, relative to parent (or, for roots, relative to the root's `FrameId`).
- Root entities carry `frame: FrameId`; root world position = `WorldPos::new(frame, translation)`.
- `WorldTransform { position: WorldPos, rotation: Quat, scale: Vec3 }`.
- Child composition (all translation math in f64, rotation/scale f32 upcast per-op): `pos = parent.pos.translated(parent_rot_d * (parent_scale_d * child.translation))`, `rot = parent.rot * child.rot`, `scale = parent.scale * child.scale` (component-wise; non-uniform scale with rotated children shear is not representable, document "TRS-only, no shear").
- Dirty flags: `set_local` marks node and all descendants dirty (iterative); `world_transform(&mut self, id, &FrameRegistry)` walks up to the nearest clean ancestor then composes down caching results. Because the API needs `&mut self` for caching, provide also `world_transform_uncached(&self, ...)` for read-only use, or use `Cell`-free approach: `update_transforms(&mut self)` pass top-down then `&self` getters. Recommended: `World::update_transforms(&mut self)` (top-down from roots, dirty only) plus `world_transform(&self, id) -> Option<WorldTransform>` that returns cached value only when clean (else `None`/Err `DirtyTransform`), making invalidation observable in tests. Pick one and test it; the explicit update pass is simplest and matches "recomputed top-down on demand".
- Frame awareness: `FrameRegistry` is only needed when converting to a different frame (`frames.convert`). Composition within a root's frame needs no registry; `World::world_position_in(id, frames, target_frame)` calls `frames.convert`.
- Non-finite input: reject in `set_local` with `WorldError::NonFinite` (WorldPos::new already does for translation).
- Origin rebasing: store root positions in the world's `PositionStore` (`PosId` per root; re-register on reparent) and expose `World::rebase(&mut self, frames, frame, new_origin)` that delegates to `engine_math::rebase` and marks affected roots dirty. Test at 6.4e6 m with tolerance 1e-8 m (per ADR-0004). Simplest safe alternative if this proves costly: store only a `PosId` per root and everything else relative; this is the recommended design.

### Pattern 3: SpatialIndex and hash grid
```rust
pub trait SpatialIndex {
    fn insert(&mut self, id: EntityId, bounds: Aabb) -> Result<(), SpatialError>;  // error on duplicate
    fn update(&mut self, id: EntityId, bounds: Aabb) -> Result<(), SpatialError>;  // error if absent
    fn remove(&mut self, id: EntityId) -> bool;
    fn query_frustum(&self, f: &Frustum, out: &mut Vec<EntityId>);
    fn query_region(&self, r: &Aabb, out: &mut Vec<EntityId>);
    fn query_radius(&self, center: DVec3, radius: f64, out: &mut Vec<EntityId>);
    fn len(&self) -> usize;
}
```
- Index works in one explicit **index frame** (`FrameId` given at construction). The caller converts `WorldPos` into that frame (`frames.convert`) before insert; `Aabb`/`Frustum` hold `DVec3` in that frame. Provide helper constructors taking `WorldPos` and returning `Err(FrameMismatch)` when the frame differs. This preserves "f64 volumes from camera WorldPos".
- `HashGridIndex`: `HashMap<[i64;3], Vec<EntityId>>` cell map + `HashMap<EntityId, (Aabb, cells range)>` for update/remove; cell size configured (default 64 m is a reasonable start, tunable; Claude's discretion). Insert each entity into every cell its AABB overlaps; cap the cell span per entity (e.g., more than N cells goes into a separate `oversize: Vec<EntityId>` list that is always tested) to avoid huge fan-out for planet-size bounds.
- Queries: gather candidate cells, dedupe (entity may be in several cells) with sort+dedup, then exact test (AABB-vs-AABB, sphere-vs-AABB, frustum-vs-AABB via 6 plane p-vertex test). Output sorted by EntityId ordering for determinism (HashMap iteration order is random; never leak it). Frustum query: iterate cells in the frustum's bounding AABB then cell-AABB vs frustum test; if the frustum bounding box covers more cells than entities stored, fall back to iterating all entities instead (far-plane 1e6 m would otherwise enumerate astronomically many cells).
- Cell key: `(x / cell).floor() as i64`; input is finite (WorldPos guarantee; validate Aabb/Frustum finite in constructors). Clamp to avoid i64 saturation surprises.
- Frustum constructor from primitives: `Frustum::from_perspective(position: DVec3, rotation: DQuat, fov_y: f64, aspect: f64, near: f64, far: f64)`; reverse-Z or not is irrelevant to planes. Document -Z forward / +Y up per the engine's camera convention (check engine-render `camera.rs` convention before fixing it; add a test against the same `Camera` parameters in Phase 5).
- Brute-force oracle test: for N pseudo-random entities, compare every query result against a linear scan (the strongest correctness test).

### Pattern 4: AssetId and Handle<T>
```rust
pub struct AssetId(uuid::Uuid);   // from_key(&str) = Uuid::new_v5(&ASSET_NAMESPACE, key.as_bytes())
// Handle<T> wraps slotmap::DefaultKey; PhantomData<fn() -> T> so Handle is Send+Sync and covariant without T: Clone
pub struct Handle<T> { key: slotmap::DefaultKey, _t: PhantomData<fn() -> T> }
// implement Clone, Copy, PartialEq, Eq, Hash, Debug manually (derive would add T: Trait bounds)
```
- `ASSET_NAMESPACE` is a fixed hard-coded `Uuid::from_u128(...)` constant (never `NAMESPACE_URL` etc. so engine IDs are distinct); add a golden-value test pinning a known key to a known uuid string so the stability guarantee is checked. Normalize keys (lowercase, `/` separators, trim) in one function and test it.
- `AssetRegistry<T>`: `SlotMap<DefaultKey, Entry<T>>` + `HashMap<AssetId, Handle<T>>`. `register(id) -> Handle<T>` idempotent for same id; `get(handle) -> Option<&T>`; `unregister` bumps generation so old handles go stale (test: stale handle returns None, same AssetId re-registered gets a different handle).
- A residency manager needs type-erased state across asset types: key the manager on `AssetId`/untyped `RawHandle` (the slot key) and keep `Handle<T>` as the typed facade; or make the manager per-class. Recommended: `ResidencyManager` stores records keyed by `AssetId` and exposes handle-typed convenience methods; handles convert to AssetId via the registry.

### Pattern 5: Residency state machine and budgets
- `enum ResidencyState { Unloaded, Requested, Generating, CpuReady, Uploading, GpuResident, Evicting, Failed }` with `fn can_transition(self, to: Self) -> bool` implemented as an exhaustive `match` over `(from, to)` (no wildcard `_ => true`), and `fn transition(&mut self, to) -> Result<(), ResidencyError::IllegalTransition{from,to}>`.
- Proposed legal edges (planner may adjust, but pin in a test as the full 8x8 matrix): Unloaded->Requested; Requested->Generating|Failed|Unloaded(cancel); Generating->CpuReady|Failed|Unloaded(cancel); CpuReady->Uploading|Evicting|Failed; Uploading->GpuResident|Failed|CpuReady(upload aborted); GpuResident->Evicting; Evicting->Unloaded; Failed->Requested(retry)|Unloaded. Everything else (including self-transitions) is illegal.
- `ResidencyManager { classes: per-class {cpu_budget, gpu_budget, cpu_used, gpu_used}, upload_cap_bytes, uploaded_this_frame, frame, records }`. `AssetClass` is a small enum or `u8` newtype (Claude's discretion).
- `begin_frame(frame_index: u64)` resets `uploaded_this_frame`. `try_begin_upload(id, bytes)`: legal only from CpuReady; fails with `UploadCapExceeded` when `uploaded_this_frame + bytes > cap` (rule for a single asset larger than the cap: allow if `uploaded_this_frame == 0` so large assets cannot starve forever, and document/test it); fails with `OverBudget` when gpu_used + bytes > gpu_budget, returning no state change. Counters update only on success (test atomicity: failed call leaves state and counters untouched).
- CPU budget gates `Requested -> Generating`/CpuReady byte reservation; GPU budget gates `Uploading`. Accounting must be released on Evicting->Unloaded and on Failed (leak test: after random walk of transitions, used == sum over resident records).
- `touch(id)` records `last_used_frame`; `eviction_candidates(class, bytes_needed) -> Vec<AssetId>` returns GpuResident (and optionally CpuReady) assets of that class ordered by least `last_used_frame` (tie-break by AssetId for determinism) until freed bytes >= needed. It never mutates state; caller drives `begin_evict`. Exclude assets in-flight (Requested/Generating/Uploading) and optionally pinned ones.
- Observability: `ResidencyStats { per-class cpu_used, gpu_used, uploaded_this_frame, counts per state, denied_uploads }` returned by a `stats()` method; tests assert on it. Do not rely on log output.
- Synthetic loader with `engine-jobs`: `JobSystem::submit(Priority, FnOnce(&CancelToken) + Send + 'static) -> JobHandle`; the job gets no return value, so deliver results through an `Arc<Mutex<..>>`/`std::sync::mpsc` channel the manager polls in `poll_completions()` (assets lacks crossbeam-channel; use std mpsc, no new dep). Job checks `token.is_cancelled()` and the manager maps `JobStatus::Cancelled` -> Unloaded and `Panicked` -> Failed. Tests: use `JobSystem::builder().workers(1)`/`start_paused(true)` to make ordering deterministic; wait with `JobHandle::wait()`. Never `sleep` in tests.

### Anti-Patterns to Avoid
- Wildcard `_ =>` in the transition match (silently legalizes new states).
- Using `Instant::now()` for LRU or caps (non-deterministic tests); pass frame indices.
- Leaking HashMap iteration order into query results.
- Recursive despawn/dirty propagation on unbounded depth.
- `derive(Clone, Copy, PartialEq)` on `Handle<T>` (adds `T:` bounds).
- Making `engine-spatial` or `engine-render` depend on each other (same tier; xtask R5 rejects).
- Letting `slotmap` types appear in public signatures other than via the newtype'd IDs (keeps storage a two-way door).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Generational IDs | custom index+generation arena | `slotmap` | Tested stale-key semantics, iteration, no unsafe (project forbids unsafe) |
| Name-based UUIDs | custom hash to 128 bits | `uuid` v5 | Standardized, stable across releases |
| Origin rebasing | own shifting code | `engine_math::rebase` + `PositionStore` | Already verified at 6.4e6 m |
| Frame conversion | manual matrix chains | `FrameRegistry::convert/delta` | LCA logic exists |
| Job scheduling/cancel | own thread pool | `engine-jobs` | Phase 2 deliverable |

**Key insight:** The risky logic here is invariants (stale IDs, cycles, budget accounting), not algorithms. Use libraries for storage and spend effort on invariant tests.

## Runtime State Inventory

Not applicable (greenfield phase; no rename/migration).

## Common Pitfalls

### Pitfall 1: Hierarchy cycles and orphaned children
**What goes wrong:** `set_parent(a, b)` where b is a descendant of a creates an unreachable cycle; dirty propagation or despawn loops forever.
**How to avoid:** ancestor walk before linking; iterative traversals; test cycle, self-parent, stale parent, reparent-keeps-consistent `children` lists.

### Pitfall 2: Stale dirty cache
**What goes wrong:** Changing a parent transform leaves descendants' cached world transforms stale; reparenting does not mark the subtree dirty.
**How to avoid:** mark subtree dirty on `set_local`, `set_parent`, and `rebase`; test each, plus "second update with no changes does zero recomputation" (expose a recompute counter for tests).

### Pitfall 3: Precision regression in composition
**What goes wrong:** Converting to f32 early or composing through `Mat4` loses the f64 guarantee.
**How to avoid:** translation accumulates in `DVec3`; only rotation/scale are f32. Add a test with a 6.4e6 m root and a 1 mm child offset that must survive (delta exact to 1e-9 m).

### Pitfall 4: Spatial index staleness across rebase
**What goes wrong:** Index frame locals change when that frame is rebased; the grid then holds wrong coordinates.
**How to avoid:** choose an index frame that is not rebased, or provide a `rebuild` path; document in the ADR; test insert, rebase, rebuild/requery gives identical results.

### Pitfall 5: Grid blowups
**What goes wrong:** Large AABBs span millions of cells; huge frustum far planes enumerate empty cells.
**How to avoid:** oversize list and the "iterate entities instead of cells when cheaper" fallback; test with a 1e7 m far plane and ensure the test finishes quickly.

### Pitfall 6: Dedup and ordering in queries
**What goes wrong:** Entities in several cells appear twice; unordered results make tests flaky.
**How to avoid:** sort+dedup output; compare against brute-force oracle as sorted sets.

### Pitfall 7: Budget accounting drift and starvation
**What goes wrong:** Failed/cancelled loads do not release reserved bytes; an asset bigger than the per-frame cap never uploads.
**How to avoid:** single place that mutates counters, invariant checker `debug_assert_consistent()` used in tests; the oversize-asset rule above.

### Pitfall 8: Job result races
**What goes wrong:** Job finishes after cancel or after the asset was evicted/unregistered; completion applied to a reused slot.
**How to avoid:** completions carry the Handle/AssetId + a request generation (monotonic u64); ignore completions whose generation does not match the record.

### Pitfall 9: Clippy/`result_large_err`
`engine-core` and `engine-sdk` allow `result_large_err` because `EngineError` is large. World/spatial/assets define their own small error enums, so no allow is needed; if they wrap `EngineError`, copy the `#![allow(clippy::result_large_err)]`. [VERIFIED: grep]

## Code Examples

### Typed handle without derive bounds
```rust
// Source: standard Rust pattern (PhantomData<fn() -> T>)
impl<T> Clone for Handle<T> { fn clone(&self) -> Self { *self } }
impl<T> Copy for Handle<T> {}
impl<T> PartialEq for Handle<T> { fn eq(&self, o: &Self) -> bool { self.key == o.key } }
impl<T> Eq for Handle<T> {}
impl<T> std::hash::Hash for Handle<T> { fn hash<H: std::hash::Hasher>(&self, h: &mut H) { self.key.hash(h) } }
```

### Stable asset ID
```rust
// Source: docs.rs/uuid (Uuid::new_v5), feature "v5"
const ASSET_NAMESPACE: uuid::Uuid = uuid::Uuid::from_u128(0x6f3c_1a52_8d4e_4b07_9c21_5e7a_b3d0_41f8); // fixed forever
pub fn from_key(key: &str) -> AssetId { AssetId(uuid::Uuid::new_v5(&ASSET_NAMESPACE, normalize(key).as_bytes())) }
```

### Synthetic loader through engine-jobs
```rust
let (tx, rx) = std::sync::mpsc::channel();
let h = jobs.submit(Priority::Normal, move |tok| { if tok.is_cancelled() { return; } let _ = tx.send((id, generation, vec![0u8; size])); });
// manager.poll_completions() drains rx.try_recv() and applies Generating -> CpuReady
```
`std::sync::mpsc::Sender` is Send; the job closure must be `Send + 'static`. [VERIFIED: engine-jobs scheduler.rs signature]

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| Entity = raw index | Generational keys (slotmap 1.x) | Stale-ID rejection built in |
| Whole-scene f32 transforms | f64 translation + camera-relative f32 at extraction | Already in engine-math/render (Phase 2-3) |

Phase 8 ADR spike will compare BVH/rstar/kiddo against the hash grid; the trait is shaped so a second backend slots in (no `HashGridIndex` types in the trait).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Default cell size 64 m is adequate for a first backend | Pattern 3 | Perf only; tunable, covered by Claude's discretion |
| A2 | Camera convention (-Z forward, +Y up) for `Frustum::from_perspective` matches engine-render `Camera` | Pattern 3 | Culling mismatch in Phase 5; verify against `crates/engine-render/src/camera.rs` during planning |
| A3 | Proposed transition edge list (e.g., Uploading->CpuReady abort) is acceptable | Pattern 5 | Planner/user may want a stricter table; tests pin whatever is chosen |
| A4 | Oversized-asset-vs-upload-cap rule (allow when nothing uploaded yet this frame) is desired | Pattern 5 | Different starvation policy; user may prefer hard rejection |
| A5 | slotmap `DefaultKey`/`new_key_type!` behaviors (Copy/Hash/Ord, null key `default()`) as described | Pattern 1,4 | Minor; confirm in docs.rs when implementing |

## Open Questions (RESOLVED)

1. **Where does Transform/frame-aware transform math live?** RESOLVED: `engine-world` (`Transform`, `WorldTransform`, composition), built on `engine-math` `WorldPos`/`FrameRegistry`/`DVec3`/`DQuat`. No change to engine-math is required (add `glam` as a direct dep of world for `Quat`/`Vec3`, same as engine-render).
2. **How do world and assets connect to render extraction without a new edge?** RESOLVED: `engine-render` (tier 3) already depends on `engine-world` and `engine-assets` (tier 2). Phase 5 adds an extraction function inside `engine-render` that iterates `World` world transforms and looks up meshes in a render-owned `HashMap<EntityId, MeshHandle>` (world does not know about meshes). `engine-spatial` cannot be used from `engine-render` (same tier); culling glue goes in `engine-sdk` or an extension, and `Frustum` is constructible from primitives so no render types are needed.
3. **Do the layer table and deny.toml need changes?** RESOLVED: No. All planned edges satisfy R5; new crates are Zlib/BSD-3/MIT/Apache, already allowed; no wildcard versions; `multiple-versions` is warn-only.
4. **How does rebasing interact with entity positions and the index?** RESOLVED: world owns a `PositionStore` for root positions and exposes `rebase`; index frame must be a non-rebased frame or be rebuilt (Pitfall 4, document in ADR-0006).
5. **Async completion delivery without a return value from `submit`?** RESOLVED: std `mpsc` channel polled by the manager with request-generation checks (Pitfall 8).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | all | yes | cargo 1.99.0 | — |
| crates.io index | slotmap/uuid resolution | yes (cargo info succeeded) | — | vendoring not needed |
| cargo-deny | CI gate | not probed locally | — | CI runs it; run `cargo deny check` if installed |

No GPU or window needed for this phase.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | built-in `cargo test` (libtest), no extra framework; `cargo xtask check-deps` for layering |
| Config file | none needed (workspace `Cargo.toml`); CI runs `cargo test --workspace --locked` on Linux/macOS/Windows |
| Quick run command | `cargo test -p engine-world -p engine-spatial -p engine-assets` |
| Full suite command | `cargo test --workspace --locked && cargo clippy --workspace --all-targets -- -D warnings && cargo xtask check-deps` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WORLD-01 | spawn/despawn, stale id -> None, slot reuse yields different id | unit | `cargo test -p engine-world entity` | Wave 0 |
| WORLD-01 | recursive despawn kills descendants; cycle/self-parent/stale-parent rejected | unit | `cargo test -p engine-world hierarchy` | Wave 0 |
| WORLD-01 | child world transform = parent o local; dirty propagation; no recompute when clean | unit | `cargo test -p engine-world transform` | Wave 0 |
| WORLD-01 | 6.4e6 m root + 1 mm child precision; rebase keeps world position (1e-8 m) | integration | `cargo test -p engine-world --test precision` | Wave 0 |
| WORLD-05 | each query vs brute-force oracle over pseudo-random entities (frustum, region, radius) | unit/integration | `cargo test -p engine-spatial --test oracle` | Wave 0 |
| WORLD-05 | insert/update/remove, duplicates, absent update, dedup + sorted output, oversize entities | unit | `cargo test -p engine-spatial hash_grid` | Wave 0 |
| WORLD-05 | trait object usable (`Box<dyn SpatialIndex>`) with second trivial test backend | unit | `cargo test -p engine-spatial trait_object` | Wave 0 |
| ASSET-01 | AssetId golden uuid value; same key same id; key normalization | unit | `cargo test -p engine-assets id` | Wave 0 |
| ASSET-01 | Handle<T> typed (compile-fail not required), stale handle after unregister -> None | unit | `cargo test -p engine-assets handle` | Wave 0 |
| ASSET-02 | exhaustive 8x8 transition matrix; illegal -> Err | unit | `cargo test -p engine-assets residency_state` | Wave 0 |
| ASSET-02 | synthetic job loader drives Requested->Generating->CpuReady; cancel -> Unloaded; panic -> Failed | integration | `cargo test -p engine-assets --test loader` | Wave 0 |
| ASSET-03 | cpu/gpu budgets reject over-budget; per-frame upload cap enforced and reset by begin_frame; failed call leaves counters unchanged | unit | `cargo test -p engine-assets budget` | Wave 0 |
| ASSET-03 | LRU candidates ordered by last_used then AssetId; in-flight excluded; accounting invariant after random walk | unit | `cargo test -p engine-assets eviction` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p <crate touched>`
- **Per wave merge:** quick run command above plus `cargo xtask check-deps`
- **Phase gate:** full suite green (incl. CI three-OS) before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] Add `slotmap`, `uuid` to `[workspace.dependencies]` and crate manifests (also run `cargo deny check` if available)
- [ ] Test files listed above (none exist; stubs are 4-line `lib.rs`)
- [ ] A tiny deterministic PRNG helper (LCG/xorshift, `#[cfg(test)]`) for oracle tests, duplicated per crate or placed in a `tests/common` module; no proptest needed
- [ ] ADR-0006 (spatial backend "hash grid first, benchmark later", feeds WS-05); update `docs/architecture/adr/README.md` index

## Security Domain

ASVS applicability is minimal (no network, auth, or persistence in this phase).

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2/V3/V4 Auth, Session, Access | no | — |
| V5 Input Validation | yes | Reject non-finite transforms/volumes, validate hierarchy ops, bound sizes (error enums, never panic) |
| V6 Cryptography | no | uuid v5 uses SHA-1 for naming only, not security; do not treat AssetId as a secret/integrity hash |

| Threat Pattern | STRIDE | Mitigation |
|----------------|--------|------------|
| Stale/forged ID use after despawn | Tampering | Generational keys; all APIs return Option/Err |
| Resource exhaustion via huge bounds / deep hierarchy | DoS | Iterative traversals, oversize list, cell-span cap, budget checks |
| Integer overflow in byte accounting | Tampering | `checked_add`/`saturating_sub`; u64 bytes; tests at near-`u64::MAX` |
| Panics from untrusted input | DoS | No `unwrap`/indexing on user-supplied ids |

## Sources

### Primary (HIGH confidence)
- Repo: `xtask/src/layers.rs`, `deny.toml`, root and per-crate `Cargo.toml`, `docs/architecture/dependency-direction.md`, `docs/architecture/adr/0004-worldpos-frames-rebasing.md`
- Repo: `engine-math` (`WorldPos`, `FrameRegistry`, `rebase`, `PositionStore`), `engine-jobs` scheduler API, `engine-render` `scene.rs` (`RenderItem`, `extract`)
- `cargo info` / `cargo search` (2026-11-25): slotmap 1.1.1 (Zlib), uuid 1.29.3 (Apache-2.0 OR MIT, `v5 = ["sha1"]`), sha1_smol 1.0.1 (BSD-3-Clause), proptest 1.12.0

### Secondary / Tertiary
- Slotmap/glam behaviors beyond what was read are from training knowledge (see A5); not re-fetched from docs.rs this session.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH, versions and licenses verified against the registry
- Architecture / crate edges: HIGH, verified against layer table and manifests
- Pitfalls: MEDIUM-HIGH, standard engineering knowledge
- Transition table specifics: MEDIUM (design choice, flagged A3)

**Research date:** 2026-11-25
**Valid until:** 2026-12-25
