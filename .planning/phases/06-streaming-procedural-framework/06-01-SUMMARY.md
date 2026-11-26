---
phase: 06-streaming-procedural-framework
plan: 01
subsystem: workspace
tags: [streaming, procedural, workspace, xtask, adr]
requires: []
provides:
  - engine-procedural crate (tier 1) and engine-streaming crate (tier 3) as workspace members
  - engine_sdk::streaming and engine_sdk::procedural module re-exports
  - ADR-0008
affects: [06-02, 06-03, 06-04, 06-05, 06-06]
tech-stack:
  added: []
  patterns: [module-alias SDK re-exports]
key-files:
  created:
    - crates/engine-streaming/Cargo.toml
    - crates/engine-streaming/src/lib.rs
    - crates/engine-procedural/Cargo.toml
    - crates/engine-procedural/src/lib.rs
    - docs/architecture/adr/0008-deterministic-streaming-and-procedural-generation.md
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/engine-sdk/Cargo.toml
    - crates/engine-sdk/src/lib.rs
    - xtask/src/layers.rs
    - xtask/tests/docs_policy.rs
    - docs/architecture/dependency-direction.md
    - docs/architecture/adr/README.md
key-decisions:
  - "engine-procedural tier 1, engine-streaming tier 3; reject-on-full queue (QueueFull)"
  - "No new third-party crates"
requirements-completed: [STRM-01, STRM-02, PROC-01, PROC-02]
duration: ~15min
completed: 2026-11-25
---

# Phase 6 Plan 01: Scaffold Streaming and Procedural Crates Summary

Both Phase 6 crates are registered as workspace members with the SDK re-exporting them as modules, the layer table and docs updated, and ADR-0008 accepted. This plan owns all manifest, lockfile, xtask and SDK changes for the phase.

## Tasks and commits
1. Crates, workspace, SDK, Cargo.lock: 2ce1ab0
2. Layer table, dependency doc, docs policy test: f9c6189
3. ADR-0008 and README row: e66738b

## Deviations from Plan
- ADR date is 2026-11-25 (real date) instead of the plan's 2026-11-26, as instructed.
- Ran `cargo fmt` after Task 1, which reordered the SDK re-export lines (alphabetical); amended into the Task 1 commit.

## Verification
fmt --check, clippy -D warnings, test --workspace --locked, xtask check-deps, check-canary and cargo deny all pass. Cargo.lock gained 19 lines, no third-party packages.

## Known Stubs
engine-streaming only re-exports types and engine-procedural only has HASH_VERSION; later plans fill them in (intentional).

## Self-Check: PASSED
