# Phase 7: Planetary Extension & Terrain LOD - Research

**Researched:** 2026-11-25
**Domain:** Cube-sphere terrain LOD (topology, restricted quadtree, skirts), deterministic planet-space generation, extension-side streaming/render glue on the existing SDK
**Confidence:** HIGH on topology/crate wiring/CI/SDK gaps (verified in repo and by script); MEDIUM on LOD/skirt tuning constants (must be tuned and recorded in the ADR with measurements)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
**Topology**
- Planetary code stays in `extensions/planetary/*`: `planetary-core` (math/topology/LOD, GPU-free) plus new crates for the terrain render pass/camera and the planetary app/example. New crates need xtask layer-table, dependency-direction doc, workspace and canary-job consistency (planetary crates must remain removable from the workspace without breaking the conventional build).
- Cube-sphere with the tangent-adjusted (equal-angle) mapping, f64 throughout; per-face patch grids with a tested adjacency table across all 12 edges and 8 corners (reciprocity tests).
- Patch ID: one packed `u64` = face (3 bits) + level (5 bits) + interleaved x/y morton bits; parent, children and neighbors derived from the ID, not stored.

**LOD**
- Quadtree refined by screen-space error with a pixel threshold, hysteresis band (no popping), and 2:1 balancing (neighbors differ by at most 1 level).
- Planetary patches implement Phase 6 generic `StreamKey` and `Generator`; the coarse parent shows until the child is resident (pinned fallback).
- Terrain from layered planet-space generators (base shape + detail + mask) sampled in unit-sphere space so shared borders agree exactly (no per-patch local noise).

**Crack-free technique (user decision)**
- Skirts + shared-border sampling: border vertices come from the same planet-space function so same-level neighbors match exactly; a small downward skirt hides T-junction gaps between different levels. 2:1 balance keeps skirts small. Geomorphing is a possible later improvement. Record as an ADR (next free number) with prototype evidence (WS-05 LOD technique decision).

**Rendering & app**
- Each patch has its own f64 origin; vertices stored as f32 offsets from it; uses existing camera-relative extraction.
- Planetary camera: orbit -> surface navigation with altitude-scaled speed, SDK input only; no gravity/walking.
- Atmosphere is deferred to Phase 8 (PLAN-08, first to cut).

**Verification**
- CPU-side seam proof: sample every shared boundary vertex from both patches and assert equality (zero gaps) incl. cube-face edges and corners; GPU readback optional.
- Scripted orbit-to-surface descent on a simulated clock asserts counted per-frame work caps (patches started/applied/uploaded), no wall-clock.
- Planetary app uses the public SDK only (like Phase 5's `sdk_only` policy test); manual window run is a human-verification item.

### Claude's Discretion
Crate names, patch grid resolution, skirt depth, error thresholds, ADR number, module layout, test structure, planet radius/seeds for the demo.

### Deferred Ideas (OUT OF SCOPE)
Geomorphing, atmosphere (Phase 8), software-adapter image-diff tests (Phase 8), gravity/walking, geospatial data ingestion, persistent edits.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PLAN-02 | Cube-sphere with stable packed patch IDs and tested face-adjacency table | Sections "Patch ID bit budget", "Cube-face adjacency" (derived table validated by script: 32,760 patch-edge checks, 0 failures) |
| PLAN-03 | Quadtree, 2:1 balance, SSE LOD with hysteresis | Section "LOD selection" (SSE formula, hysteresis state, grandparent-split balance algorithm across faces) |
| PLAN-04 | Crack-free across LOD, face edges and corners | Sections "Crack-free technique", "Seam proof tests"; effective-cut balancing under streaming fallback (critical finding) |
| PLAN-05 | Layered planet-space generators, borders agree | Section "Planet-space generation" (exact-op tan, canonical xyz sum order, global integer lattice) |
| PLAN-06 | Planetary camera orbit -> surface | Section "Planetary camera" |
| PLAN-07 | Scripted descent holds frame budget without stalls | Section "Scripted descent test" (counted caps on simulated clock) |
| APP-02 | Planetary app on public SDK only | Sections "Crate topology", "SDK gaps"; `sdk_only` policy test clone |
</phase_requirements>

## Summary

The topology, LOD and generation math is fully specifiable with existing Phase 6 primitives and no new third-party dependencies. The cube-face adjacency can be **derived from six integer (normal,u,v) basis triples** instead of a hand-typed 24-entry table. I implemented the derivation in a throwaway script and verified reciprocity plus an independent integer-geometry oracle (shared-edge corner lattice points identical from both sides) for every patch and edge at levels 0-5: 32,760 checks, 0 failures. The Rust implementation should be a direct port of that logic, and its tests should include both the reciprocity check and the geometry oracle, exhaustively up to level 6.

Three findings change the plan shape and are not in CONTEXT.md. (1) **`Resources` (GPU buffer creation) exists only during `App::setup`** (ADR-0007 decision 7, verified in `crates/engine-sdk/src/app.rs` / `runtime.rs`), so streamed terrain cannot upload or free meshes at runtime without a small, generic SDK addition (a default-no-op per-frame GPU hook plus `Resources::destroy_mesh`). This must be a first plan, recorded in the ADR, and contains no planetary concept. (2) **Streaming fallback breaks the 2:1 guarantee** for the *drawn* set (a wanted leaf may resolve to a much coarser resident ancestor next to a fine neighbor), which would exceed the skirt bound and reopen cracks. The fix is a second balancing stage on the *effective* (residency-limited) cut that coarsens rather than splits. (3) **`tan` is banned by ADR-0008**, and equal-angle mapping needs it; use an exact-op sin/cos Taylor implementation (verified: max error 2.2e-16 vs libm, monotone, odd, exact +-1 at the face edge by special case).

Crack-freedom rests on: shared border vertices bit-identical in f64 (global integer lattice index -> dyadic `s` -> exact-op `tan` -> canonical xyz-order `sqrt` normalisation -> level-independent height function), plus skirts whose depth is bounded by the *parent-level* geometric error (valid precisely because 2:1 balance limits the neighbor level difference to 1). Seam proofs are CPU-only: bit-equality of shared border vertices (same level) and a measured T-junction gap bound <= skirt depth (mixed level).

**Primary recommendation:** Build in this order: (A) SDK hook + `destroy_mesh` + xtask/docs/CI wiring for 2 new crates; (B) `planetary-core` pure math: PatchId, adjacency, mapping, generator, mesh builder, LOD select + balance, with exhaustive tests; (C) `planetary-terrain` streaming/upload/draw glue + camera; (D) `planetary-app` + descent test + ADR-0009.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| PatchId, adjacency, mapping | planetary-core (pure f64/integer) | - | GPU-free, exhaustively testable, no engine tier needed |
| Height/biome generation (layers) | planetary-core (`Generator` impls via `engine_sdk::procedural`) | engine-procedural (core, generic) | Pure functions of planet-space position; core stays planet-agnostic |
| Patch mesh build (f32 offsets, skirts, indices) | planetary-core | - | CPU only; runs on worker threads inside `StreamSource::produce` |
| LOD select, hysteresis, 2:1 balance, effective cut | planetary-core | - | Pure function of (view, prev state, residency predicate) |
| Stream key/source/uploader, GPU mesh lifecycle, draw list | planetary-terrain | engine-streaming, engine-sdk (`Resources`) | Needs SDK `Resources` and `StreamManager`; keeps core pure |
| Planetary camera | planetary-terrain (uses SDK `Camera`/`InputEvent`) | - | Must not use conventional-3d `FlyCamera` (R4 cross-family ban) |
| App, scripted descent, example | planetary-app | - | `App` impl + `sdk_only` policy test (APP-02) |
| Per-frame GPU hook, mesh destroy | engine-sdk (generic, no planet concepts) | engine-render backend | Only place `Resources` can be exposed beyond setup |
| Rendering of patches | engine-render/-wgpu unchanged (`RenderItem` + lit pipeline) | - | Geometry-agnostic extraction (Phase 5) |

## Project Constraints (from CLAUDE.md)

- Core must not depend on planetary topology/terrain; planetary code only in the planetary extension (HLD 5.1 / 22.4). Phase constraint: rendering must never block on generation (async streaming).
- Simplicity: no premature abstractions, no ECS/physics/scripting; record assumptions/limitations/ADRs as work proceeds.
- Pinned toolchain 1.99.0 (verified `rustc 1.99.0`), edition 2024, `unsafe_code = forbid` (workspace lint), clippy `-D warnings` in CI.
- Disk is tight (C: shows ~12 GB free, `target/` is 5.8 GB): add **no new third-party dependencies**, no new targets; cross-target use `cargo check` only.
- Global user instruction: `graphify` skill trigger only on `/graphify` (not relevant here).

## Standard Stack

No new third-party crates. Everything is built from workspace crates reached through `engine-sdk` (re-exports verified in `crates/engine-sdk/src/lib.rs`).

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `engine-sdk` | workspace | Only dependency of planetary crates; re-exports `procedural`, `streaming`, `JobSystem`, `Camera`, `RenderItem`, `VertexPosNormalUv`, `Resources`, `DVec3`, `DQuat`, `WorldPos` | Layer rule: extensions reach the engine through the SDK `[VERIFIED: dependency-direction.md, planetary-core/Cargo.toml]` |
| `engine_sdk::procedural` | HASH_VERSION=1 | `Generator`, `LayerStack`, `Params`, `StableId`, `value_noise_3d`, `fbm_3d`, `splitmix64`, `layer_seed`, `unit_f64`, `Fnv1a64` | ADR-0008 exact-op noise `[VERIFIED: crates/engine-procedural/src]` |
| `engine_sdk::streaming` | - | `StreamManager`, `StreamKey`, `StreamSource`, `Uploader`, `StreamRequest`, `StreamConfig`, `FrameReport`, `ResidencyManager`, `ClassBudget`, `AssetClass` | Generic streaming w/ pinned parent fallback `[VERIFIED: crates/engine-streaming/src]` |
| `glam` via SDK (`DVec3`, `DQuat`, `Quat`, `Vec3`) | 0.34.3 | f64 math, f64->f32 quat cast (`DQuat::as_quat`) | Already in tree |
| `thiserror` | 2.0.21 (workspace dep) | Error enums | Already used by conventional-3d |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Exact-op sin/cos Taylor for equal-angle `tan` | libm `f64::tan` | Banned by ADR-0008 decision 8 (platform-dependent last bit would break border bit-equality across OSes) |
| `noise` crate etc. | - | Rejected in ADR-0008 |
| Planetary crates depending on `engine-procedural`/`engine-streaming` directly | via `engine-sdk` re-exports | Layer table allows lower tiers, but the project convention ("Extensions depend on the engine through `engine-sdk`") and APP-02 require SDK-only |
| Geomorphing | skirts | Locked: skirts; geomorph deferred |
| `uuid` direct dep for cheap `AssetId` from a `u64` pair | `AssetId::from_key(&str)` with the id cached in the key | Start with zero deps (cache the `AssetId` in the key); only add `uuid` if profiling shows SHA-1 cost in `resolve` |

**Installation:** none. **Version verification:** versions are the workspace pins in `Cargo.toml` (read this session); no registry lookup needed since nothing new is added.

## Architecture Patterns

### Crate topology (answers item 6)

```
extensions/planetary/
  planetary-core/      tier 6  (existing stub; dep: engine-sdk)
    src/{lib, patch_id, adjacency, mapping, tan, height(layers), mesh, lod/{select,balance,effective}, view}.rs
  planetary-terrain/   tier 7  (NEW; deps: engine-sdk, planetary-core via path)
    src/{lib, key (PatchKey: StreamKey), source (PatchSource: StreamSource), uploader, terrain (TerrainSystem), camera}.rs
  planetary-app/       tier 8  (NEW; deps: engine-sdk, planetary-core, planetary-terrain via path)
    src/{lib (PlanetaryApp: App), descent (DescentScript)}.rs ; examples/planetary_flight.rs ; tests/{sdk_only,descent}.rs
```

Required changes (each verified against the repo):

| File | Change |
|------|--------|
| root `Cargo.toml` `[workspace] members` | Add `"extensions/planetary/planetary-terrain",` and `"extensions/planetary/planetary-app",` **each on its own line containing the literal `extensions/planetary/`**. The CI canary job runs `sed -i '/extensions\/planetary\//d' Cargo.toml` then fails if `planetary` still occurs in `Cargo.toml`. |
| root `Cargo.toml` `[workspace.dependencies]` | **Do NOT add** planetary entries. Use `planetary-core = { path = "../planetary-core" }` inside the planetary crates' own manifests (deleted with the directory). Any `planetary` text left in root `Cargo.toml` fails the CI job. |
| `xtask/src/layers.rs` LAYERS | Add `planetary-terrain` (tier 7) and `planetary-app` (tier 8). R5 (`evaluate`) already waives the tier check within the same extension family (`same_family`), and R4 only bans *cross*-family deps, so intra-planetary deps pass; tiers 7/8 are for readability. R6 `unlisted-crate` fails if a package is missing from LAYERS. |
| `docs/architecture/dependency-direction.md` | Add table rows and mermaid edges for both crates. `docs_cover_all_crates` in `xtask/tests/docs_policy.rs` requires every workspace package name in backticks. |
| `.github/workflows/ci.yml` build job step "Test without planetary-core (fast canary)" | Currently `--exclude planetary-core`. With new crates that still compile planetary code, either extend to `--exclude planetary-core --exclude planetary-terrain --exclude planetary-app` or drop the step (ADR-0007 already calls it a cheap extra). |
| canary job | No change needed: it deletes the whole `extensions/planetary` dir and builds/tests with `--exclude xtask` (xtask layer-table policy lists planetary crates by design). Verify locally by simulating (copy repo is too big; do the sed + `cargo metadata` check only, or run `cargo xtask check-canary` which already covers reachability). |
| `xtask` canary (`check-canary`) | No code change: `is_planetary` already matches anything under `extensions/planetary/` or named `planetary-*`. `conventional-3d` must never gain a planetary dep (R4 + canary both enforce; the planetary crates must **not** import `conventional-3d`, which also means no `FlyCamera` reuse). |
| `xtask/tests/*` | `canary.rs` fixtures unaffected. `fixtures_negative.rs` unaffected. |
| `deny.toml` | No change (no new third-party crates). |
| `engine-sdk` | Add generic GPU hook (see "SDK gaps"). |

### System architecture diagram

```
input events ──► PlanetaryApp::input ──► PlanetaryCamera (f64 pos, parallel-transported orientation)
                                              │ per frame
                                              ▼
 PlanetaryApp::update(dt) ──► camera update ─► View{eye, basis, proj_scale, tan_half_fov, aspect}
                                              │
                                              ▼
          planetary-core::lod::select(view, prev_split_set, params)         [pure, counted node visits]
              SSE + hysteresis ► wanted split set ► 2:1 balance (grandparent-split worklist, cross-face)
                                              │ wanted leaves (balanced)
                         ┌────────────────────┴───────────────────────┐
                         ▼                                            ▼
        TerrainSystem::request_missing                  lod::effective_cut(wanted, is_resident)
        (parent-first, cancel stale tokens,              top-down: split only if wanted AND all 4
         priority by SSE/level)                          children resident; then coarsen-balance
                         │                                            │ drawn leaves (balanced, resident)
                         ▼                                            ▼
        StreamManager::request ──► worker JobSystem        manager.resolve(leaf) (touch/shown)
          StreamSource::produce(PatchKey)                    build Vec<RenderItem> (≤ cap) into scene.items
           = planetary-core::mesh::build_patch(f64 gen)
           → bytes (header + f32 verts + u32 idx)
                         │ CpuReady
                         ▼
 App::gpu hook (NEW in SDK, has &mut Resources) ──► StreamManager::update(jobs, frame, &mut GpuUploader)
      GpuUploader::upload(key, bytes) → Resources::create_lit_mesh → held: BTreeMap<PatchId, MeshHandle>
      reconcile: any held key whose residency != GpuResident → Resources::destroy_mesh (NEW)
                                              │
                                              ▼
                  Runtime::frame: extract (camera-relative f64→f32) → graph → backend.submit
```

### Pattern 1: Patch ID and bit budget (answers item 2)

Layout (matches locked decision): bits 63..61 face (0..5), 60..56 level, bits 55..0 morton(x,y) with x in even bit positions, y in odd, only the low `2*level` bits used (others zero). 56 morton bits allow level <= 28; the 5-bit level field allows 31, so define `MAX_LEVEL = 24` as a hard cap (lattice `G = N * 2^L <= 32 * 2^24 = 2^29`, so every dyadic `s = (2g - G)/G` stays exactly representable and `|coord| << 2^52` for noise).

Resolution arithmetic `[VERIFIED: computed]`: face arc = R*pi/2 = 1.005e7 m (R=6.4e6). With N=32 cells per patch edge: cell(L) = 1.005e7 / (32 * 2^L) m (center of face; equal-angle varies by a factor of order 1.3 across a face `[ASSUMED]`). L=18: 1.2 m, **L=19: 0.60 m**, L=20: 0.30 m. 1 m resolution needs L=19 (20 levels, 0..19). Recommend `MAX_LOD_LEVEL = 19` for the demo (`maximum_lod: 20` in the HLD sample is within the format). Derived ops are pure bit math: `parent`, `children[4]`, `from_face_xy_level`, `to_face_xy_level`; `from_raw` must validate (face < 6, level <= MAX_LEVEL, no stray morton bits) and return `Result`.

### Pattern 2: Cube-face adjacency derived from basis triples (answers item 1)

Face basis (integer vectors, `u x v = n`, so triangles wound CCW in (u,v) are CCW seen from outside, matching `FrontFace::Ccw` + back-face culling in the lit pipeline `[VERIFIED: pipeline.rs:256]`):

| Face | n | u | v |
|------|---|---|---|
| 0 +X | (1,0,0) | (0,1,0) | (0,0,1) |
| 1 -X | (-1,0,0) | (0,0,1) | (0,1,0) |
| 2 +Y | (0,1,0) | (0,0,1) | (1,0,0) |
| 3 -Y | (0,-1,0) | (1,0,0) | (0,0,1) |
| 4 +Z | (0,0,1) | (1,0,0) | (0,1,0) |
| 5 -Z | (0,0,-1) | (0,1,0) | (1,0,0) |

Edges: 0 = -u (x=0), 1 = +u (x=K-1), 2 = -v (y=0), 3 = +v (y=K-1); outward vector = -u, +u, -v, +v. Neighbor of patch `(f, L, x, y)` across edge `e` (`K = 2^L`):
1. If not on the face boundary: plain +-1 step, neighbor edge is the opposite edge.
2. Else `g` = face whose normal equals `out(f,e)`. The edge of `g` that faces back is the one whose outward vector equals **`+n_f`** (a sign slip here -- using `-n_f` -- was the one bug found by the script).
3. Along-edge coordinate `t` = `y` for e in {0,1}, `x` for e in {2,3}. Compare the along-edge axis of `f` with that of `g` (`v_g` if the back edge is 0/1 else `u_g`): dot = +1 keep `t`, dot = -1 flip `t' = K-1-t`.
4. Land on the back edge of `g` at `(0,t')`, `(K-1,t')`, `(t',0)` or `(t',K-1)` accordingly.

Cross-level: `neighbor_same_level(p, e)` always exists (faces tile the sphere). A patch at level L facing a coarser neighbor: neighbor leaf is the ancestor of `neighbor_same_level` at the found leaf level; facing finer: the two children of `neighbor_same_level` adjacent to the shared edge (select by the neighbor edge `e'` and which half; flip already encoded). Only edge neighbors are needed; the 8 cube corners need no special code because the three faces meeting at a corner are pairwise edge-adjacent, so 2:1 balance via edge links covers them (corner vertex is a single shared point, bit-identical by the canonical-order rule below).

**Exhaustive tests (reciprocity):** for L in 0..=6 (6*4^6*4 = 98k lookups, milliseconds): (a) `neighbor(neighbor(p,e).patch, back_edge) == (p, e)`; (b) geometry oracle independent of the table: corner lattice points `K*n + (2(x+ci)-K)*u + (2(y+cj)-K)*v` (pure integers) of the shared edge are identical sets from both sides; (c) at L=0 each face's four neighbors are distinct faces and cover exactly the 4 non-opposite faces; (d) every edge pair is visited exactly 12 * 2^L times per level (counts of cross-face links = 4 * 6 * K / ... assert total cross-face links = 24*K). `[VERIFIED: script at levels 0-5, 0 failures]`.

### Pattern 3: Planet-space generation with bit-exact borders (answers item 5)

Rules that make shared borders agree *bit-exactly* in f64 (each is load-bearing; each needs a test):
1. **Global integer lattice:** vertex `(i,j)` of patch `(f,L,x,y)` has global indices `gu = x*N + i`, `gv = y*N + j` in `[0, G]`, `G = N*2^L` (N a power of two). `s = (2*gu - G) / G` is an exact dyadic double, **independent of level** (a coarse vertex equals fine vertex `2*i`), and the neighbor face's `s` along the shared edge is `s` or `-s` exactly.
2. **Exact-op equal-angle map:** `a = T(s)`, `T(s) = sin(pi/4*s)/cos(pi/4*s)` via Horner Taylor (sin to x^19, cos to x^20) using only `+ - * /`, `FRAC_PI_4` literal; **special-case `s == +-1.0` returning exactly +-1.0** (otherwise the face-edge coordinate is not exactly 1 and the two faces disagree); T is odd by construction (sin odd, cos even). Verified in script: max error 2.2e-16 vs libm, monotone over 2^16 samples, `T(0.5)+T(-0.5)==0`.
3. **Canonical normalisation:** world cube point `p = n + a*u + b*v` (integers times doubles; exact placement of components), then `|p|^2 = px*px + py*py + pz*pz` **always summed in world x,y,z order** (not face-local order), `d = p / sqrt(|p|^2)`. `sqrt` is IEEE-exact (allowed by ADR-0008). This is what makes the same border vertex computed from two different faces identical.
4. **Level-independent height function** `height(d)`: same octave count at every level (a level-dependent octave count would make a coarse vertex differ from the fine vertex at the same position and *open* cracks). 
5. **Layers** (via `engine_sdk::procedural::Generator`): `base` (continent shape, `fbm_3d` on `d * f0`, ~4 octaves), `detail` (own octave loop over `value_noise_3d`, ~8-12 octaves, own gain), `mask` (biome/ocean blend; clamp to sea level using `max`, exact). Each layer's seed = `layer_seed(world_seed, name, version)`. The `Generator` trait is `generate(StableId, seed, params, out)`; for terrain, `StableId(patch_id.0)` identifies the patch and the layer writes `(N+1)^2` heights (`output_len`) -- the *position* of each sample is derived from the id via the lattice above, so the layer remains a pure function of (id, seed, params). Expose also a pure `height_at_dir(stack_seeds, d)` for the camera clearance and tests (single source of truth used by both).
6. **Noise coordinate range:** `value_noise_3d` requires `|coord| < 2^52`; `d*freq` with freq up to ~6.4e6/1 m is ~1e7: safe. Reaching ~1 m wavelength needs ~23 octaves; do not chase it -- value noise is smooth below the last octave and the mesh cell is >= 0.3 m, so 12-16 total octaves are enough `[ASSUMED: visual quality]`.
7. **Banned ops:** reuse the `banned_ops.rs` scan pattern (token scan of `src/` for `.sin( .cos( .tan( .exp( .ln( .powf( .powi( .mul_add( .cbrt( .hypot( .atan2( HashMap DefaultHasher`) in `planetary-core/src` (allow `as f32` there: the cast is IEEE round-to-nearest, deterministic; but keep the f32 cast out of hashed paths). `f64::sqrt`, `floor`, `max/min/abs/clamp` are allowed. Note `FRAC_PI_4` is a literal constant, fine.
8. **Golden hashes:** pin `Fnv1a64` over canonical bits of heights/positions for a fixed set of patches (incl. a face-edge patch and a corner patch) at 1 and N threads; computed on Windows, CI cross-OS confirmation remains pending exactly as in ADR-0008.

### Pattern 4: Patch mesh (answers item 2 mesh resolution, item 6 per-patch origin)

- Grid `N = 32` cells (33x33 vertices), plus a skirt ring: 4 edges * (N+1) extra vertices. Index topology is **identical for all patches** (generate once; skirt quads wound so the outward face points away from the patch, since the lit pipeline back-face culls). ~1,220 vertices (39 KB) + ~3,800 u32 indices (15 KB) = ~55 KB/patch `[ASSUMED: arithmetic from N=32]`. Tests use a smaller N (8) for speed; border agreement does not depend on N provided N is a power of two.
- Origin: `origin = d(center lattice point) * (R + h_center)` computed in f64 (store in the mesh header, 3 x f64). Vertices: `pos_f32 = (P_f64 - origin_f64) as f32`. `RenderItem { mesh, position: WorldPos(origin in root frame), rotation: IDENTITY, scale: ONE, material }`; existing `extract` does `camera_relative_f32` in f64 then casts `[VERIFIED: scene.rs extract]`. Planet center = root-frame origin (no extra frame needed).
- Vertex format `VertexPosNormalUv` (the only lit layout). Colors: `uv = (height01, biome/moisture01)` sampling a small palette texture created in `setup` (`Resources::create_texture`), single shared `Material { base_color: white, base_color_texture: Some(palette) }` added in setup. Avoids needing a vertex-color layout.
- Normals: interior vertices from central differences of the already-computed 3D positions (no extra height evaluations). **Border vertices** (4N of them) use a 4-tap height stencil in a tangent basis derived from `d` alone (face-independent), tap spacing a fixed per-level angle (power-of-two arithmetic), so same-level neighbors, including across cube-face edges, get identical normals (no lighting seam). Cross-level normals differ slightly (cell-size dependent) -- accepted, noted as a known limitation.
- Payload `produce()` returns `Vec<u8>`: little-endian header (version, vertex count, index count, origin f64x3, bound radius f32) + f32 vertices + u32 indices. The uploader must **validate lengths** before decoding and use `Resources::create_lit_mesh` (validates indices).
- GPU memory classes: levels 0-3 (510 patches, ~28 MB) in `AssetClass(BASE)` with a budget >= that so they are never evicted; deeper levels in `AssetClass(DETAIL)` with an LRU budget (e.g. 1500 patches ~ 80 MB `[ASSUMED]`). Set `ResidencyManager::new(upload_cap_bytes)` to ~4 patches/frame.

### Pattern 5: LOD selection (answers item 3)

**Geometric error:** `eps(L) = max(kappa * cell(L), sagitta(L))`, `cell(L) = (R*pi/2)/(N*2^L)`, `sagitta(L) = R*theta^2/8` with `theta = (pi/2)/(N*2^L)`; initial `kappa = 0.25` `[ASSUMED, tune]`. Constants `R`, `pi` literals; no libm. The ADR must contain a measured table (sampled max |true - mesh| per level on the real generator) showing `eps(L)` is an upper bound; add that measurement as a test (a sampling test over a few patches at several levels; assertion `measured <= eps(L)`).

**Screen-space error:** `rho = eps(L) * K / d`, `K = viewport_height_px / (2*tan(fov_y/2))` supplied by the caller as `view.proj_scale` (so selection contains no trig and tests use literal constants), `d = max(|eye - c| - r, d_min)` using the node's bounding sphere `(c, r)` (`c` = center direction * R, `r` = half chord diagonal + max relief `A_max`; sphere radius is inflated by `A_max` so relief cannot escape the bound). Worked numbers `[VERIFIED: arithmetic]`: 1080 px, 60 deg => K = 935; tau = 2 px (HLD sample `target_error_pixels: 2.0`); kappa=0.25 means a node splits when `d < ~117 * cell`, i.e. ~8 px per mesh cell: reasonable. From orbit (d ~1.5e7 m) levels 0-1 split, level 2 stops => ~96 leaves.

**Hysteresis (no popping):** state = previous frame's split set `BTreeSet<PatchId>` (ordered for determinism, not `HashSet`). Node splits if `rho > tau` (was unsplit) and stays split while `rho > tau * (1 - h)` (was split), `h = 0.2` `[ASSUMED, tune]`. Test: oscillate the camera distance +-5% around the split boundary for 100 frames and assert zero split-set toggles; and a monotone approach asserts exactly one toggle per node.

**Traversal:** top-down from the 6 level-0 roots; visits counted (`SelectStats { nodes_visited, leaves }`). Leaf cap guard: if `leaves > max_leaves` (recommend 700, hard render limit `MAX_DRAWS_PER_FRAME = 1024` `[VERIFIED: engine-render backend.rs:158]`, and `validate_packet` rejects more) re-run with `tau *= 1.5` (deterministic) and record `tau_eff` in stats. Optional conservative frustum (bounding sphere vs four side planes, using precomputed `tan_half_fov_y`, `aspect`) and horizon culling; **culling is an optimisation only** -- all partition/balance/crack tests run with culling off; keep culled leaves in the tree for balancing and just skip draws/requests.

**2:1 balancing (restricted quadtree, cross-face):** Let `S` = set of split nodes. A node `p` at level `L >= 2` that exists (its parent is in `S`) requires, for each of its 4 same-level neighbors `q` (via the face adjacency), that `parent(parent(q)) in S`: the leaf covering `q` then has level >= L-1. Worklist: initialise with all existing nodes; for each, check its 4 neighbors; if the grandparent `g` of `q` is not in `S`, insert `g` **and all its missing ancestors**, then enqueue the 4 children of every newly split node. Terminates (S only grows, bounded by levels). Complexity is O(|S| * 4) and tiny. This is the only place cross-face neighbor logic is needed in LOD.

**Effective (drawn) cut under streaming (critical):** the wanted tree is balanced, but if a leaf resolves to a coarser resident ancestor while its neighbor draws fine, level differences can exceed 1 and **skirts no longer bound the gap**. Therefore compute the drawn cut as: (1) `S_eff` = nodes in `S` reachable from the roots such that **all four children are `GpuResident`** (use `manager.state(key)`, which is `&self` and does not touch LRU), (2) then **coarsen to fixpoint**: while any drawn leaf `p` (level L) has an edge-adjacent drawn leaf at level <= L-2, remove `parent(p)` from `S_eff` (merge siblings). Merging only moves toward nodes that are already resident (every node in the effective tree is resident by construction; roots are never evicted because they live in the BASE class). Termination: `S_eff` only shrinks. Result: always a partition (`sum 4^-L == 6`) and always balanced and resident -> skirt guarantee holds every frame, including mid-load. Then call `manager.resolve(&leaf_key)` once per drawn leaf to mark it shown (protects it from LRU eviction).

**Requests (progressive, cancellable):** for each wanted leaf not resident: request the nearest *non-resident ancestor whose parent is resident* (so ancestors always precede descendants), priority from level/SSE (`Priority::High` for coarsest missing, `Normal`/`Low` deeper), `StreamRequest::with_cancel(token)`; keep `BTreeMap<PatchId, CancelToken>`; each frame cancel tokens of requested patches no longer wanted. Treat `StreamError::Duplicate` / `QueueFull` as non-fatal (expected back-pressure). Bound requests issued per frame (counted).

### Pattern 6: Crack-free technique and why 2:1 bounds skirts (answers item 4)

- Same-level neighbors: border vertices are bit-identical in f64 (above) -> zero gap (up to f32 offset quantisation, see Pitfall 4).
- Different levels (coarse L, fine L+1): the coarse edge has vertices at even fine indices; **even fine vertices coincide exactly** with coarse vertices (rule 1, same dyadic `s`). Odd fine vertices lie on the true surface while the coarse edge is a straight chord, so a T-junction gap of size `g` opens at each odd vertex. Skirt: every patch edge gets a wall dropping from each border vertex toward the planet center by `depth(L)`, along `-d`. Coarse-above-fine is covered by the coarse skirt, fine-above-coarse by the fine skirt, so both need `depth >= g`.
- **Why 2:1 matters:** `g` is bounded by the coarser patch's geometric error `eps(L_coarse)` (the finer side is ~ the true surface). With 2:1 the coarse neighbor of a level-L patch is at level >= L-1, so `depth(L) = margin * eps(L-1) = 2*margin*eps(L)` suffices (margin ~1.5). Without balance the required depth would be `eps(L-k)` for unbounded `k`, i.e. large visible walls. With the effective-cut balancing above this holds every frame.
- Quantitative check at coarse levels `[VERIFIED: arithmetic]`: sagitta at L=2 is ~120 m while `kappa*cell` = 1.9e4 m, so relief (not curvature) dominates `eps`; skirt depth tracks `eps`.
- Skirt vertices: same uv/normal as their border vertex, so shading continuity is preserved. No geomorphing (deferred).

### Pattern 7: Seam proof tests (CPU, answers item 4 tests)

1. **Same-level border equality, exhaustive at small L:** for every patch at L<=4 (N=8) and every edge: for i in 0..=N, `position_f64(p, edge vertex i)` equals `position_f64(neighbor, matching vertex i or N-i)` **by `to_bits()`**, including across all 12 cube edges and from all three faces at each of the 8 corners. Matching index uses the adjacency flip, not the lattice (so the test also checks the flip).
2. **Mixed-level gap bound:** build balanced trees from "seam cameras" (see below); for every leaf edge adjacent to two finer children: even fine vertices equal coarse vertices bit-exactly; for odd fine vertices assert 3D distance from the fine vertex to the coarse edge segment is `<= min(depth(coarse), depth(fine))` (a conservative sufficient condition for no see-through). Run with the real height layers so `kappa`/skirt margin are validated by data, and print the measured max ratio gap/depth into the ADR table.
3. **Seam cameras** (deterministic `View`s): (a) one at each of the 8 cube corners (direction (+-1,+-1,+-1), altitude 1 km and 50 m), (b) one at the midpoint of each of the 12 edges, (c) one on a face center, (d) at altitude ~ a LOD boundary found by bisection (camera distance where the SSE of some node equals tau) to exercise L/L+1 junctions, (e) orbit. For each: select -> balance -> assert partition + 2:1 for the *wanted* tree and the *effective* tree with an artificial residency predicate (e.g. only levels <= k resident, and random-but-seeded subsets) to prove the coarsen stage.
4. **Sanity negative:** a unit test that deliberately removes the balance stage (or builds a 3-level jump) makes the gap test fail, proving the test can fail.
5. GPU readback is optional; skip (software-adapter image diff is Phase 8).

### Pattern 8: Planetary camera (answers item 8)

- State: `pos: DVec3` (f64, planet-centered), `orient: DQuat`, speed params. Authoritative in f64; write `scene.camera.position = WorldPos::from_xyz(root, ..)`, `camera.orientation = orient.as_quat()`.
- Orientation without a pole singularity: keep a quaternion and **parallel-transport** it as the camera moves: before/after each move, rotate `orient` by the shortest-arc rotation taking old `up = pos_old/|pos_old|` to new `up`. Mouse look (right button, as in `FlyCamera`) applies yaw about local `up` and pitch about local right, pitch clamped relative to the horizon only when below a threshold altitude (free look above it).
- Altitude: `alt = |pos| - (R + h(dir))` using the pure `height_at_dir`. Speed `v = clamp(alt * k, v_min, v_max)` with `k ~ 0.5 /s`, boost multiplier; at 6.4e6 altitude (orbit) ~3e6 m/s is excessive: clamp `v_max` (e.g. 2e6 m/s); descent from 2 R to 2 m at k=0.5/s takes ~ln(1e7)/0.5 ~ 32 s. Inputs: WASD/arrows planar+forward in view space, E/Space/Q/Ctrl radial up/down, Shift boost, Esc exit; all through `engine_sdk::{InputEvent, KeyCode, MouseButton}` (mirror of FlyCamera bit-set; write own, do not import conventional-3d).
- Clearance: clamp `alt >= min_clear = 2 m + eps(level of drawn patch under camera)` pushing along `up`; needs `locate(dir, level)` (see next bullet) or simply `2 m + eps(MAX_LOD_LEVEL)*?`; use the patch actually drawn to avoid sinking into a coarse fallback mesh.
- `locate(dir) -> PatchId` without `atan`: face = axis with max |component| (ties broken by lowest face index), ratios `a = (d.u)/(d.n)`, `b = (d.v)/(d.n)`, then descend the quadtree comparing `a`, `b` against `T(s_mid)` (the same exact-op `T`), choosing the child at each level.
- Projection: `Camera.z_near` is a public f32; set it from altitude (`clamp(alt*0.01, 0.05, 1.0e4)`), safe because the projection is reverse-Z infinite-far `[VERIFIED: camera.rs]`; fov default 60 deg.
- Camera and view data: `View { eye, forward, right, up, tan_half_fov_y, aspect, proj_scale }` built from the same f64 state (no f32 round trip).

### Pattern 9: SDK gaps (answers item 6, critical)

`Resources` is created only inside `Runtime::setup` (`Resources { backend: &mut self.backend }`) and `App::update(dt, scene)` has no resource access; `SdkError`/facade exports no `destroy` either `[VERIFIED: app.rs, runtime.rs]`. Streaming needs per-frame create and destroy. Minimal generic change (no planetary text anywhere; precedent: Phase 5 extended `Resources`):

```rust
// engine-sdk/src/app.rs (additive, default no-op => conventional-3d / basic_scene unchanged)
pub trait App {
    // ...existing methods...
    /// Called every frame after `update` and before extraction, with GPU resource access.
    fn gpu_frame(&mut self, _resources: &mut Resources<'_>, _scene: &mut SceneState)
        -> Result<(), SdkError> { Ok(()) }
}
impl Resources<'_> {
    /// Destroy the vertex (and index) buffer of a mesh created by this object.
    pub fn destroy_mesh(&mut self, mesh: MeshHandle) -> Result<(), SdkError> { /* destroy_buffer x2 */ }
}
// Runtime::frame: after app.update -> app.gpu_frame(&mut Resources{backend:&mut self.backend}, &mut self.scene)?;
```
Both `NullBackend` (tracks `buffer_count`) and the wgpu backend implement `destroy_buffer` already `[VERIFIED: backend.rs:560]`, giving tests a leak check (`buffer_count` returns to baseline after eviction). Record in ADR-0009. Optional later optimisation: share one index buffer across all patches (would need a new SDK call); not required.

### Pattern 10: Streaming integration specifics

- `PatchKey { planet: u32, id: PatchId, asset: AssetId }`; `Ord` manual on `(planet, id)`; `asset_id()` returns the cached `AssetId::from_key(&format!("planet/{planet:08x}/{id:016x}"))` computed once at construction; `parent()` constructs the parent key (one SHA-1 per ancestor step; bounded because `resolve` is called only for drawn leaves). Root: level 0 -> `None`.
- `PatchSource: StreamSource<PatchKey>`: `estimate_bytes` = constant from N; `produce` checks `cancel.is_cancelled()` between row batches (return `None` when cancelled) then `build_patch`.
- **Eviction is silent** (`make_room` evicts inside `update`; the `Uploader` trait has no eviction callback `[VERIFIED: manager.rs, source.rs]`). `TerrainSystem` keeps `BTreeMap<PatchId, MeshHandle>` and after each `update` (cheaply, only when `report.evicted > 0` or every N frames) destroys any held mesh whose `manager.state(key) != Some(GpuResident)` and drops the draw item. Test with `NullBackend::buffer_count`.
- Upload cap: `Uploader::upload` runs on the frame thread inside `update`; returns `false` to retry. Counted caps come from `StreamConfig { max_pending, max_starts_per_frame, max_completions_per_frame }` plus `ResidencyManager` upload byte cap (`FrameReport.uploads/uploaded_bytes`).
- `JobSystem` is created by the app (SDK re-export `JobSystem::builder().workers(n).build()`), as `ModelLoader` does in conventional-3d. Shut it down in `App`'s drop/`shutdown`.

### Pattern 11: Scripted descent test (answers item 7)

- Simulated clock: `t = frame * (1.0/60.0)`; no `Instant`/`SystemTime`/sleep in test or `planetary-*` src (add a source scan test like Phase 6). Script = pure function `camera_at(t)`: exponential altitude decay from ~2 R to ~3 m over a fixed ~40 s of simulated time with a lateral drift (so patches change continuously), plus optionally a second leg crossing a cube edge/corner.
- Driver: `Runtime::new(Engine::new(..), NullBackend::new())`, `rt.setup(&mut app)`, loop `rt.frame(&mut app, DT)`. In test mode the app calls `StreamManager::settle()` **after** `update` each frame (documented test-only blocking helper, ADR-0008) so results are deterministic across thread timing; production path never settles.
- Per-frame asserts (counted, never wall-clock): `report.started <= max_starts_per_frame`, `completions_applied <= max_completions_per_frame`, `uploads <= upload cap`, `uploaded_bytes <= cap` (except a single first oversize), selection `nodes_visited <= cap`, `leaves <= max_leaves`, `draws <= MAX_DRAWS_PER_FRAME`, requests issued per frame `<= cap`, `pending <= max_pending`, no `StreamError` other than Duplicate/QueueFull, drawn set is a partition (`sum 4^-L == 6` with culling off in the coverage variant) and 2:1 balanced, drawn count never zero after warm-up (roots resident), no frame where a wanted leaf is drawn at level < wanted-1 for more than a bounded number of frames (progress/liveness: eventually exact), and `NullBackend::buffer_count` bounded (no leak).
- Also run the same script twice and assert identical per-frame `(started, uploads, draw leaf set)` sequence (determinism), and once with `max_starts_per_frame = 1` to prove the caps degrade gracefully (coarse fallback, still balanced, still no holes).
- Optional non-asserting wall-clock logging in an `#[ignore]` test for the human perf check.

### Anti-Patterns to Avoid
- **Level-dependent octave count or per-patch local noise coordinates:** opens cracks (borders disagree).
- **Computing `|p|^2` in face-local component order:** same border vertex differs by 1 ulp between faces.
- **Using `f64::tan`/`atan` anywhere in planet-space position code** (ADR-0008 ban).
- **Balancing only the wanted tree** and then drawing `resolve()` fallbacks: invalidates the skirt bound (see effective cut).
- **Reaching for `conventional_3d::FlyCamera`** or any conventional crate: violates R4 and the canary.
- **Adding planetary entries to `[workspace.dependencies]`:** breaks the CI canary job's `grep -q planetary Cargo.toml`.
- **Calling `StreamManager::settle()` on the frame path:** it blocks; test-only.
- **Hand-typing the adjacency table:** derive from basis triples; the oracle test guards it.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async generation, budgets, cancellation, parent pin | A planetary streaming scheduler | `engine_sdk::streaming::StreamManager` | Phase 6 delivered pins, caps, residency states, LRU; duplicating breaks accounting |
| Layer seeding, hashing, noise | New hash/noise | `engine_sdk::procedural` (`layer_seed`, `value_noise_3d`, `fbm_3d`, `Fnv1a64`, `hash_values`) | Bit-exact, golden-pinned |
| Camera-relative f32 | Custom origin rebasing for patches | `engine-render::extract` with per-patch `RenderItem.position` | Already f64 subtract then f32 cast |
| Mesh buffer upload/validation | Direct backend calls | `Resources::create_lit_mesh` (+ new `destroy_mesh`) | Index validation, no leak on failure |
| Job threads | `std::thread` pools | `engine_sdk::JobSystem` | Priorities + cancel tokens |
| Face adjacency table | Literal 24-entry arrays | Derivation from basis triples + oracle test | Error-prone orientation flips |
| `tan` | libm | Local exact-op Taylor `T(s)` (about 15 lines) | The only transcendental needed; no crate can be added for it under determinism rules |

**Key insight:** Everything that makes this phase hard (determinism, cracks, balance) is a *math/topology property*, so test them as properties over exhaustive small cases before any rendering exists.

## Runtime State Inventory

Not applicable: greenfield extension code (no rename/refactor/migration).

## Common Pitfalls

### Pitfall 1: Cube-face edge `+n` vs `-n` sign slip
**What goes wrong:** Neighbor lands on the wrong face and reciprocity fails (I hit exactly this in the first script run: 3,024 failures). **How to avoid:** back edge of `g` has outward vector `+n_f`; run reciprocity + integer-corner oracle at L<=6 in CI. **Warning signs:** reciprocity test failing only at face boundaries.

### Pitfall 2: Face-edge coordinate not exactly +-1
**What goes wrong:** Taylor `T(1.0)` = 0.9999999999999999 or 1.0000000000000002; the two faces' border vertices differ by 1 ulp. **How to avoid:** special-case `|s| == 1`; test `p.to_bits()` equality at all 12 edges and 8 corners.

### Pitfall 3: Fallback breaks the 2:1 invariant
**What goes wrong:** Cracks reappear only while data loads (slow descent, small caps). **How to avoid:** effective-cut coarsening; test with artificial residency predicates and `max_starts_per_frame = 1`.

### Pitfall 4: f32 quantisation across different origins
**What goes wrong:** Same-level neighbors store `pos - origin_A` and `pos - origin_B` in f32; reconstructed world positions differ by up to ~ulp(offset) (e.g. ~2 micrometres for a 19 m patch, ~1 m at level 0 where it is sub-pixel). **How to avoid / test:** exact-equality test is on the f64 positions; the f32 test asserts `|recon_A - recon_B| <= 2 * patch_extent * 2^-23` and that skirts exceed it; do not claim bit equality in f32. `[ASSUMED: sub-pixel claim; verify visually in the manual run]`.

### Pitfall 5: Silent GPU eviction leaks or dangling draws
**What goes wrong:** `StreamManager` evicts residents without telling the uploader; GPU buffers leak, or a draw item points at a destroyed buffer (backend `validate_packet` error). **How to avoid:** reconcile held meshes against `state()` every frame an eviction happened, destroy before building the draw list.

### Pitfall 6: Draw cap exceeded
**What goes wrong:** `MAX_DRAWS_PER_FRAME = 1024` makes the packet invalid and the frame errors. **How to avoid:** leaf cap guard (`max_leaves` 700) and an assertion in tests; balancing forced-splits add leaves, so keep headroom.

### Pitfall 7: AssetId cost in `resolve`
**What goes wrong:** `AssetId::from_key` formats a string and runs SHA-1 (UUID v5) per `asset_id()` call. **How to avoid:** cache the id in the key; only drawn leaves are resolved; measure; if hot, a direct `uuid` dependency with `from_u64_pair` is the escape hatch.

### Pitfall 8: Mesh winding / skirt culling
**What goes wrong:** Back-face culling (`Face::Back`, `Ccw`) hides terrain or skirts. **How to avoid:** `u x v = n` bases guarantee CCW-outside for grid triangles; wind skirt quads so their normal faces away from the patch interior; add a CPU test computing every triangle's geometric normal and asserting `dot(normal, d) > 0` for terrain triangles and `dot(normal, outward_edge_dir) >= 0` for skirt triangles.

### Pitfall 9: Orbit clipping / precision
**What goes wrong:** Tiny near plane at orbit wastes nothing (reverse-Z infinite far) but a huge near plane at the surface clips terrain. **How to avoid:** near = clamp(alt*0.01, 0.05, 1e4); test altitude 2 m and 1e7 m both produce finite `view_proj`.

### Pitfall 10: Hysteresis state in an unordered set
**What goes wrong:** `HashSet` iteration order differs run to run, so tests/ goldens flake. **How to avoid:** `BTreeSet`/sorted `Vec`; banned-ops scan includes `HashMap`/`HashSet`.

## Code Examples

### Exact-op equal-angle `tan` (verified numerically in Python with identical IEEE ops)
```rust
// Source: this research (script /tmp/adjchk/tan.py); ADR-0008 allows + - * / only.
fn equal_angle_t(s: f64) -> f64 {
    if s == 1.0 { return 1.0; }
    if s == -1.0 { return -1.0; }
    let x = core::f64::consts::FRAC_PI_4 * s;
    let x2 = x * x;
    let mut sn = 1.0;                       // sin(x) = x * (1 - x2/(2*3) * (1 - x2/(4*5) * ...))
    for k in (1..=9).rev() { let k = f64::from(k); sn = 1.0 - sn * x2 / ((2.0 * k) * (2.0 * k + 1.0)); }
    let mut cs = 1.0;                       // cos(x) = 1 - x2/(1*2) * (1 - x2/(3*4) * ...)
    for k in (1..=10).rev() { let k = f64::from(k); cs = 1.0 - cs * x2 / ((2.0 * k - 1.0) * (2.0 * k)); }
    (x * sn) / cs
}
```

### Canonical sphere direction from face lattice indices
```rust
// Source: this research. xyz summation order is fixed => identical bits from any face.
fn face_dir(face: Face, gu: u64, gv: u64, g: u64) -> DVec3 {
    let s = (2.0 * gu as f64 - g as f64) / g as f64;   // exact dyadic
    let t = (2.0 * gv as f64 - g as f64) / g as f64;
    let (a, b) = (equal_angle_t(s), equal_angle_t(t));
    let (n, u, v) = face.basis();                       // integer-valued DVec3
    let p = DVec3::new(
        n.x + a * u.x + b * v.x, n.y + a * u.y + b * v.y, n.z + a * u.z + b * v.z);
    let len2 = p.x * p.x + p.y * p.y + p.z * p.z;       // world xyz order, never face-local
    p / len2.sqrt()
}
```
(`n + a*u + b*v` per component involves at most one non-zero term besides the +-1 normal term for each axis, so component values are exact copies of `a`, `b` or +-1 regardless of face; this is why the border vertices agree.)

### Neighbor derivation (port of the validated script)
```rust
// Source: this research (/tmp/adjchk/chk.py, 0 failures levels 0-5).
fn neighbor(p: PatchId, e: Edge) -> (PatchId, Edge) {
    let (f, l, x, y) = p.unpack(); let k = 1u32 << l;
    if let Some(inner) = step_inside(f, l, x, y, e, k) { return (inner, e.opposite()); }
    let out = FACE_BASIS[f].out(e);                   // integer vector
    let g = face_with_normal(out);
    let ge = FACE_BASIS[g].edge_with_outward(FACE_BASIS[f].n);   // +n_f, NOT -n_f
    let t = if e.is_u_edge() { y } else { x };
    let along_f = if e.is_u_edge() { FACE_BASIS[f].v } else { FACE_BASIS[f].u };
    let along_g = if ge.is_u_edge() { FACE_BASIS[g].v } else { FACE_BASIS[g].u };
    let t2 = if dot(along_f, along_g) > 0 { t } else { k - 1 - t };
    (PatchId::pack(g, l, edge_x(ge, k, t2), edge_y(ge, k, t2)), ge)
}
```

### Descent test skeleton
```rust
// Source: pattern from crates/engine-sdk/tests/headless.rs + extensions/conventional-3d basic_scene --headless
let mut rt = Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new())?;
let mut app = PlanetaryApp::for_test(TestConfig { settle_each_frame: true, ..Default::default() });
rt.setup(&mut app)?;
for frame in 0..FRAMES {
    rt.frame(&mut app, 1.0 / 60.0)?;                  // simulated clock: dt constant, no wall time
    let r = app.last_report();                         // FrameReport + SelectStats + draw count
    assert!(r.stream.started <= cfg.max_starts_per_frame && r.draws <= 1024 /* ... counted caps */);
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Distance-only LOD | Screen-space error with hysteresis | HLD 12.6 forbids distance-only | `proj_scale` and `eps(L)` model |
| Independent face quadtrees | Face adjacency with orientation flips + restricted quadtree | PITFALLS #5 | Exhaustive reciprocity tests |
| f32 world positions | f64 authoritative, per-patch origin + camera-relative extraction | Phase 2/5 | Already available |
| Setup-only GPU resource creation | Per-frame GPU hook (new, generic) | Phase 7 | Enables streaming |

**Deprecated/outdated:** none relevant (no new third-party APIs; wgpu/glam are used through existing wrappers).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `kappa = 0.25`, `h = 0.2`, `tau = 2 px`, skirt margin 1.5 are adequate | LOD / crack-free | Visible quality or gap; mitigated by measured-gap test and ADR table (planner should treat as initial values to tune) |
| A2 | Equal-angle cell size varies by ~1.3 across a face | Patch ID | L=19 may give 0.45-0.6 m cells; only affects the demo cap |
| A3 | 12-16 octaves sufficient visually; ~55 KB/patch and ~80 MB DETAIL budget | Generation / mesh | Memory budget tuning only |
| A4 | f32 origin-offset quantisation seams are sub-pixel | Pitfall 4 | Visible hairline seams in the manual run (skirts + tolerance test mitigate) |
| A5 | Per-patch generation (~1.2k heights x 12-16 octaves) is fast enough on workers at 8 starts/frame | Streaming | If not, lower `max_starts_per_frame`; caps are counted so tests still pass; measure in the `#[ignore]` perf log |
| A6 | Horizon culling formula is left to the planner as a conservative optimisation | LOD | None for correctness (culling off in proofs); wrong culling could hide terrain, so it needs its own test |
| A7 | Cross-OS bit equality of goldens holds (no libm, no FMA contraction in Rust) | Generation | Unverifiable until hosted CI runs (billing-blocked, same status as ADR-0008) |

## Open Questions (all RESOLVED)

1. **Can streamed meshes be uploaded after setup?** RESOLVED: No (verified `Resources` only in `setup`). Resolution: generic SDK hook `App::gpu_frame` + `Resources::destroy_mesh`, ADR-0009, first plan.
2. **Do planetary crates need their own tiers or can they share 6?** RESOLVED: `same_family` waives R5; use 7 and 8 for readability; must be listed in LAYERS and the docs table (R6 + `docs_cover_all_crates`).
3. **Does the canary CI job survive additional crates?** RESOLVED: yes if members are one-per-line containing `extensions/planetary/`, no planetary text elsewhere in root `Cargo.toml`, and inter-planetary deps are path deps in the crates' own manifests; update the `--exclude planetary-core` step.
4. **How to keep `tan` deterministic?** RESOLVED: exact-op Taylor sin/cos ratio with +-1 special case (numerically verified).
5. **Does 2:1 balance suffice for crack-freedom with streaming fallback?** RESOLVED: No; need effective-cut coarsening (Pattern 5).
6. **How to find the patch under the camera without `atan`?** RESOLVED: top-down boundary comparison using `T`.
7. **How many levels for ~1 m?** RESOLVED: L=19 with N=32 (0.6 m); format supports up to 28; cap 24.
8. **Does eviction notify the app?** RESOLVED: No; reconcile via `state()`.
9. **Exact SSE tuning constants:** RESOLVED as initial values + a measured-error test; final values recorded in ADR-0009 by the executing plan (not a blocker).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rustc / cargo | everything | yes | 1.99.0 | - |
| Cross-target `cargo check` (linux, macOS) | cross-platform confidence | yes (target dirs exist: `aarch64-apple-darwin`, others) | - | Windows-only local verification |
| GPU / window for manual run | APP-02 human verification | unknown | - | NullBackend headless run; manual window run stays a human-verification item |
| Hosted CI (3-OS) | golden cross-platform | no (GitHub billing) | - | Local Windows + cross `cargo check`, note pending in ADR/VERIFICATION as in Phase 6 |
| Disk | builds | tight (~12 GB free, `target` 5.8 GB) | - | No new deps/targets; `cargo test -p <crate>` per crate; avoid `--all-targets` release builds |
| python3 (research script only) | none at runtime | yes | - | not used by the project |

**Missing with no fallback:** none. Note `planetary-core -> engine-sdk` means its test binaries link wgpu/winit code (large, but already built for other crates; no extra disk beyond binaries).

## Validation Architecture

> `workflow.nyquist_validation` is `true` in `.planning/config.json`.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | built-in `cargo test` (libtest); no new framework |
| Config file | none (workspace lints; CI uses `RUSTFLAGS=-D warnings`) |
| Quick run command | `cargo test -p planetary-core` (then `-p planetary-terrain`, `-p planetary-app`) |
| Full suite command | `cargo test --workspace --locked` + `cargo clippy --workspace --all-targets --locked -- -D warnings` + `cargo run --locked -p xtask -- check-deps` + `check-canary` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PLAN-02 | PatchId pack/unpack/parent/children/validation | unit | `cargo test -p planetary-core --test patch_id` | Wave 0 |
| PLAN-02 | Adjacency reciprocity + integer-corner oracle, L<=6, all faces/edges/corners | integration (exhaustive) | `cargo test -p planetary-core --test adjacency` | Wave 0 |
| PLAN-03 | SSE selection, hysteresis (no toggles in band), leaf cap guard | unit/property | `cargo test -p planetary-core --test lod_select` | Wave 0 |
| PLAN-03 | 2:1 balance incl. cross-face; partition `sum 4^-L = 6` | property | `cargo test -p planetary-core --test lod_balance` | Wave 0 |
| PLAN-04 | Same-level border bit-equality (all 12 edges, 8 corners); mixed-level gap <= skirt; seam cameras; negative control | integration | `cargo test -p planetary-core --test seam` | Wave 0 |
| PLAN-04 | Effective-cut stays partition + balanced under partial residency; max_starts=1 | integration | `cargo test -p planetary-terrain --test effective_cut` | Wave 0 |
| PLAN-05 | `T(s)` exactness/monotone/odd; layer purity; golden hashes at 1 and N threads; banned ops scan | unit + golden | `cargo test -p planetary-core --test generator --test banned_ops` | Wave 0 |
| PLAN-05 | Triangle winding/normals sanity; border normal equality | unit | `cargo test -p planetary-core --test mesh` | Wave 0 |
| PLAN-06 | Camera: parallel transport, altitude-scaled speed, clearance, near-plane; input mapping | unit | `cargo test -p planetary-terrain --test camera` | Wave 0 |
| PLAN-07 | Scripted descent on simulated clock: counted caps per frame, determinism (twice), no leaks, no wall-clock tokens | integration (headless NullBackend) | `cargo test -p planetary-app --test descent` | Wave 0 |
| APP-02 | App uses only SDK + planetary roots; manifest deps; forbidden names | policy | `cargo test -p planetary-app --test sdk_only` | Wave 0 |
| (support) | SDK `gpu_frame` hook called each frame; `destroy_mesh` frees; `basic_scene` unchanged | unit | `cargo test -p engine-sdk` | extend existing tests |
| (support) | Layer table, docs, canary with new crates | policy | `cargo test -p xtask` and `cargo run -p xtask -- check-canary` | extend existing |

### Sampling Rate
- **Per task commit:** `cargo test -p <crate touched>` (< ~30 s each; exhaustive adjacency at L<=6 and seam at L<=4 with N=8 are milliseconds to seconds).
- **Per wave merge:** `cargo test --workspace --locked`, `cargo clippy ... -D warnings`, `check-deps`, `check-canary`, cross `cargo check --target x86_64-unknown-linux-gnu` (and macOS).
- **Phase gate:** full suite green, then manual window run (human-verification) before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] All test files above (none exist for planetary crates)
- [ ] Crates `planetary-terrain`, `planetary-app` scaffolded + LAYERS + docs + CI edits
- [ ] SDK hook + `destroy_mesh` + tests
- [ ] Banned-ops and no-wall-clock source scans for planetary crates
- [ ] No framework install needed

## Security Domain

> `security_enforcement` is absent in config (enabled by default). This phase has no network, auth, or persistence surface.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | - |
| V3 Session Management | no | - |
| V4 Access Control | no | - |
| V5 Input Validation | yes (limited) | `PatchId::from_raw` validation (face<6, level<=MAX, no stray bits); validate payload lengths/header before decoding bytes in the uploader; finite-checks on camera/view inputs; `create_lit_mesh` index validation |
| V6 Cryptography | no | FNV/splitmix are non-cryptographic (ADR-0008 decision 10); do not use for integrity |

### Known Threat Patterns
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malformed/oversized streamed payload crashing the decoder | Tampering/DoS | Length and count bounds against `N`-derived maxima before slicing; return `false`/error, never panic |
| Resource exhaustion via unbounded patch/requests | DoS | Counted caps (`max_pending`, starts/completions, leaf cap, budgets) |
| Out-of-range shifts/overflow in morton packing | Tampering | `checked_*`/range validation in `PatchId`, level <= 24 |
| NaN/Inf propagating into extraction | DoS | Reject non-finite camera positions (`WorldPos::from_xyz` already errors) |

## Sources

### Primary (HIGH confidence, read this session)
- `.planning/phases/07-.../07-CONTEXT.md`, `REQUIREMENTS.md`, `STATE.md`, `research/PITFALLS.md`, `research/ARCHITECTURE.md`
- `HLD.md` sections 12.3-12.7 and the planet config sample (`maximum_lod: 20`, `target_error_pixels: 2.0`, radius 6371000)
- `xtask/src/layers.rs`, `xtask/src/check.rs` (R1-R6 incl. `same_family` waiver), `xtask/src/canary.rs`, `xtask/tests/docs_policy.rs`, `.github/workflows/ci.yml` (build + canary jobs), `deny.toml`, root `Cargo.toml`, `docs/architecture/dependency-direction.md`
- ADR-0007 (Resources only in setup; canary), ADR-0008 (determinism rules, banned ops, goldens)
- `crates/engine-sdk/src/{lib,app,runtime,run}.rs`, `crates/engine-render/src/{backend,scene,handles,camera}.rs`, `crates/engine-render-wgpu/src/{backend,pipeline}.rs` (Ccw, back-face cull, `Greater` depth, `MAX_DRAWS_PER_FRAME`)
- `crates/engine-streaming/src/*`, `crates/engine-assets/src/{id,manager}.rs`, `crates/engine-procedural/src/*` (+ `tests/banned_ops.rs`, `crates/engine-streaming/tests/procedural_stream.rs`, `tests/common/mod.rs`)
- `extensions/conventional-3d` (`sdk_only.rs`, `basic_scene.rs`, `fly_camera.rs`), `extensions/planetary/planetary-core`
- Numeric verification scripts (throwaway, outside the repo): adjacency derivation + integer-geometry oracle (0 failures across 32,760 checks, levels 0-5) and exact-op `tan` accuracy/monotonicity/oddness.

### Secondary (MEDIUM)
- General chunked-LOD / restricted-quadtree / skirt practice (training knowledge, consistent with PITFALLS.md which itself is training-sourced; no live web verification performed this session).

### Tertiary (LOW)
- Tuning constants (kappa, h, margins, octave counts, memory budgets): initial values only, to be replaced by measurements recorded in ADR-0009.

## Metadata

**Confidence breakdown:**
- Standard stack / crate wiring / CI: HIGH, read from the repo and checked against the CI sed/grep logic
- Topology/adjacency: HIGH, validated by independent geometry oracle
- Generation determinism: HIGH on design, MEDIUM on cross-OS (unverified until hosted CI)
- LOD/skirt constants: MEDIUM-LOW (need prototype measurement; test design bakes in the measurement)
- Streaming integration: HIGH on API facts (manager/uploader source read), MEDIUM on perf

**Research date:** 2026-11-25
**Valid until:** 30 days (internal repo state; re-check if `engine-sdk` or `engine-streaming` change before planning)
