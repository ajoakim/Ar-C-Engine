---
phase: 06-streaming-procedural-framework
plan: 04
subsystem: procedural
tags: [determinism, golden-hash, generator, layers]
requires: [06-02]
provides: [Generator, Params, LayerStack, LayerOutput, ProcError, hash_values, golden suite]
affects: [06-05, 06-06]
key-files:
  created:
    - crates/engine-procedural/src/error.rs
    - crates/engine-procedural/src/generator.rs
    - crates/engine-procedural/tests/golden.rs
  modified:
    - crates/engine-procedural/src/lib.rs
requirements-completed: [PROC-01, PROC-02]
completed: 2026-11-25
---

# Phase 6 Plan 04: Generator API and Golden Hashes Summary

Pure `Generator` layers composed in a `LayerStack` (seed = `layer_seed(world_seed, name, version)`), with canonical, NaN-rejecting output hashing and a golden-hash suite proven identical across 1/2/4/8 std threads and forward/reversed/shuffled order.

## Commits
- 8b791ae: Generator trait, Params, LayerStack, LayerOutput, ProcError (8 `generator_*` unit tests)
- edf8643: tests/golden.rs with pinned constants (6 `golden_*` tests)

## Pinned constants (Windows x86_64)
GOLDEN_ALL 0x9c6423c0be898ef7, GOLDEN_BASE 0xaba82342011c3f1c, GOLDEN_DETAIL 0xc38847391ba9cd5d, GOLDEN_MASK 0xf50077ddf6edf6ff.

Linux/macOS confirmation of these constants awaits hosted CI (the same constants run on the matrix).

## Deviations from Plan
None. Unit tests and implementation were committed together rather than as separate RED/GREEN commits.

## Self-Check: PASSED
cargo test, clippy -D warnings and banned_ops pass for engine-procedural.
