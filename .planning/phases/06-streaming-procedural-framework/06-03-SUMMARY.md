---
phase: 06-streaming-procedural-framework
plan: 03
subsystem: streaming
tags: [streaming, residency, cancellation, priority-queue]
requires: [06-01]
provides: [StreamManager, StreamKey, StreamRequest, StreamSource, Uploader, StreamConfig, FrameReport, StreamError]
affects: [06-05]
key-files:
  created:
    - crates/engine-streaming/src/key.rs
    - crates/engine-streaming/src/request.rs
    - crates/engine-streaming/src/source.rs
    - crates/engine-streaming/src/config.rs
    - crates/engine-streaming/src/error.rs
    - crates/engine-streaming/src/manager.rs
    - crates/engine-streaming/tests/common/mod.rs
    - crates/engine-streaming/tests/request.rs
    - crates/engine-streaming/tests/cancel.rs
  modified:
    - crates/engine-streaming/src/lib.rs
decisions:
  - "Queue overflow rejects with QueueFull; CPU OverBudget keeps the request pending (back-pressure)"
  - "Tests gate producers with an atomic release level; no sleep or Instant"
requirements-completed: [STRM-01, STRM-02]
duration: n/a
completed: 2026-11-25
---

# Phase 6 Plan 03: Streaming Request Pipeline Summary

Generic StreamManager over ResidencyManager + AsyncLoader: bounded priority queue (descending priority, FIFO within), reject-on-full, cancellation that frees budget immediately, and a non-blocking update with counted caps on starts, completions and upload bytes.

## Tasks
1. Contract types (key, request, source, uploader, config, error, report): commit 0a89eb7.
2. StreamManager plus request/cancel tests (8 request tests, 5 cancel tests): commit 7bae280.

## Verification
- `cargo test -p engine-streaming`: 13 tests pass, repeated 8+ times with no failures; `request_caps_hold_per_frame` run 3 more times stable.
- clippy `--all-targets -D warnings` clean; fmt clean.
- `wait_all` only inside `settle`; no sleep/Instant in src or tests.

## Deviations from Plan

**1. [Rule 1 - Test determinism] Gated producers instead of bare settle**
- Plan-review fix applied: `settle()` after every `update` in `request_caps_hold_per_frame`. That alone was still racy, because `update`'s own poll can see a job started in the same update finish. Producers are therefore gated at level 1: the gate is closed (store 0) during `update` and opened (store 1) before `settle`. Added `GatedSource::gated_at`. Same gating used in `cancel_cpu_ready_evicts` and `cancel_failed_producer_releases` (the failure return was moved after the gate wait).
- `cancel_cpu_ready_evicts` no longer assumes which key is applied first: completion order follows asset id order, so the test picks the deferred key dynamically.

**2. [Rule 2] Extras**: added `StreamManager::frame()` getter (avoids dead field) and `request_invalid_config_rejected` test.

## Known Stubs
None.

## Self-Check: PASSED
