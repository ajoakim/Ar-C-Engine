---
phase: 05-conventional-reference-app-canary
plan: 01
subsystem: dependencies
tags: [gltf, image, cargo-deny, adr]
requires: []
provides: [gltf/image workspace pins, deny wrappers, ADR-0007]
affects: [conventional-3d]
key-files:
  modified: [Cargo.toml, Cargo.lock, deny.toml, extensions/conventional-3d/Cargo.toml, docs/architecture/dependency-direction.md, docs/architecture/adr/README.md]
  created: [docs/architecture/adr/0007-lit-rendering-gltf-import-and-canary.md]
requirements-completed: [REND-05, PLAN-01]
completed: 2026-11-26
---

# Phase 5 Plan 01: Dependency pins, cargo-deny confinement and ADR-0007 Summary

gltf 1.4.1 and image 0.25.10 are pinned in the workspace, confined to conventional-3d by cargo-deny wrappers, and the Phase 5 decisions are recorded in ADR-0007.

## Tasks
1. Pins, manifest, deny wrappers (commit 35d93d2). `cargo check --workspace --locked`, `cargo xtask check-deps` and `cargo deny check bans licenses sources advisories` all pass. The license allow-list is unchanged.
2. ADR-0007 and ADR index row (see git log).

## Deviations
- Also updated "Third-party isolation" in docs/architecture/dependency-direction.md to mention gltf and image (requested by caller).
- Did not run the negative deny fixture check or the xtask docs_policy tests (the plan says 05-02 runs the latter).

## Self-Check: PASSED
