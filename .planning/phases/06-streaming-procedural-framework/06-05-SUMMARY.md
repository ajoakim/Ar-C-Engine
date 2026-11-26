---
phase: 06-streaming-procedural-framework
plan: 05
subsystem: streaming
tags: [streaming, pins, lru, resolve, fallback]
requires: [06-03]
provides:
  - "StreamManager::resolve / pin_count / evict, pin-aware make_room"
  - "Pins refcount table, Resolved<K>, StreamError::Pinned, FrameReport.evicted"
affects: [engine-streaming]
tech-stack:
  patterns: ["single removal point retire() unpins", "FULL candidate list filtered before take", "gate closed during update, opened just before settle"]
key-files:
  created:
    - crates/engine-streaming/src/pins.rs
    - crates/engine-streaming/tests/slow_load.rs
    - crates/engine-streaming/tests/pins_lru.rs
  modified:
    - crates/engine-streaming/src/manager.rs
    - crates/engine-streaming/src/config.rs
    - crates/engine-streaming/src/error.rs
    - crates/engine-streaming/src/lib.rs
key-decisions:
  - "Eviction filters pinned, in-flight, awaiting-upload and recently shown (frame, frame-1) ids from the full eviction_candidates(u64::MAX) list"
  - "If make_room cannot free enough, the pending request is re-queued (back-pressure), never an error"
requirements-completed: [STRM-02]
duration: n/a
completed: 2026-11-25
---

# Phase 6 Plan 05: Pinned fallback and LRU eviction Summary

StreamManager now serves the best GpuResident ancestor via `resolve` while a refinement loads, pins that ancestor for the life of each in-flight request, and evicts only unpinned, settled, not-recently-shown data under budget pressure.

## Commits
- 46e49ce feat(06-05): pins, resolve, evict, pin-aware make_room (Task 1)
- 021bf5a test(06-05): slow_load.rs and pins_lru.rs (Task 2)

## Tests
- slow_load: 5 tests (parent served until child resident incl. pin_count, grandchild two-level fallback, none-resident, caps hold per frame with identical report vectors across two runs, update never waits over 500 frames)
- pins_lru: 7 tests (pinned parent survives pressure with explicit frames 3/4/5 then 10, back-pressure, explicit evict refuses pinned, unpin on failure/panic/cancel/token, recently shown protection)
- Full crate suite run 6 times consecutively: no flakiness. clippy -D warnings and fmt clean. No sleep/Instant.

## Deviations from Plan
- Plan-review fixes applied: slow-load uses update, settle, update to make P resident before requesting the gated child; pressure test states frames explicitly (P shown 3, A 4, B 5, request at 10).
- `recently_shown` test uses frame f / f+1 / f+2 (R shown at 5; protected at 6, evicted at 7) because resolve touches last_used, so a protected entry cannot also be strictly LRU alongside an unprotected one via the public API.
- Added `start_job` and `class_used` private helpers in manager.rs (needed to retry loader.start after make_room).

Known limitation: an `Err` from make_room (residency invariants) inside the start path returns early without unpinning; such errors are not expected in normal operation.

## Known Stubs
None.

## Self-Check: PASSED
