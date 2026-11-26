---
phase: 06-streaming-procedural-framework
verified: 2026-11-25T00:00:00Z
status: human_needed
score: 4/4 roadmap criteria verified locally (SC4 cross-platform half pending)
gaps: []
human_verification:
  - test: "Run CI on Linux and macOS (and Windows) and confirm golden hashes in crates/engine-procedural/tests/golden.rs (GOLDEN_ALL/BASE/DETAIL/MASK) and the streamed procedural test match"
    expected: "Identical golden hashes on all three platforms"
    why_human: "Hosted CI has never run (GitHub billing); only Windows verified locally. PROC-02 cross-platform equality is unproven."
---

# Phase 6: Streaming & Procedural Framework Verification

**Goal:** Data can be generated and streamed asynchronously and deterministically without ever stalling rendering.
**Re-verification:** No (initial)

## Truths

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Requests carry key, priority, budget class, cancel token; cancel frees budget | VERIFIED | `StreamRequest` in engine-streaming/src/request.rs has all four fields. `StreamManager::cancel` and `cancel_in_flight` release residency. Tests in tests/cancel.rs cover pending, generating, cpu-ready, token-on-next-update and failed producer. |
| 2 | Slow load keeps frame caps; coarse parent shown until refinement resident | VERIFIED | `resolve()` walks `parent()` to the nearest GpuResident ancestor. Tests in tests/slow_load.rs: gate-driven parent fallback, two-level fallback, caps per frame, update never waits. No Instant/SystemTime/sleep in engine-streaming/src (caps are counts per frame). |
| 3 | Layers are pure functions of stable ID + seed; independent layer seeds | VERIFIED | `layer_seed(world_seed, name, version)` in seed.rs with known-answer and isolation tests. Generator/LayerStack in generator.rs. |
| 4 | Golden hashes identical across thread counts/orders; no libm/usize/HashMap in hashed paths | VERIFIED locally / cross-platform UNVERIFIED | tests/golden.rs checks fixed goldens at 1/2/4/8 threads and orders. `canonical_bits` hashes f64 bits (-0 normalised, non-finite rejected). tests/banned_ops.rs scans for sin/cos/powf/mul_add/HashMap/DefaultHasher/`as f32`. usize appears only in lengths, not hashed. Streamed determinism test at 1 vs 4 workers exists (tests/procedural_stream.rs). Linux/macOS equality not run. |

## Checks Run

- `cargo test -p engine-streaming -p engine-procedural --locked`: all pass (23+8+5+... no failures)
- `cargo test -p xtask --locked`: all pass
- `check-deps`: OK (16 packages); `check-canary`: OK

## Requirements

STRM-01, STRM-02, PROC-01: satisfied. PROC-02: satisfied on Windows; cross-platform half needs human/CI confirmation.

## Anti-patterns

None found in the inspected sources (no TODO stubs, no wall-clock).

## Note

ROADMAP.md still shows the phase 6 plans unchecked ("[ ]"). This is a bookkeeping item for the orchestrator, not a code gap.

_Verifier: Claude (gsd-verifier)_
