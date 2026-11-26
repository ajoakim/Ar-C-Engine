# Phase 6: Streaming & Procedural Framework - Context

**Gathered:** 2026-11-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Data can be generated and streamed asynchronously and deterministically without ever stalling rendering (STRM-01, STRM-02, PROC-01, PROC-02). Introduces two NEW crates, `engine-streaming` and `engine-procedural` (deferred from Phase 1), which require xtask layer-table, dependency-direction doc, workspace and cargo-deny updates. Core stays planet-agnostic.

</domain>

<decisions>
## Implementation Decisions

### Streaming framework
- New `engine-streaming` crate, generic over a `StreamKey`; only keys, priority, budget class and cancel tokens (no planetary concepts).
- `StreamRequest {key, priority, budget_class, cancel}` drives a generation-tagged state machine reusing Phase 4 residency states; cancel frees reserved budget immediately.
- Per-frame caps on completions applied and bytes uploaded plus a pending-queue cap; the frame path never blocks on a worker.
- Pinned coarse-parent fallback: a resident parent is non-evictable while any child request is in flight; `resolve(key)` returns the best resident ancestor.

### Procedural framework
- New `engine-procedural` crate: `Generator` trait + layer stack; pure, GPU-free, no dependency on planetary crates.
- Determinism: `output = f(stable_id, layer_seed, params)`; layer seeds from a fixed, versioned hash (not `DefaultHasher`); in-house integer-hash noise in f64 (no `noise` crate).
- Layer seed = hash(world_seed, layer_name, version) so adding/removing layers does not perturb others.

### Verification
- Golden-hash tests with fixed expected hashes at 1 and N worker threads; the Linux/macOS/Windows CI matrix is the cross-platform check (only Windows verified locally; hosted CI has not run yet).
- Float determinism: avoid transcendentals in hashed paths where possible; hash quantized results where used; document tolerance in an ADR (0008).
- Synthetic slow generator under a simulated frame loop asserts frame budget holds and coarse data is served; no wall-clock-only assertions.

### Claude's Discretion
Names, error types, module layout, hash function choice (fixed and versioned), test structure, ADR numbering (next free number).

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- Phase 2 `JobSystem` (priority queue, CancelToken, single-worker deterministic test mode); Phase 4 `ResidencyManager` (states, budgets, per-frame upload cap, LRU), `AsyncLoader`, `AssetId`/`Handle<T>`; engine-math `WorldPos`.

### Established Patterns
- Rust 1.99.0, edition 2024, unsafe forbid, explicit versions in [workspace.dependencies], cargo-deny wildcards deny, xtask `LAYERS` table + `docs_policy` tests must be updated for new crates, ADRs in docs/architecture/adr, CI matrix Linux/macOS/Windows plus the planetary-absent canary (the new crates are core, not planetary).

### Integration Points
- engine-streaming builds on engine-jobs + engine-assets; engine-procedural is a leaf (math-level). Phase 7 planetary extension will implement StreamKey/Generator for patches.

</code_context>

<specifics>
## Specific Ideas

Golden values pinned in tests so any change to hashing/noise is a deliberate, versioned break.

</specifics>

<deferred>
## Deferred Ideas

Software-adapter GPU CI (Phase 8); planetary patch keys and terrain generators (Phase 7); on-disk caching of generated data.

</deferred>
