---
phase: 01-workspace-toolchain-guardrails
plan: 03
subsystem: tooling
tags: [xtask, cargo_metadata, dependency-direction, guardrails]
requires: [01-01, 01-02]
provides:
  - "cargo xtask check-deps enforcing rules R1-R6 from a single layer table"
  - "Negative fixture workspaces proving the checker fails on violations"
  - "Workspace metadata and docs/ADR policy tests"
affects: [01-04]
tech-stack:
  added: [cargo_metadata 0.23.1]
  patterns: [no-deps manifest analysis, fixture workspaces excluded from root]
key-files:
  created:
    - xtask/src/layers.rs
    - xtask/src/check.rs
    - xtask/src/main.rs
    - xtask/src/lib.rs
    - xtask/tests/fixtures_negative.rs
    - xtask/tests/workspace_policy.rs
    - xtask/tests/docs_policy.rs
    - xtask/fixtures/core-depends-on-ext
    - xtask/fixtures/wgpu-winit-misplaced
    - .cargo/config.toml
  modified: [Cargo.toml, Cargo.lock]
decisions:
  - "Extension family = second path component under extensions/ (conventional-3d, planetary)"
metrics:
  tasks: 3
  completed: 2026-11-25
---

# Phase 1 Plan 3: xtask dependency-direction guardrail Summary

In-repo `xtask` crate whose `check-deps` command enforces rules R1-R6 via `cargo_metadata --no-deps`, backed by two violating fixture workspaces that must fail and policy tests for toolchain, edition, MSRV, docs and ADRs.

## Commits
- bd946f2: xtask rule engine, layer table, CLI, alias, Cargo.lock
- fc98b6d: fixtures, negative tests, workspace policy test
- (task 3): docs_policy tests

## Results
- `cargo xtask check-deps` prints `check-deps: OK (14 workspace packages checked)`, exit 0.
- core-depends-on-ext fixture: exit=1, emits `core-depends-on-extension` (plus `tier-violation`).
- wgpu-winit-misplaced fixture: exit=1, emits `wgpu-outside-allowed-crate` (engine-core) and `winit-outside-allowed-crate` (engine-world, dev-dep); engine-render-wgpu not flagged.
- Full suite green: fmt check, clippy -D warnings, `cargo test --workspace` (8 unit, 3 negative, 2 policy, 3 docs tests), check-deps.
- Fixture target/ and Cargo.lock are gitignored; none added.

## Deviations from Plan
None. Plan executed as written. (`cargo fmt` reformatted the new files before commit.)

## Known Stubs
None.

## Self-Check: PASSED
