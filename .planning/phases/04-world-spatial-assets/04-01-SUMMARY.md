---
phase: 04-world-spatial-assets
plan: 01
subsystem: build-deps
tags: [cargo, slotmap, uuid, adr, spatial]
requires: []
provides: [pinned Phase 4 dependencies, ADR-0006]
affects: [engine-world, engine-spatial, engine-assets]
tech-stack:
  added: [slotmap 1.1.1, uuid 1.29.3 (v5 only)]
key-files:
  modified: [Cargo.toml, Cargo.lock, crates/engine-world/Cargo.toml, crates/engine-spatial/Cargo.toml, crates/engine-assets/Cargo.toml, docs/architecture/adr/README.md]
  created: [docs/architecture/adr/0006-spatial-index-hash-grid-first.md]
decisions:
  - "Spatial backend: uniform hash grid first, benchmark BVH/rstar/kiddo in Phase 8 (ADR-0006)"
metrics:
  completed: 2026-11-26
---

# Phase 4 Plan 01: Dependencies and ADR-0006 Summary

slotmap 1.1.1 and uuid 1.29.3 (v5 only) are pinned in the workspace, the three Phase 4 crates declare all their third-party deps, and ADR-0006 records the hash-grid-first spatial decision.

## Tasks

1. Pin deps and wire manifests: commit d685f56. A plain `cargo build --workspace` regenerated Cargo.lock, then `--locked` builds passed.
2. ADR-0006 and README index row: commit c1d471f.

## Verification

- `cargo build --workspace --locked`, `cargo xtask check-deps` (14 packages OK), clippy `-D warnings` on the three crates, and `cargo test --workspace --locked` all pass.
- `cargo deny check` (0.22.1 installed locally): advisories, bans, licenses and sources all ok.

## Deviations from Plan

None. The plan executed as written, with the Cargo.lock regeneration step done first as requested.

## Self-Check: PASSED
