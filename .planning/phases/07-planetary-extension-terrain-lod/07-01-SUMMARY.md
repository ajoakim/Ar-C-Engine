---
phase: 07-planetary-extension-terrain-lod
plan: 01
subsystem: sdk, workspace, docs
tags: [sdk-hook, scaffold, adr, xtask]
requires: []
provides:
  - "App::gpu_frame default no-op hook and Resources::destroy_mesh"
  - "planetary-terrain (tier 7) and planetary-app (tier 8) workspace crates"
  - "ADR-0009 (accepted), ADR-0010 (proposed)"
affects: [07-02, 07-05, 07-10]
tech-stack:
  added: []
  patterns: ["generic SDK hook, no planetary concept in core"]
key-files:
  created:
    - crates/engine-sdk/tests/gpu_frame.rs
    - extensions/planetary/planetary-terrain/Cargo.toml
    - extensions/planetary/planetary-terrain/src/lib.rs
    - extensions/planetary/planetary-app/Cargo.toml
    - extensions/planetary/planetary-app/src/lib.rs
    - docs/architecture/adr/0009-sdk-per-frame-gpu-hook.md
    - docs/architecture/adr/0010-terrain-lod-skirts-and-shared-border-sampling.md
  modified:
    - crates/engine-sdk/src/app.rs
    - crates/engine-sdk/src/runtime.rs
    - crates/engine-sdk/src/lib.rs
    - Cargo.toml
    - Cargo.lock
    - xtask/src/layers.rs
    - xtask/tests/docs_policy.rs
    - docs/architecture/dependency-direction.md
    - docs/architecture/adr/README.md
    - .github/workflows/ci.yml
key-decisions:
  - "ADR-0009 accepted; ADR-0010 proposed until 07-05/07-10 evidence"
metrics:
  completed: 2026-11-25
---

# Phase 7 Plan 01: SDK GPU hook, planetary crate scaffold, ADRs Summary

Generic `App::gpu_frame` per-frame GPU hook plus `Resources::destroy_mesh` in the SDK, two empty planetary crates wired into workspace/xtask/docs/CI, and ADR-0009/0010.

## Commits
- 32ec5b2 feat: SDK hook, destroy_mesh, MAX_DRAWS_PER_FRAME re-export, 7 tests
- 39ccf1a chore: scaffold planetary-terrain and planetary-app (members, lock, LAYERS, docs, CI excludes)
- 16ff407 docs: ADR-0009, ADR-0010, README rows, docs_policy tests

## Verification (scoped, per plan)
- `cargo test -p engine-sdk` (7 new gpu_frame tests plus existing), `-p conventional-3d`, `-p xtask` all pass.
- basic_scene headless: 5 frames, 10 draws.
- check-deps OK (18 packages), check-canary OK; canary sed simulation prints 0.
- clippy `-D warnings` on engine-sdk/xtask/conventional-3d and scoped `cargo fmt --check` pass.
- `cargo check -p planetary-terrain -p planetary-app` passed (planetary-core compiled at that moment).

## Deviations from Plan
- ADR date is 2026-11-25 (actual date) rather than the plan's 2026-11-26, per instruction.
- A rustfmt reflow of the `lib.rs` re-export list was folded into the first commit via amend.

## Known Stubs
New crates are intentionally empty (modules arrive in later plans).

## Self-Check: PASSED
