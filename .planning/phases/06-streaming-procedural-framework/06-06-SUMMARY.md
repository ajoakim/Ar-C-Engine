---
phase: 06-streaming-procedural-framework
plan: 06
subsystem: streaming-procedural
tags: [determinism, golden, jobs, adr]
requires: [06-04, 06-05]
provides:
  - streamed procedural golden (GOLDEN_STREAMED) across 1 and 4 JobSystem workers
  - ADR-0008 evidence table and limitations
affects: [phase-07]
key-files:
  created:
    - crates/engine-streaming/tests/procedural_stream.rs
  modified:
    - crates/engine-streaming/src/manager.rs
    - docs/architecture/adr/0008-deterministic-streaming-and-procedural-generation.md
    - .planning/phases/06-streaming-procedural-framework/06-VALIDATION.md
key-decisions:
  - "GOLDEN_STREAMED = 0x43c8_6516_7d16_b0fe (Windows x86_64); Linux/macOS pending hosted CI"
metrics:
  completed: 2026-11-25
---

# Phase 6 Plan 6: Streamed procedural determinism and phase sign-off Summary

Procedural output streamed through StreamManager on the real JobSystem is byte-identical at 1 and 4 workers, equal to direct LayerStack generation, independent of submission order and priority, and pinned to GOLDEN_STREAMED.

## Commits
- ba753c5 test(06-06): streamed procedural determinism test
- fix(06-06): unpin ancestor when make_room errors in start path
- docs(06-06): ADR-0008 evidence and validation sign-off

## Deviations from Plan

**1. [Rule 1 - Bug] Pin leak in start path** - the 06-05 known limitation (make_room Err left the ancestor pin held) was a trivial early-return unpin, so it was fixed in `crates/engine-streaming/src/manager.rs` (outside the plan's file list) and documented in ADR-0008 Limitations. The popped request is still dropped on that path (only reachable on residency invariant violations).

TestKey.depth is u8, so the golden hashes it via `write_u32(u32::from(depth))`.

## Gate results (Windows)
fmt --check, clippy --workspace --all-targets --locked -D warnings, test --workspace --locked, xtask check-deps, check-canary, cargo deny (bans licenses sources advisories) all pass. Cross-target `cargo check --workspace --all-targets --locked` passes for x86_64-unknown-linux-gnu and aarch64-apple-darwin. No manifest/lockfile changes since plan start.

## Known limitation
Golden constants are Windows-computed; Linux/macOS execution is pending hosted CI (GitHub billing). Validation row stays manual/pending. 06-VALIDATION.md signed off with nyquist_compliant true.

## Known Stubs
None.

## Self-Check: PASSED
