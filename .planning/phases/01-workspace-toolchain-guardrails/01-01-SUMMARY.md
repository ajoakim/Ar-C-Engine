---
phase: 01-workspace-toolchain-guardrails
plan: 01
subsystem: workspace
tags: [cargo, workspace, toolchain]
requires: []
provides: [pinned toolchain 1.99.0, 13-crate tiered workspace]
affects: [01-02, 01-03]
key-files:
  created: [rust-toolchain.toml, Cargo.toml, Cargo.lock, .gitignore, .gitattributes, rustfmt.toml, crates/*, extensions/*]
requirements-completed: [WS-01, WS-02]
completed: 2026-11-25
---

# Phase 1 Plan 01: Workspace and Toolchain Summary

Pinned Rust 1.99.0 (MSRV 1.95, edition 2024, resolver 3) with a 13-crate Cargo workspace whose path-dependency edges follow the tier table.

## Tasks
1. Toolchain pin and workspace root config - 77574b3
2. 13 crates with tiered deps, Cargo.lock - see git log (feat(01-01))

## Verification
build, fmt --check, clippy -D warnings, test all pass on 1.99.0; engine-sdk tree contains engine-render-wgpu; engine-core tree has no planetary.

## Deviations from Plan
None - plan executed exactly as written.

## Self-Check: PASSED
