---
phase: 08-hardening-decisions
plan: 06
subsystem: diagnostics, debug-ui
tags: [egui, diagnostics, overlay, deny]
requires:
  - phase: 08-05
    provides: workspace pins for egui and egui-wgpu
provides:
  - DiagnosticsSnapshot, Budget, Histogram (engine-diagnostics, tier 0, re-exported by engine-sdk)
  - debug-ui crate (tier 4): panel(), overlay(), frame_shapes()
  - planetary-app overlay_snapshot::build and PlanetaryApp::diagnostics_snapshot (feature debug-overlay)
  - deny.toml: OFL-1.1, Ubuntu-font-1.0 and egui-wgpu wrapper
affects: [08-07, 08-08]
key-files:
  created:
    - crates/engine-diagnostics/src/snapshot.rs
    - crates/debug-ui/Cargo.toml
    - crates/debug-ui/src/lib.rs
    - extensions/planetary/planetary-app/src/overlay_snapshot.rs
  modified:
    - deny.toml
    - Cargo.toml
    - Cargo.lock
    - xtask/src/layers.rs
    - docs/architecture/dependency-direction.md
    - crates/engine-sdk/src/lib.rs
    - extensions/planetary/planetary-app/Cargo.toml
    - extensions/planetary/planetary-app/src/lib.rs
requirements-completed: []
completed: 2026-11-27
---

# Phase 8 Plan 06: Diagnostics snapshot, debug-ui, planetary snapshot builder Summary

Planet-agnostic `DiagnosticsSnapshot` (gauges, budgets, histograms, frame/prepare ms), a headless-tested egui panel crate on pure `egui` (no wgpu, winit, egui-winit), and a planetary snapshot builder behind the off-by-default `debug-overlay` feature.

## Commits
- aa83f31 DiagnosticsSnapshot in engine-diagnostics
- 21e0ee3 deny.toml: OFL-1.1, Ubuntu-font-1.0 (comment names epaint_default_fonts), `egui-wgpu` wgpu wrapper; committed before egui entered the tree
- 3d93ec3 debug-ui crate, layers.rs tier-4 row, dependency-direction.md row
- 162db9a planetary snapshot builder
- b6517ae APP-02 fix (see deviations)

## Results
- `cargo deny check`: advisories, bans, licences, sources ok. The only licence-policy change is the two font licences. One benign warning: the `egui-wgpu` wrapper is not yet encountered (unused-wrapper), expected until the painter lands.
- check-deps OK (20 packages); check-canary OK (conventional-3d reaches 310 packages, none planetary).
- clippy `--workspace --all-targets -D warnings` clean, also `-p planetary-app --features debug-overlay`; `cargo fmt --check` clean; docs_policy passes.
- `cargo test --workspace --locked`: 883 passed, 0 failed. `cargo test -p planetary-app --features debug-overlay --locked`: all pass.

## Deviations from Plan
**1. [Rule 3 - Blocking] APP-02 guard (planetary-app/tests/sdk_only.rs)**: planetary-app may depend only on engine-sdk and planetary crates, and its sources may not name `engine_diagnostics`. The plan's direct `engine-diagnostics` and optional `debug-ui` dependencies failed three tests. Without weakening the guard: `engine-sdk` now re-exports `DiagnosticsSnapshot`, `Budget`, `Histogram`; planetary-app uses `engine_sdk::DiagnosticsSnapshot`. The feature is `debug-overlay = []` for now; the `dep:debug-ui` link is deferred to plan 08 (painter and F3 wiring), which must decide how the guard accommodates it.
**2. [Rule 1 - Bug] egui frame**: a new window is laid out invisibly on its first pass, so `frame_shapes` runs two passes and merges texture deltas; the returned `FrameShapes` carries `textures_delta` (egui panics if dropped unapplied).

## Known Stubs
None.

## Not run
Hosted CI, GPU rendering of the overlay (plan 08).

## Self-Check: PASSED
