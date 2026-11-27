---
phase: 09-overlay-ci-test-hygiene
plan: 03
subsystem: debug-overlay
tags: [egui, overlay, hidpi, formatting]
requires: [09-01, 09-02]
provides: [integer count formatting, format-keyed egui painter, scale-aware overlay host]
affects: [debug-ui, engine-render-wgpu, engine-sdk, engine-platform]
key-files:
  modified:
    - crates/debug-ui/src/lib.rs
    - crates/engine-render-wgpu/src/overlay.rs
    - crates/engine-render-wgpu/src/backend.rs
    - crates/engine-sdk/src/overlay.rs
    - crates/engine-sdk/src/run.rs
    - crates/engine-platform/src/window.rs
metrics:
  completed: 2026-11-27
---

# Phase 9 Plan 03: Overlay counts, painter rebuild, scale factor Summary

Gauges/budgets print integers (`selected: 593`), times keep two decimals; the egui painter rebuilds when the colour format changes; the overlay host lays out in points using the window scale factor.

## Commits
- 151cce2 debug-ui: fmt_count/fmt_ms, `scale_factor` parameter on frame_shapes
- 9ab1930 engine-render-wgpu: `renderer_format`, `needs_rebuild`, `take_overlay_renderer_rebuilt`
- 57eabeb engine-sdk host scale factor + `reset_atlas`; `PlatformWindow::scale_factor`

## Decisions
- Rebuild frame (plan-checker a): the frame that rebuilds the renderer applies its texture deltas and frees but skips the paint pass (`skip_paint`), so no draw references a missing font texture. The host sees `take_overlay_renderer_rebuilt()` next frame, resets its egui Context, and re-sends the atlas. I did not rely on egui-wgpu tolerating a missing texture id.
- Text testing (b): panel text is built by small helpers `header_text` and `gauge_text`, used by `panel()`; tests assert `"selected: 593"` and `"frame 8.30 ms  prepare 1.20 ms"` directly, no vacuous test.
- Stale label (c): test snapshot now uses `held meshes (base+detail)`.
- Scale (d): `self.window` is engine-platform's `PlatformWindow`, which did NOT expose `scale_factor()`; added a one-line `scale_factor()` accessor (winit stays confined to engine-platform).
- Device recreation: backend has no device-recreation path (DeviceLost is only an error) and `color_format` is never reassigned after construction, so nothing speculative was added; the rebuild logic is ready for a format change.

## Deviations
- [Rule 3] Added `PlatformWindow::scale_factor` (window.rs, outside the plan's five files) to make run.rs wiring possible.
- Plan test expectation corrected: egui tessellation yields vertices in points (the painter multiplies by ppp), so the scale test asserts ppp == 2.0 and identical point layout for the same point-size screen, rather than 2x vertex x.

## Verification
- debug-ui: 6 tests pass; clippy clean.
- engine-render-wgpu (egui-overlay): 19 lib tests, 12 image_diff tests (overlay_corner golden unchanged) pass; clippy clean.
- engine-sdk (egui-overlay): all pass; clippy clean for engine-sdk/engine-platform; planetary-app overlay_toggle passes.
- Not run here: check-deps / cargo deny (plan 04 gate); no new dependency edges were added.

## Bookkeeping
STATE.md Plan line and ROADMAP (09-03 ticked, progress 3/4) hand-edited; requirements not marked (09-04).

## Self-Check: PASSED
