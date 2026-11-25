---
phase: 02-foundation-core-jobs-diagnostics-precision
plan: 06
subsystem: engine-core
tags: [engine, lifecycle, diagnostics, modules]
requires: [02-03, 02-05]
provides: [Engine driver, EngineError, lifecycle example]
affects: [engine-core]
tech-stack:
  added: []
  patterns: [guarded lifecycle returning EngineError, span-timed frame ticks]
key-files:
  created:
    - crates/engine-core/src/engine.rs
    - crates/engine-core/tests/engine_lifecycle.rs
    - crates/engine-core/examples/lifecycle.rs
  modified:
    - crates/engine-core/src/error.rs
    - crates/engine-core/src/lib.rs
decisions:
  - "Tests allow clippy::result_large_err (EngineError embeds ron errors, cold path), mirroring lib.rs"
metrics:
  completed: 2026-11-25
requirements: [CORE-01, CORE-02, CORE-05]
---

# Phase 2 Plan 06: Engine Driver Summary

Engine driver composing config, module registry, guarded lifecycle and FrameClock: tick emits `engine_frame` and per-module `module_update` spans and returns FrameStats.

## Tasks
1. Engine + EngineError (4f72b1c): 5 unit tests; every out-of-phase call returns an error.
2. Integration tests and example (0388f1c): 7 `engine_lifecycle_*` tests; `examples/lifecycle.rs` prints `engine phase` log lines and per-frame stats.

## Deviations from Plan
- [Rule 3] Added `#![allow(clippy::result_large_err)]` to the integration test file (clippy -D warnings failed on `Result<(), EngineError>` test signatures).
- [Rule 3] `cargo fmt --check` was failing on pre-existing drift in engine-diagnostics, engine-jobs and engine-math; formatted and committed separately (34ba588, style only).

## Verification
cargo test --workspace, clippy -D warnings, fmt --check, xtask check-deps all pass. `cargo deny check`: advisories/bans/licenses/sources ok, with a pre-existing warning about an unmatched `winit` wrapper in deny.toml.

## Known Stubs
None.

## Self-Check: PASSED
