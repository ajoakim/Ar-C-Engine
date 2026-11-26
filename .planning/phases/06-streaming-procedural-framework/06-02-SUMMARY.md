---
phase: 06-streaming-procedural-framework
plan: 02
subsystem: engine-procedural
tags: [hash, seed, noise, determinism]
requires: [06-01]
provides: [splitmix64, fnv1a64, Fnv1a64, hash2, hash3, unit_f64, canonical_bits, StableId, layer_seed, value_noise_2d, value_noise_3d, fbm_3d]
affects: [06-04]
key-files:
  created:
    - crates/engine-procedural/src/hash.rs
    - crates/engine-procedural/src/seed.rs
    - crates/engine-procedural/src/noise.rs
    - crates/engine-procedural/tests/banned_ops.rs
  modified:
    - crates/engine-procedural/src/lib.rs
decisions:
  - "Golden fbm_3d(.., 4) bits pinned for 4 samples; changing them requires a HASH_VERSION bump"
metrics:
  completed: 2026-11-25
---

# Phase 6 Plan 02: Hash, seed and noise primitives Summary

Versioned splitmix64/fnv1a64 hashing, order-independent `layer_seed`, and exact-arithmetic value noise/fBm, pinned by known-answer vectors, golden bits and a banned-identifier source scan.

## Commits
- e7251b1: hash.rs, seed.rs (KATs, seed isolation)
- Task 2 commit: noise.rs, banned_ops.rs

## Verification
`cargo test -p engine-procedural`: 15 unit + 1 integration tests pass; clippy `-D warnings` and `cargo fmt --check` clean. All published KATs (splitmix64, fnv1a64, layer_seed) matched.

## Deviations from Plan
- [Rule 1] Clippy `identity_op` rejected `s ^ 0` in the one-octave test; written as `s ^ u64::from(0u32)` (same value).

## Known Stubs
None.

## Self-Check: PASSED
