---
phase: 02-foundation-core-jobs-diagnostics-precision
plan: 03
subsystem: diagnostics
tags: [tracing, tracing-subscriber, frame-timing]
requires: [02-01]
provides: [engine_diagnostics::init, FrameTimingLayer, FrameClock, FrameStats]
affects: [engine-core (02-05)]
tech-stack:
  added: []
  patterns: [custom tracing Layer, scoped with_default in unit tests, isolated global-init test binary]
key-files:
  created:
    - crates/engine-diagnostics/src/error.rs
    - crates/engine-diagnostics/src/layer.rs
    - crates/engine-diagnostics/src/frame_stats.rs
    - crates/engine-diagnostics/src/init.rs
    - crates/engine-diagnostics/tests/global_init.rs
  modified:
    - crates/engine-diagnostics/src/lib.rs
requirements-completed: [CORE-05]
duration: ~10 min
completed: 2026-11-25
---

# Phase 2 Plan 03: Diagnostics Summary

Tier-0 diagnostics: a span-timing tracing Layer, a bounded-history FrameClock producing FrameStats, and a non-panicking global `init()`.

## Commits
- 8535b3e: FrameTimingLayer, FrameClock, FrameStats (Task 1)
- b440546: init() and global_init integration test (Task 2)

## Verification
`cargo test -p engine-diagnostics` (9 unit + 1 integration pass), clippy `-D warnings` clean, `cargo xtask check-deps` OK.

## Deviations from Plan
None. Both tasks were implemented together and then committed as two commits (Task 1 with a stub init.rs, then Task 2).

## Known Stubs
None.

## Self-Check: PASSED
