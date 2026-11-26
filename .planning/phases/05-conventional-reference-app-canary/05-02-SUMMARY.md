---
phase: 05-conventional-reference-app-canary
plan: 02
subsystem: tooling
tags: [xtask, canary, cargo_metadata, architecture]
requires: []
provides:
  - "cargo xtask check-canary (transitive planetary reachability)"
  - "canary-leak negative fixture"
affects: [05-10]
key-files:
  created: [xtask/src/canary.rs, xtask/tests/canary.rs, xtask/fixtures/canary-leak/]
  modified: [xtask/src/lib.rs, xtask/src/main.rs, xtask/tests/docs_policy.rs, docs/architecture/dependency-direction.md]
requirements-completed: [PLAN-01]
duration: short
completed: 2026-11-25
---

# Phase 5 Plan 02: Canary Check Summary

Transitive `cargo xtask check-canary` walks the resolved cargo metadata graph (all dep kinds, no `--no-deps`) from `conventional-3d` and fails with rule `canary-planetary-reachable` plus the full path if any planetary crate is reachable.

## Results
- Real workspace: exit 0 (287 packages reached, none planetary).
- canary-leak fixture: exit 1, path `conventional-3d -> engine-sdk -> planetary-core`.
- Unknown package: exit 2. `check-deps` unchanged.
- 11 canary tests plus `canary_documented` docs-policy test; clippy and fmt clean.

## Commits
- eeb99e1: canary.rs, lib exports, fixture, tests (task 1)
- c99e662: CLI subcommand, docs, docs policy test (task 2)

## Deviations from Plan
None. Both tasks were written together, then committed as two commits. CLI tests live in xtask/tests/canary.rs, so they landed in the task 1 commit.

## Self-Check: PASSED
