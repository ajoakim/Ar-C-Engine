# Phase 6: Streaming & Procedural Framework - Research

**Researched:** 2026-11-25
**Domain:** Rust workspace: async streaming framework over existing job/residency layers, deterministic procedural generation, xtask layer policy
**Confidence:** HIGH (codebase facts verified by reading source); MEDIUM on cross-platform float claims (language-semantics knowledge, not re-verified online this session; CI matrix is the final check)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- New `engine-streaming` crate, generic over a `StreamKey`; only keys, priority, budget class and cancel tokens (no planetary concepts).
- `StreamRequest {key, priority, budget_class, cancel}` drives a generation-tagged state machine reusing Phase 4 residency states; cancel frees reserved budget immediately.
- Per-frame caps on completions applied and bytes uploaded plus a pending-queue cap; the frame path never blocks on a worker.
- Pinned coarse-parent fallback: a resident parent is non-evictable while any child request is in flight; `resolve(key)` returns the best resident ancestor.
- New `engine-procedural` crate: `Generator` trait + layer stack; pure, GPU-free, no dependency on planetary crates.
- Determinism: `output = f(stable_id, layer_seed, params)`; layer seeds from a fixed, versioned hash (not `DefaultHasher`); in-house integer-hash noise in f64 (no `noise` crate).
- Layer seed = hash(world_seed, layer_name, version) so adding/removing layers does not perturb others.
- Golden-hash tests with fixed expected hashes at 1 and N worker threads; the Linux/macOS/Windows CI matrix is the cross-platform check (only Windows verified locally; hosted CI has not run yet).
- Float determinism: avoid transcendentals in hashed paths where possible; hash quantized results where used; document tolerance in an ADR (0008).
- Synthetic slow generator under a simulated frame loop asserts frame budget holds and coarse data is served; no wall-clock-only assertions.

### Claude's Discretion
Names, error types, module layout, hash function choice (fixed and versioned), test structure, ADR numbering (next free number).

### Deferred Ideas (OUT OF SCOPE)
Software-adapter GPU CI (Phase 8); planetary patch keys and terrain generators (Phase 7); on-disk caching of generated data.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| STRM-01 | Streaming requests carry key, priority, budget class, cancel token | `StreamRequest<K>` + `StreamSource<K>`; delegates budget/generation to `ResidencyManager`, priority to `engine-jobs::Priority`, cancel to `engine_jobs::CancelToken` (Standard Stack, Pattern 1) |
| STRM-02 | Rendering never blocks; coarse parent fallback until refinement resident | Pin table + `resolve()` + per-frame caps + simulated-frame test (Patterns 2-4, Validation) |
| PROC-01 | Deterministic, independently seeded generator layers, pure functions of stable ID and seed | SplitMix64 + FNV-1a hashing, exact-arithmetic value noise, layer-seed derivation (Pattern 5-6) |
| PROC-02 | Golden-hash tests identical across thread counts and platforms | FNV-1a-64 over LE bytes, key-ordered collection, 1 vs N workers, CI matrix (Pattern 7) |
</phase_requirements>

## Summary

Phase 6 is almost entirely in-house Rust with **zero new third-party dependencies**. `engine-streaming` is a thin orchestration layer over what already exists: `engine_jobs::{JobSystem, Priority, CancelToken}` for scheduling/cancellation and `engine_assets::{ResidencyManager, AsyncLoader, AssetId, AssetClass}` for state machine, budgets, per-frame upload cap, generation tagging and LRU candidates. What the existing code does NOT provide, and streaming must add: a generic key (ResidencyManager is keyed by `AssetId`), a not-yet-started pending queue with a cap, a per-frame completion cap, pin/refcount protection against eviction (`eviction_candidates` is pure and pin-unaware), and `resolve()` ancestor fallback. `engine-procedural` is a leaf of pure integer-hash + IEEE-exact arithmetic; no deps needed.

Determinism hinges on two things: (1) every hashed value derived from integer hashing (SplitMix64 finalizer, FNV-1a-64) with explicit `u64`/little-endian conventions, and (2) float math restricted to `+ - * / sqrt floor` (all correctly rounded / exact per IEEE-754), never `sin/cos/exp/ln/pow/powf/powi/tan` (platform libm/compiler-rt dependent). With that design, golden hashes can hash raw `f64::to_bits` bytes and need no tolerance; quantization is kept only as an escape hatch.

The xtask change is mechanical: two rows in `LAYERS`, two rows + mermaid edges in `dependency-direction.md`, root `Cargo.toml` members + workspace.dependencies, commit `Cargo.lock` (CI uses `--locked`), and add ADR-0008 (+ README row + optional docs_policy literals). `deny.toml` needs no change. The planetary-absent canary is unaffected.

**Primary recommendation:** `engine-streaming` = tier 3 (deps engine-jobs, engine-assets); `engine-procedural` = tier 1 (may depend on engine-math/engine-diagnostics, initially none); delegate budgets/LRU to `ResidencyManager`, add pins and caps in the streaming layer, and make all slow-load tests gate-driven (frame-stepped), never sleep-driven.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Stream request queue, priority, caps | engine-streaming (tier 3) | engine-jobs (1) | Policy lives above scheduler |
| Budget accounting, generation tags, LRU candidates | engine-assets ResidencyManager (2) | — | Already built/tested in Phase 4; reuse, do not duplicate |
| Pin / ancestor resolve | engine-streaming | — | Eviction policy owner; ResidencyManager stays pin-unaware |
| Generator trait, layer seeds, noise, hashing | engine-procedural (tier 1) | — | Pure math-level leaf, GPU-free |
| Parallel execution of generation | engine-jobs | engine-streaming | Streaming submits; procedural never spawns threads |
| Key/patch semantics (quadtree parent, etc.) | Phase 7 planetary ext | — | Out of scope; core only sees `StreamKey::parent()` |

## Standard Stack

### Core (all already in workspace; no new external crates)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| engine-jobs | workspace | `JobSystem::submit(Priority, FnOnce(&CancelToken))`, `JobHandle`, builder `.workers(n)` / `.start_paused()` | Single-worker deterministic mode exists [VERIFIED: crates/engine-jobs/src/scheduler.rs] |
| engine-assets | workspace | `ResidencyManager`, `AsyncLoader`, `AssetId`, `AssetClass`, `ClassBudget`, `ResidencyState` | Budgets, `cancel()` releasing bytes + bumping generation, `try_begin_upload` cap, `eviction_candidates` [VERIFIED: crates/engine-assets/src/manager.rs] |
| thiserror | 2.0.21 | error enums | Already pinned in `[workspace.dependencies]` |
| crossbeam-channel | 0.5.17 | (tests only) gate channels | Already pinned; optional |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| In-house SplitMix64/FNV-1a | `xxhash-rust`, `noise`, `fastnoise` | Extra dep, cargo-deny surface, `noise` uses libm-dependent math; in-house is ~40 LOC and fully specified. Locked by CONTEXT. |
| Own state machine in streaming | Reuse `ResidencyManager` via `AssetId` | Reuse wins: budgets, generation, upload cap, LRU already verified. |

**Installation:** none (`cargo add` not needed). Both new crates use `[dependencies] ... .workspace = true` and `[lints] workspace = true`, copying the engine-jobs Cargo.toml shape.

## Architecture Patterns

### System Architecture Diagram

```
 caller (frame thread)                         worker threads (JobSystem)
 ---------------------                         --------------------------
 request(StreamRequest{key,prio,class,cancel})
        |
        v
 [pending queue: cap N, ordered (priority desc, seq asc)] --reject if full--> Err(QueueFull)
        |  update(frame):
        |   1 sweep: cancel tokens set / explicit cancel -> manager.cancel() (bytes freed now), unpin
        |   2 start up to `max_starts` from pending: pin best resident ancestor,
        |        manager.request -> AsyncLoader::start(est_bytes, produce)  ----->  produce(key, &CancelToken)
        |   3 loader.poll() (non-blocking) -> completions buffered                    (may be slow/gated)
        |   4 apply <= max_completions: key CpuReady, release pin on success/fail
        |   5 uploads: <= upload cap via manager.try_begin_upload -> Uploader -> complete_upload
        |   6 evict: candidates = manager.eviction_candidates(..) minus pinned/touched-this-frame
        v
 resolve(key) -> best resident ancestor (touch() for LRU) -> renderer draws coarse until child GpuResident
```

### Recommended Project Structure
```
crates/engine-streaming/src/{lib.rs, key.rs (StreamKey trait), request.rs, manager.rs (StreamManager), pins.rs, error.rs}
crates/engine-streaming/tests/{slow_load.rs, cancel.rs, pins_lru.rs}
crates/engine-procedural/src/{lib.rs, hash.rs (splitmix64, fnv1a64, hash_lattice), seed.rs (layer_seed, StableId), noise.rs, generator.rs (Generator, LayerStack), golden.rs?}
crates/engine-procedural/tests/golden.rs
docs/architecture/adr/0008-deterministic-streaming-and-procedural-generation.md
```

### Pattern 1: Generic key delegating to ResidencyManager
`ResidencyManager` is keyed by `AssetId` (UUID v5 over normalized string). Define
```rust
pub trait StreamKey: Clone + Ord + Send + 'static {
    fn asset_id(&self) -> AssetId;          // stable, injective-enough mapping (e.g. AssetId::from_uuid on hashed bytes)
    fn parent(&self) -> Option<Self>;       // coarse ancestor; None at root
}
```
Keep a `BTreeMap<AssetId, K>` side table (BTreeMap, never HashMap, for iteration determinism). `StreamSource<K>` supplies `estimate_bytes(&K) -> u64` and `produce(&K, &CancelToken) -> Option<Vec<u8>>` (the spec'd request fields do not include a size, but `begin_generating(id, cpu_bytes)` needs one at start). `StreamRequest { key, priority: engine_jobs::Priority, budget_class: AssetClass, cancel: CancelToken }`.

Request flow: `track(id, class)` once, `manager.request(id)` (Unloaded|Failed -> Requested, returns generation), then `AsyncLoader::start(jobs, manager, id, priority, cpu_bytes, produce)` which does `begin_generating` (reserves bytes; fails `OverBudget` before submitting) [VERIFIED: loader.rs]. Cancel: `AsyncLoader::cancel(manager, id)` cancels the job handle and calls `manager.cancel`, which releases bytes and bumps generation so any late completion is rejected with `StaleRequest` [VERIFIED: manager.rs `cancel`, `finish_generating`]. This is the "cancel frees budget immediately" mechanism; assert `verify_accounting()` and class `cpu_used` back to prior value right after cancel.
Requests still in the pending queue (not started) hold no bytes; cancelling just removes them.

### Pattern 2: Per-frame caps (all deterministic, counted not timed)
`StreamConfig { max_pending, max_starts_per_frame, max_completions_per_frame, upload_cap_bytes (ResidencyManager::new) }`. `AsyncLoader::poll` applies ALL finished work at once (no cap) [VERIFIED], so streaming buffers returned `LoadEvent`s in a `VecDeque` and only *surfaces/promotes* `max_completions_per_frame` per `update` (rest carry to next frame, FIFO by completion then AssetId order). Uploads use the existing `try_begin_upload` per-frame cap (an oversized asset may go first in a frame so it can't starve) and `begin_frame(frame)` resets the counter. Never call `wait_all`/`JobHandle::wait` on the frame path; `wait_all` is test-only.

### Pattern 3: Pinned coarse-parent fallback and LRU
`eviction_candidates(class, pool, bytes_needed)` is pure, sorted by `(last_used_frame, id)`, and stops once `bytes_needed` is covered; it knows nothing about pins [VERIFIED]. Therefore:
1. Streaming keeps `pins: BTreeMap<AssetId, u32>` (refcount).
2. When starting a child request for key `k`, find `a = best resident ancestor of k` (walk `parent()`; resident = `GpuResident` or `CpuReady` per policy) and `pin(a)`; store `a` in the in-flight record. Unpin on completion (success, fail, cancel, stale). Pinning only the immediate parent is insufficient when the parent itself is not resident; pin the actual resolved fallback.
3. Eviction wrapper: call `eviction_candidates(class, pool, u64::MAX)` (ask for the full ordered list), filter out pinned ids and anything `touch`ed this frame, then take until `bytes_needed` is covered. Do NOT filter the truncated list (it could under-cover). Optionally later add a filtered variant to engine-assets, but not required.
4. `resolve(key)`: walk key -> parent -> ... returning the first whose state is `GpuResident`; call `manager.touch()` on it each frame so LRU does not age out the data being displayed. Return `Resolved { key, exact: bool }`.
5. Budget interplay: a pinned parent still counts against budget; if a class is over budget with only pinned residents, eviction yields fewer bytes and the new request waits (stays in pending) rather than evicting a pin. Document as intended back-pressure; test it.
Evicting uses `begin_evict` then `finish_evict` (CpuReady|GpuResident -> Evicting -> Unloaded).

### Pattern 4: Deterministic slow-loading test (no wall clock)
- Use `JobSystem::builder().workers(1)` (or N). The synthetic producer blocks on a **gate**: an `Arc<AtomicU64>` "released tick" the test advances, polled with `std::thread::yield_now()` while also checking `CancelToken::is_cancelled()`; or a crossbeam channel `recv()` the test sends on. No `sleep`, no `Instant`.
- Simulated frame loop: `for frame in 0..F { mgr.begin_frame(frame); stream.update(frame); count work units; render(resolve(key)) }`. Assert each frame: completions applied <= cap, bytes uploaded <= cap (or the single-oversize exception), `update` returned (non-blocking is proven because the test thread keeps stepping frames while the gate is closed; add a per-test timeout via the CI job/`cargo test` harness for hang-as-failure). Assert `resolve(child).exact == false` and returns the parent for every frame until the test opens the gate and the child reaches `GpuResident`, then `exact == true`.
- Make "when completion becomes visible" deterministic by opening the gate and then calling `loader.wait_all`-equivalent in the test harness only at the designated frame (test-only helper), so the frame at which refinement appears is fixed, not timing-dependent.
- Frame "budget" = work-unit counts (starts, completions, upload bytes, pending length), not milliseconds. An optional informational timing log is fine but never asserted.

### Pattern 5: Hashing (fixed, versioned, in-house)
Specify in ADR-0008 and name `HASH_VERSION: u32 = 1`.
```rust
// Source: SplitMix64 (Steele/Lea/Flood; public-domain reference by Vigna), FNV-1a 64 (public spec)
pub const fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
pub const fn fnv1a64(bytes: &[u8]) -> u64 {            // offset 0xcbf29ce484222325, prime 0x100000001b3
    let mut h = 0xcbf2_9ce4_8422_2325u64; let mut i = 0;
    while i < bytes.len() { h ^= bytes[i] as u64; h = h.wrapping_mul(0x0000_0100_0000_01b3); i += 1; }
    h
}
```
Use SplitMix64 for seed mixing/lattice hashing and FNV-1a-64 for (a) layer-name hashing and (b) golden output hashing. Rules: only `u64`/`i64` (never `usize`/`isize`), signed lattice coordinates cast `i64 as u64` (defined two's complement), all multi-byte serialization via `to_le_bytes()`, `wrapping_*` arithmetic only. Pin known-answer tests: FNV-1a("")=0xcbf29ce484222325, FNV-1a("a")=0xaf63dc4c8601ec8c (published vectors), and SplitMix64 outputs for seed 0 from the reference (record the generated values in tests as goldens; verify FNV vectors against the spec during implementation).
Layer seed: `layer_seed = splitmix64(splitmix64(world_seed ^ fnv1a64(name.as_bytes())) ^ u64::from(layer_version))`. It depends only on `(world_seed, name, version)`, never on layer index/order, so adding/removing/reordering layers cannot perturb others. Test: layer A's output hash is identical in stacks `[A]`, `[B,A]`, `[A,B,C]`.
Lattice: `hash3(seed, ix, iy, iz) = splitmix64(seed ^ splitmix64((ix as u64) ^ splitmix64((iy as u64) ^ splitmix64(iz as u64))))`.

### Pattern 6: Exact-arithmetic noise
- Float from hash: `((h >> 11) as f64) * (1.0 / 9007199254740992.0)` -> [0,1), exact (53-bit integer to f64 is exact; scale by 2^-53 is exact power-of-two multiply).
- Value noise: `floor` the coordinate (exact), fractional `t = x - floor(x)` (exact for the ranges used), fade `t*t*(3.0 - 2.0*t)`, lerp `a + (b - a) * t`. Only `+ - * /`, `floor`, optionally `sqrt`.
- fBm: fixed octave loop; frequency/amplitude via repeated `* 2.0` / `* 0.5` (exact), NOT `powi`/`powf`.
- Evaluation order is fixed by the source expression; Rust does not reassociate or fuse floating-point ops by default (no fast-math), so the same source gives the same bits on any IEEE-754 target [ASSUMED: language-level guarantee from training knowledge; CI matrix + goldens verify]. Do not use `mul_add`, `sin/cos/tan/exp/ln/log/powf/powi/cbrt/hypot/atan2/tanh`. Add a CI-grep-style unit/lint note in ADR (and optionally a `#[test]` that greps procedural `src/` for banned identifiers).
- Never `-C target-cpu=native`, no `-ffast-math`-like flags; keep `.cargo/config` untouched.
- Hash raw bits: golden hash feeds `f64::to_bits().to_le_bytes()` after canonicalizing `-0.0 -> 0.0` and asserting no NaN (panic/error if NaN). With exact-only ops this needs no tolerance; ADR-0008 states the tolerance policy: bit-exact for the framework; any future transcendental use must be quantized (e.g. `(v * 2^20).round() as i64`) before hashing and documented, or routed through a pure-Rust software implementation.

### Pattern 7: Golden-hash tests across thread counts
Generate a fixed grid of stable IDs through `JobSystem` with 1 worker and with N (e.g. 4) workers, in forward and shuffled submission order; collect results into `BTreeMap<StableId, Vec<f64>>` (order by key, never by completion), hash in key order with FNV-1a-64, assert equal to each other AND to a hard-coded `const GOLDEN: u64`. Cross-platform check = the same constant asserted on the Linux/macOS/Windows CI matrix (`ci.yml` build matrix `[ubuntu-latest, windows-latest, macos-latest]`). Initial golden: compute on Windows locally, commit; hosted CI is the first real cross-platform confirmation (flag in plan: first CI run may reveal drift; if so fix the offending op, don't loosen the test). Include also per-layer goldens and a "layer isolation" golden.

### Anti-Patterns to Avoid
- `HashMap`/`HashSet` iteration or `std::collections::hash_map::DefaultHasher` anywhere in hashed/ordered paths (randomized/unspecified).
- Ordering results by thread completion; always sort by key.
- `usize` in hashes, `as f64` of large ints (>2^53), `format!("{}", f64)` into hashes.
- Asserting elapsed milliseconds; blocking on `JobHandle::wait` in `update`.
- Evicting a pinned or just-touched resident; filtering truncated candidate list.
- Putting patch/planet notions (lat/long, face, LOD naming) in either new crate.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Budget/byte accounting, generation tags, LRU | Another ledger | `ResidencyManager` | Phase 4 verified incl. `verify_accounting` |
| Cancellation | New flag type | `engine_jobs::CancelToken` | Shared with scheduler/loader |
| Worker pool/priority queue | Threads/channels | `JobSystem` + `Priority` | Has single-worker deterministic mode |
| Async result plumbing | New channel protocol | `AsyncLoader` (start/poll/cancel) | Stale-generation rejection built in |
| Noise/hash | `noise` crate / DefaultHasher | In-house SplitMix64 + FNV-1a (tiny, specified) | Locked decision; exactness |

**Key insight:** the new code is policy (pins, caps, resolve) and a specification (hash/noise); everything stateful and risky already exists.

## Common Pitfalls

### Pitfall 1: Truncated eviction list vs pins
**What goes wrong:** filtering `eviction_candidates(.., bytes_needed)` after truncation frees too little or evicts the wrong thing. **Avoid:** request `u64::MAX`, filter, then take. **Warning:** budget stalls with free-able residents.

### Pitfall 2: Parent evicted while child in flight
**Goes wrong:** pop-in/holes. **Avoid:** pin the *resolved* ancestor at start, unpin on every terminal path (done/fail/cancel/stale/drop). Test every terminal path returns pin count to zero.

### Pitfall 3: Completion timing nondeterminism in tests
`AsyncLoader::poll` surfaces whatever finished. Tests asserting "frame K shows refinement" must use gates and an explicit join point, not real timing.

### Pitfall 4: Stale completions after cancel/re-request
Handled by generation (`StaleRequest`); streaming must treat `StaleRequest` as benign (drop data, don't `fail`).

### Pitfall 5: Float drift sources
`powi`, `sin`, fused multiply-add contraction, 32-bit x87 (not in CI), `as f32` mid-chain, `f64::mul_add`. Keep ops list above; golden on CI matrix.

### Pitfall 6: Layer seeds that depend on index
Using `layer_index` or stack length in the seed breaks the isolation requirement. Seed from name+version only.

### Pitfall 7: Queue cap semantics
`request` over `max_pending` must return `Err(QueueFull)` (or evict lowest priority, document one) without blocking; test both reject path and priority order (priority desc, then FIFO seq).

### Pitfall 8: Panicking producers
A panic in `produce` becomes `LoadEvent::Failed` (asset -> `Failed`, bytes released) [per loader doc]; unpin and allow re-request (`Failed -> Requested` legal).

## Code Examples

### Layer + generator shape
```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct StableId(pub u64);
pub trait Generator: Send + Sync {
    fn name(&self) -> &'static str;      // seed input
    fn version(&self) -> u32;            // bump = deliberate golden break
    fn generate(&self, id: StableId, seed: u64, params: &Params, out: &mut Vec<f64>);
}
// LayerStack::generate(world_seed, id, params): for each layer in declared order,
// seed = layer_seed(world_seed, name, version); layer writes only its own output slice.
```
(Pure: no globals, no clock, no thread-local RNG, no I/O.)

## xtask / workspace changes (exact)

| File | Change |
|------|--------|
| `Cargo.toml` | add `"crates/engine-streaming"`, `"crates/engine-procedural"` to `members`; add `engine-streaming = { path = "crates/engine-streaming" }`, `engine-procedural = { path = "crates/engine-procedural" }` to `[workspace.dependencies]` |
| `xtask/src/layers.rs` | `Layer { name: "engine-procedural", tier: 1 }`, `Layer { name: "engine-streaming", tier: 3 }`. R5 only checks strictly-lower tier (verified: table holds only `name`+`tier`), so "allowed edges" are documentation + tier arithmetic. |
| `docs/architecture/dependency-direction.md` | table rows: `engine-procedural` / crates/engine-procedural / 1 / `engine-math`, `engine-diagnostics`; `engine-streaming` / crates/engine-streaming / 3 / `engine-jobs`, `engine-assets`, `engine-core`. Mermaid: `engine-streaming --> engine-jobs`, `engine-streaming --> engine-assets`; `engine-sdk --> engine-streaming`, `engine-sdk --> engine-procedural` (if sdk re-exports). `docs_cover_all_crates` requires `` `name` `` literal for every workspace package. |
| `crates/engine-sdk` | add both as deps and re-export so Phase 7 extensions (which may depend only on `engine-sdk`, rule R4/R1 pattern) can implement `StreamKey`/`Generator`. Tier 5 > 3, legal. The sdk row says "all engine-* crates above" so the doc row needs no change. |
| `deny.toml` | No change: no new third-party crates; workspace path deps covered by `allow-wildcard-paths = true`. |
| `Cargo.lock` | Regenerate and commit (CI uses `--locked`). |
| `docs/architecture/adr/0008-...md` + `README.md` index row | New ADR from `0000-template.md` (must contain `- Status:`, Context, Decision, Alternatives Considered, Consequences, Evidence). Optionally extend `docs_policy.rs` with a test requiring ADR-0008 literals (`splitmix64`, `fnv1a`, `bit-exact`). |
| `xtask/tests/docs_policy.rs` | No mandatory change (tests iterate workspace packages / LAYERS); add the optional ADR-0008 test. |
| CI | `.github/workflows/ci.yml` needs no change: matrix `cargo test --workspace` picks up new crates; canary job only deletes `extensions/planetary` and the `Cargo.toml` lines matching `extensions/planetary/`, which does not touch the new members; `check-canary` closure from `conventional-3d` stays planetary-free because the new crates depend only on core crates. Verify by running `cargo run -p xtask -- check-deps` and `check-canary` locally. |

Tier justification: streaming depends on tier 1 (jobs) and tier 2 (assets) so it must be >= 3; it shares tier 3 with `engine-render`/`engine-spatial` but has no edge to them. Procedural is a pure math-level leaf; tier 1 allows it to use `engine-math` later without tier edits (tier 0 would forbid it).

## Runtime State Inventory
Not applicable (greenfield additions; no rename/migration).

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| libm `sin/cos` gradient noise | Integer-hash value noise with exact ops | Bit-identical across OS/arch |
| `DefaultHasher` seeds | Versioned SplitMix64/FNV-1a | Stable across Rust releases |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Rust does not contract/reassociate float ops without fast-math, so exact-op expression trees give identical bits on x86_64/aarch64 | Pattern 6 | Goldens differ on a CI OS; fix by isolating the op, not loosening test |
| A2 | FNV-1a("a") = 0xaf63dc4c8601ec8c and empty = offset basis | Pattern 5 | Wrong KAT in test; verify against spec when implementing |
| A3 | Immediate parent of a "resident" fallback means `GpuResident` only (CpuReady not drawable) | Pattern 3 | Resolve policy tweak; low |
| A4 | Tier 1 for engine-procedural is acceptable to the user (vs tier 0) | xtask table | Easy table edit |
| A5 | Reject-on-full pending queue is preferred over drop-lowest | Pitfall 7 | Behavior detail |

## Open Questions (RESOLVED)

1. **Does ResidencyManager support generic keys?** RESOLVED: no (AssetId only). Use `StreamKey::asset_id()` + side table.
2. **Does it support pinning?** RESOLVED: no; `eviction_candidates` is pure so streaming filters (Pattern 3).
3. **Does `AsyncLoader` cap completions?** RESOLVED: no; streaming buffers and caps (Pattern 2).
4. **Request size for budget reservation?** RESOLVED: `StreamSource::estimate_bytes(&K)`.
5. **Do new crates affect the canary?** RESOLVED: no (see table).
6. **Does deny.toml need changes?** RESOLVED: no.
7. **ADR number?** RESOLVED: 0008 (0007 is last).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | build/test | yes | 1.99.0 pinned (`rust-toolchain.toml`) | — |
| cargo-deny | optional local check | not probed | — | CI runs it |
| Disk | builds | ~18 GB free | — | Avoid extra deps; no new crates so no large builds; use `cargo test -p <crate>` and shared target dir |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | built-in `cargo test` (libtest), Rust 1.99.0 |
| Config file | none; workspace lints |
| Quick run command | `cargo test -p engine-streaming -p engine-procedural` |
| Full suite command | `cargo test --workspace --locked` plus `cargo run --locked -p xtask -- check-deps` and `check-canary` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| STRM-01 | Request carries key/priority/class/cancel; priority order; queue cap | unit/integration | `cargo test -p engine-streaming request` | Wave 0 |
| STRM-01 | Cancel frees budget immediately (`cpu_used` restored, `verify_accounting`), late completion stale | integration | `cargo test -p engine-streaming cancel` | Wave 0 |
| STRM-02 | Slow generator: caps hold every frame, parent served until child resident, no blocking | integration (gate + frame loop) | `cargo test -p engine-streaming slow_load` | Wave 0 |
| STRM-02 | Pinned parent survives LRU pressure; unpinned on all terminal paths | integration | `cargo test -p engine-streaming pins_lru` | Wave 0 |
| PROC-01 | Layer seed isolation, add/remove/reorder invariance; purity | unit | `cargo test -p engine-procedural seed` | Wave 0 |
| PROC-01 | Hash KATs (FNV, SplitMix64) | unit | `cargo test -p engine-procedural hash` | Wave 0 |
| PROC-02 | Golden hash equal at 1 vs N workers, shuffled order, matches constant | integration | `cargo test -p engine-procedural --test golden` | Wave 0 |
| PROC-02 | Cross-platform | CI matrix | `cargo test --workspace --locked` on 3 OSes | CI (first run pending) |
| Policy | Layer table/docs/ADR coverage | policy | `cargo test -p xtask` | exists |

### Sampling Rate
- Per task commit: quick run command; Per wave merge: full suite; Phase gate: full suite green + check-deps + check-canary.

### Wave 0 Gaps
- [ ] Both crate scaffolds (Cargo.toml, lib.rs) and workspace/xtask/doc registration (do first; other tests depend on it)
- [ ] `engine-streaming/tests/{slow_load,cancel,pins_lru}.rs`, `engine-procedural/tests/golden.rs`
- [ ] ADR-0008 file + README row

## Security Domain

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2/V3/V4 | no | — |
| V5 Input Validation | yes | Bounded pending queue, byte estimate checked vs budget (`OverBudget`), reject NaN in golden path |
| V6 Cryptography | no | Hashes are non-cryptographic by design; ADR must say so (not for security) |

### Known Threat Patterns
| Pattern | STRIDE | Mitigation |
|---------|--------|------------|
| Request flood / unbounded memory | DoS | `max_pending`, budget checks, per-frame start cap |
| Panicking/hung producer | DoS | Job panic -> `Failed`; cancel token; frame path never joins |
| Integer overflow in accounting | Tampering | Existing `checked_add`/`AccountingOverflow`; `wrapping_*` only in hashes |

## Sources

### Primary (HIGH confidence, read this session)
- crates/engine-assets/src/{manager,loader,residency,id,lib}.rs; crates/engine-jobs/src/{scheduler,priority,cancel}.rs
- xtask/src/layers.rs, xtask/tests/docs_policy.rs, docs/architecture/dependency-direction.md, deny.toml, Cargo.toml, .github/workflows/ci.yml, ADR README

### Secondary / Tertiary
- SplitMix64 and FNV-1a constants and IEEE-754 operation exactness from training knowledge [ASSUMED]; not re-fetched online; confirm KATs when implementing.

## Metadata

**Confidence breakdown:**
- Standard stack / xtask changes: HIGH (read code)
- Architecture (pins, caps): HIGH (derived from verified APIs)
- Cross-platform float exactness: MEDIUM (design avoids risky ops; CI is final proof)

**Research date:** 2026-11-25
**Valid until:** 2026-12-25
