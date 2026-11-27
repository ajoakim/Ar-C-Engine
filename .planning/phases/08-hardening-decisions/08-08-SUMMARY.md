---
phase: 08-hardening-decisions
plan: 08
subsystem: render, sdk, planetary-app
tags: [egui-wgpu, debug-overlay, sdk-hook, f3]
requires:
  - phase: 08-03
    provides: image_diff harness, software-adapter goldens
  - phase: 08-06
    provides: debug-ui crate, DiagnosticsSnapshot, planetary snapshot builder
  - phase: 08-07
    provides: egui-wgpu R2 gate, ADR-0018
provides:
  - engine-render-wgpu feature egui-overlay (WgpuBackend::set_overlay, OverlayData)
  - engine-sdk feature egui-overlay (App::overlay_frame, Runtime::frame_with)
  - planetary-app debug-overlay feature with F3 toggle
affects: [08-12]
key-files:
  created:
    - crates/engine-render-wgpu/src/overlay.rs
    - crates/engine-render-wgpu/tests/golden/windows/overlay_corner.png
    - crates/engine-sdk/src/overlay.rs
    - crates/engine-sdk/tests/overlay_hook.rs
    - extensions/planetary/planetary-app/tests/overlay_toggle.rs
  modified:
    - crates/engine-render-wgpu/{Cargo.toml,src/backend.rs,src/lib.rs,tests/image_diff.rs}
    - crates/engine-sdk/{Cargo.toml,src/app.rs,src/lib.rs,src/run.rs,src/runtime.rs}
    - crates/engine-platform/src/input.rs
    - extensions/planetary/planetary-app/{Cargo.toml,src/lib.rs,examples/planetary_flight.rs}
    - Cargo.toml
    - docs/architecture/adr/0018-debug-overlay-placement-and-font-licences.md
requirements-completed: []
completed: 2026-11-27
---

# Phase 8 Plan 08: egui-wgpu painter, SDK hand-off, F3 overlay Summary

The debug overlay is wired end to end behind off-by-default features: an egui-wgpu painter in `engine-render-wgpu`, a feature-gated `App::overlay_frame` hand-off in `engine-sdk`, and an F3 read-only toggle in `planetary-app`. QA-02 is implemented but NOT marked complete: plan 12's human flight must confirm it, and no windowed look at the overlay was taken here.

## Commits
- d16b6b2 feat(08-08): egui-wgpu overlay painter and image test
- 8f27201 feat(08-08): feature-gated App::overlay_frame hand-off
- 59b9d96 feat(08-08): planetary-app F3 overlay wiring
- docs commit: ADR-0018 note, this summary, ROADMAP/STATE

## Design
- Painter: `WgpuBackend::set_overlay(OverlayData)` stages tessellated egui output; `render` records the overlay pass (`LoadOp::Load`, same encoder, before present), so it works for offscreen and surface targets and the non-feature path is unchanged (existing goldens bit-identical, max delta 0). The staged texture deltas merge if a frame is never rendered, and are cleared on drop (egui panics on unapplied deltas). Empty `OverlayData` is a no-op. `render` now takes `&mut self` (painter needs mutable state).
- SDK hand-off: `App::overlay_frame() -> Option<DiagnosticsSnapshot>` (default `None`). The SDK owns the egui context, runs `debug_ui::frame_shapes` (two passes, textures delta applied) and stages the result via `Runtime::frame_with`, a hook run just before submit. `FramePacket` and `NullBackend` untouched. The plan's `OverlayFrame` type was dropped: returning the snapshot keeps egui entirely out of app code.
- sdk_only guard (APP-02): unchanged, not weakened. `planetary-app` feature `debug-overlay = ["engine-sdk/egui-overlay"]` adds no `[dependencies]` entry and no egui/debug-ui/engine_diagnostics names; it only builds the snapshot (`DiagnosticsSnapshot` is re-exported by engine-sdk from plan 06). debug-ui and egui are optional deps of engine-sdk (tier 5 may depend on tier 4).
- Layer table: `engine-render-wgpu` does not dev-depend on `debug-ui`; the image test builds shapes with raw egui (rect plus "ab" text in the 32x32 corner).

## Results
- Image test `overlay_corner_differs` (ARC_REQUIRE_ADAPTER=1, WARP): corner pixels differ, every pixel outside the corner identical to the baseline, empty overlay identical to baseline, golden compare passes with ARC_REQUIRE_GOLDENS=1. Windows golden `overlay_corner.png` generated locally on WARP (I viewed it: yellow square with "ab" over the triangle). No Linux/macOS golden: skipped there unless required, as for plan 03 goldens; CI regeneration remains plan 11.
- A1 (sRGB target): egui-wgpu logs a warning for sRGB formats but picks its sRGB-aware entry point; colours look right on WARP.
- Unit/integration: engine-sdk (default and `--features egui-overlay`), planetary-app `overlay_toggle` (2) with `--features debug-overlay`, engine-platform F3 mapping.
- Gates: check-deps OK (20 packages) plus all four fixtures exit 1; check-canary OK (310 packages, none planetary) plus canary-leak negative exit 1; `cargo deny check` advisories/bans/licenses/sources ok, and the unused `egui-wgpu` wrapper warning is gone (only duplicate-crate warnings remain); docs_policy 17 pass; `cargo fmt --check` clean; clippy `-D warnings` clean for `--workspace --all-targets` and for planetary-app `--features debug-overlay`, engine-render-wgpu and engine-sdk `--features egui-overlay`.
- `cargo test --workspace --locked`: 890 passed, 0 failed, 11 ignored (888 in plan 07 plus engine-platform F3 test and engine-sdk overlay_hook default test). No flaky tests. Feature runs: planetary-app `--features debug-overlay` all pass (incl. sdk_only 14), engine-render-wgpu `--features egui-overlay` 18 + 10 image_diff pass.
- `planetary_flight --headless 30` with `--features debug-overlay`: 30 frames, 53 draws in last frame, 120 meshes; no window or F3 needed. `basic_scene` (conventional-3d) builds with no egui.

## Deviations from Plan
1. [Rule 3] `KeyCode::F3` did not exist; added it to `engine-platform` (non_exhaustive enum) with a mapping test. File not in the plan list.
2. [Design] Hand-off returns a `DiagnosticsSnapshot`, not an `OverlayFrame`, and the overlay is staged before submit via `Runtime::frame_with` rather than drawn after submit (the surface frame is presented inside submit, so a post-submit draw cannot reach it). The `draw_overlay` name became `set_overlay`.
3. [Rule 3] `debug-ui` added to `[workspace.dependencies]`; Cargo.lock updated.
4. Added an ADR-0018 consequence note describing the built hand-off (docs_policy still passes).
5. Plan said not to run the whole workspace suite; run anyway per the orchestrator instruction.

## Not run / open
- No windowed look at the overlay or real F3 key press (plan 12 human flight). Surface-target overlay is covered only by code path sharing with offscreen (`render` receives either view); not exercised on a real swapchain here.
- No Linux/macOS overlay goldens (plan 11 CI).

## Known Stubs
None.

## Self-Check: PASSED
