---
phase: 04-world-spatial-assets
plan: 06
subsystem: assets
tags: [residency, budgets, upload-cap, lru]
requires: [04-03]
provides: [ResidencyState, ResidencyManager, ResidencyStats]
affects: [engine-assets]
key-files:
  created:
    - crates/engine-assets/src/residency.rs
    - crates/engine-assets/src/manager.rs
  modified:
    - crates/engine-assets/src/lib.rs
    - crates/engine-assets/src/error.rs
requirements-completed: [ASSET-02, ASSET-03]
duration: ~20min
completed: 2026-11-25
---

# Phase 4 Plan 06: Residency State Machine and Budgets Summary

Deterministic residency manager: 17-edge exhaustive transition table, per-class CPU/GPU budgets, per-frame upload cap with oversize-first exception, LRU eviction candidates and observable stats.

## Commits
- 6dd7412: ResidencyState transition table and residency errors
- 90673d0: ResidencyManager lifecycle, budgets, upload cap
- fe95850: LRU eviction candidates, stats, verify_accounting

## Deviations from Plan

**[Rule 1 - Bug] Transition check too permissive for multi-source targets**
- Found during: Task 3 (random-walk accounting test, step 3242)
- Issue: `finish_generating`, `abort_upload` and `finish_evict` used only `can_transition`, which also allows e.g. Uploading -> CpuReady via finish_generating, leaving GPU bytes held in CpuReady.
- Fix: added `check_from` requiring the exact source state as well.
- Commit: fe95850

Also: `cargo fmt` reformatted unrelated files (handle.rs, id.rs, registry.rs); those were reverted and not committed.

## Verification
`cargo test -p engine-assets` (33 tests pass) and `cargo clippy -p engine-assets --all-targets -- -D warnings` clean.

## Known Stubs
None.

## Self-Check: PASSED
