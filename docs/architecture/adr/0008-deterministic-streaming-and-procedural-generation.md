# ADR-0008: Generic Streaming and Deterministic Procedural Generation

- Status: accepted
- Date: 2026-11-25
- Requirement/Phase: STRM-01, STRM-02, PROC-01, PROC-02 / Phase 6

## Context

Phase 6 delivers a generic streaming framework (keyed requests, budgets, cancellation, per-frame caps, coarse fallback) and a deterministic procedural generation framework (seeded, layered, reproducible across threads and platforms). Both must be general-purpose core crates with no planetary concepts; Phase 7 builds the planetary implementations on top of them through `engine-sdk`.

## Decision

1. Two core crates: `engine-procedural` (tier 1, pure leaf, no dependencies besides `thiserror`) and `engine-streaming` (tier 3, over `engine-jobs` + `engine-assets`). Neither contains planetary concepts; Phase 7 implements `StreamKey`/`StreamSource`/`Generator` in the planetary extension via `engine-sdk`.
2. `StreamRequest { key, priority, budget_class, cancel }`; `StreamKey` exposes only `asset_id()` and `parent()`; `StreamSource` supplies `estimate_bytes` and `produce`. Residency states, budgets, generation tags, upload cap and LRU ordering are delegated to the Phase 4 `ResidencyManager`/`AsyncLoader` (not duplicated).
3. Pending queue: bounded by `max_pending`, ordered by priority (descending) then submission sequence (FIFO); a request over the cap is REJECTED with `StreamError::QueueFull` (no drop-lowest-priority). Requests waiting in the queue hold no budget.
4. Cancellation frees budget immediately: pending -> removed; `Requested`/`Generating` -> `AsyncLoader::cancel` (bytes released, generation bumped, late completion rejected as stale); `CpuReady` not yet uploaded -> evicted. A cancelled request token is honoured at the next `update`; `StreamManager::cancel` acts at once.
5. The frame path never blocks on a worker: `update` only polls (`AsyncLoader::poll`), never waits. Per-frame caps are counted, not timed: `max_starts_per_frame`, `max_completions_per_frame`, and a per-frame upload byte cap (`ResidencyManager::try_begin_upload`; an oversized item may upload first in a frame so it cannot starve). Frame budget in tests = work-unit counts; no wall-clock assertions.
6. Coarse fallback: when a request starts, its best `GpuResident` ancestor is pinned (refcount) until the request reaches `GpuResident`, fails or is cancelled; pinned and recently shown (current or previous frame) residents are never evicted; `resolve(key)` returns the nearest `GpuResident` key on the ancestor chain with `exact` = whether it is the key itself. If only pinned data occupies the budget, new work waits in the queue (back-pressure) instead of evicting a pin.
7. Hashing specification, `HASH_VERSION = 1`: `splitmix64` (constants `0x9E3779B97F4A7C15`, `0xBF58476D1CE4E5B9`, `0x94D049BB133111EB`, shifts 30/27/31) and `fnv1a64` (offset `0xcbf29ce484222325`, prime `0x100000001b3`) over little-endian bytes; only `u64`/`i64` (never `usize`) in hashed values; `wrapping_*` arithmetic. Layer seed = `splitmix64(splitmix64(world_seed ^ fnv1a64(name)) ^ version)`: depends only on (world seed, layer name, layer version), never on layer index, so adding, removing or reordering layers does not perturb other layers. Generator output = f(stable id, layer seed, params).
8. Float determinism: hashed paths use only `+ - * /`, `floor`, `sqrt` and exact integer-to-float conversions (< 2^53); banned: `sin`, `cos`, `tan`, `exp`, `ln`, `log`, `powf`, `powi`, `cbrt`, `hypot`, `atan2`, `tanh`, `mul_add`, `DefaultHasher`, `HashMap`/`HashSet` in ordered paths; no `target-cpu=native` or fast-math flags. Tolerance policy: bit-exact for the framework (golden hashes over `f64::to_bits` little-endian bytes after canonicalising `-0.0` to `0.0`; NaN/infinite outputs are an error). Any future transcendental use must quantize before hashing (e.g. `(v * 1048576.0).round() as i64`) and be recorded in a superseding ADR. A unit test greps `crates/engine-procedural/src` for banned identifiers.
9. Golden-hash tests pin output at 1 and N threads (std threads inside `engine-procedural`, `JobSystem` with 1 and 4 workers through `engine-streaming`); the Linux/macOS/Windows CI matrix asserts the same constants. Goldens were computed on Windows; the first hosted CI run is the first cross-platform confirmation, and any drift is fixed at the offending operation, never by loosening the test.
10. These hashes are non-cryptographic and not for security; they must never be used for authentication, integrity against adversaries or secrets.

## Alternatives Considered

- `noise`/`fastnoise-lite` crates: libm-dependent output, stability not guaranteed.
- `DefaultHasher`: unspecified, may change between Rust releases.
- `xxhash-rust`: extra dependency and cargo-deny surface for ~40 LOC.
- Drop-lowest-priority on a full queue: silent loss; rejected by orchestrator decision.
- Own state machine in streaming: duplicates verified Phase 4 accounting.
- Tier 0 for procedural: would forbid a future `engine-math` dependency.
- Wall-clock frame budget tests: flaky.

## Consequences

- Cross-platform equality is unverified until hosted CI runs (billing-blocked).
- Value noise is blockier than gradient noise (acceptable for the framework; Phase 7 may add layers within the same rules).
- `ResidencyManager` stays pin-unaware, so the streaming layer filters LRU candidates itself; a pinned parent still counts against budget.
- `settle()` is a blocking test/shutdown helper that must never be called on the frame path.

## Evidence

Tests in `crates/engine-procedural/tests/golden.rs`, `crates/engine-procedural/tests/banned_ops.rs`, `crates/engine-streaming/tests/{request,cancel,slow_load,pins_lru,procedural_stream}.rs`; pinned golden constants are listed here by plan 06-06 once recorded.
