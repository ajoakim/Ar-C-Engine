---
phase: 08-hardening-decisions
plan: 07
subsystem: policy, xtask
tags: [xtask, deny, adr, egui-wgpu]
requires:
  - phase: 08-06
    provides: debug-ui crate; deny.toml font licences and egui-wgpu wrapper
provides:
  - xtask R2 gating of direct egui-wgpu dependencies to engine-render-wgpu
  - egui-wgpu-misplaced negative fixture and CI negative step
  - ADR-0018 (overlay placement, font-licence policy)
affects: [08-08]
key-files:
  created:
    - xtask/fixtures/egui-wgpu-misplaced/
    - docs/architecture/adr/0018-debug-overlay-placement-and-font-licences.md
  modified:
    - xtask/src/check.rs
    - xtask/tests/fixtures_negative.rs
    - xtask/tests/docs_policy.rs
    - docs/architecture/adr/README.md
    - docs/architecture/dependency-direction.md
    - .github/workflows/ci.yml
requirements-completed: []
completed: 2026-11-27
---

# Phase 8 Plan 07: Overlay policy gates Summary

xtask rule R2 now rejects any direct `egui-wgpu` dependency outside `engine-render-wgpu`, proven by a unit test, a negative fixture (exit 1) and a CI negative step; ADR-0018 records the overlay placement and the user-approved font-licence policy change.

## Commits
- Task 1: xtask R2 extension, fixture, CI step (feat(08-07))
- Task 2: ADR-0018, README row, dependency-direction text, docs_policy tests (docs(08-07))

## Results
- check-deps OK (20 packages). `egui-wgpu-misplaced` fixture exits 1 with `wgpu-outside-allowed-crate: debug-ui -> egui-wgpu`.
- check-canary OK (310 packages, none planetary).
- `cargo deny check`: advisories, bans, licenses, sources ok. One expected warning: the `egui-wgpu` wrapper is unused until plan 08 adds the painter (documented in ADR-0018 as a transitional state). deny.toml was not edited.
- xtask tests and docs_policy pass; clippy `--workspace --all-targets -D warnings` clean; `cargo fmt --check` clean.
- `cargo test --workspace --locked`: 888 passed, 0 failed (883 before plus 5 new: 1 rule unit test, 2 fixture tests, 2 docs_policy tests). No flaky tests.

## Deviations from Plan
- The `egui-wgpu` violation reuses `Rule::WgpuOutsideAllowedCrate` (dependency field `egui-wgpu`) rather than a new rule id, so the CLI id and docs list stay unchanged.
- The fixture is a small workspace (debug-ui violating, engine-render-wgpu allowed, vendored stand-in `egui-wgpu`) mirroring `wgpu-winit-misplaced`, rather than a single `src/lib.rs`.
- No cargo-deny negative step added for the fixture (optional in the plan; the xtask rule is the gate).

## Known Stubs
None.

## Not run
Hosted CI; QA-02 left unmarked (shared with plan 08).

## Self-Check: PASSED
