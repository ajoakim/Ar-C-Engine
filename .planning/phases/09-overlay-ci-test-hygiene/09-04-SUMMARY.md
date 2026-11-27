---
phase: 09-overlay-ci-test-hygiene
plan: 04
subsystem: ci-gate
tags: [gate, verification, bookkeeping]
requires: ["09-01", "09-02", "09-03"]
provides: ["Phase 9 full local gate evidence", "DEBT-01..05 closed"]
key-files:
  modified:
    - crates/debug-ui/src/lib.rs
    - extensions/planetary/planetary-app/src/overlay_snapshot.rs
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - .planning/STATE.md
metrics:
  completed: 2026-11-27
---

# Phase 9 Plan 04: Full local gate and requirement bookkeeping Summary

Full workspace gate run once after plans 01-03; one whitespace-only rustfmt fix was needed, everything else was green first time; DEBT-01..05 are recorded complete.

## Gate results

| # | Command | Exit | Time |
|---|---------|------|------|
| 1 | `cargo fmt --all -- --check` (first run) | 1 (two test-code wraps) | 1s |
| 1' | `cargo fmt --all` then `--check` re-run | 0 | <5s |
| 2 | `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 (also 0 after fmt) | 6s |
| 3 | `cargo test --workspace --locked` | 0 (no failures; sdk_only guard ran) | 1202s |
| 4 | `xtask check-deps` | 0 | 1s |
| 4n | negative fixtures core-depends-on-ext / wgpu-winit-misplaced / egui-wgpu-misplaced / egui-core-misplaced | all 1 (expected) | 0-1s each |
| 5 | `xtask check-canary` | 0 | 2s |
| 5n | canary-leak fixture | 1 (expected) | 0s |
| 6a | clippy planetary-app `--features debug-overlay` | 0 | 3s |
| 6b | test planetary-app debug-overlay `--lib --test overlay_toggle` | 0 | 3s |
| 6c | clippy engine-sdk + engine-render-wgpu egui-overlay | 0 | 1s |
| 6d | test engine-sdk `--features egui-overlay` | 0 | 4s |
| 6e | image_diff with egui-overlay, `--test-threads=1` | 0 | 2s |
| 7 | `cargo deny check bans licenses sources advisories` | 0 | 2s |
| 8 | golden dirs unchanged (see below) | empty output | - |

Negative fixtures failed for the intended reason, not a crash:
- core-depends-on-ext: `error[core-depends-on-extension]: engine-core -> planetary-core`
- wgpu-winit-misplaced: `error[wgpu-outside-allowed-crate]` and `error[winit-outside-allowed-crate]`
- egui-wgpu-misplaced: `error[wgpu-outside-allowed-crate]: debug-ui -> egui-wgpu`
- egui-core-misplaced: `error[egui-in-core-crate]: engine-core -> egui`
- canary-leak: `error[canary-planetary-reachable]: conventional-3d -> planetary-core`

Golden check: `git status --short crates/engine-render-wgpu/tests/golden extensions/planetary/planetary-app/tests/golden` and `git diff --stat -- '*golden*'` both printed nothing (checked after the gate and after the fmt fix). No golden PNG changed in this phase.

Flaky tests: none (no test failed in any run).

After the fmt fix, `cargo test -p debug-ui -p planetary-app --locked` was re-run (all ok) instead of repeating the 20 minute workspace suite, since the diff is whitespace only.

## Caveat: image-diff cap calibration

The absolute bad-pixel cap of 4 was calibrated only on local WARP: all 5 scenes (fog_gradient, lit_quad_directional, morph_half, overlay_corner, unlit_triangle) report `bad_pixels=0 max_delta=0`. Behaviour on the hosted lavapipe CI adapter is unverified until CI runs. No push was authorised in this plan.

## Deviations from Plan

**1. [Rule 3 - Blocking] rustfmt wrapping in test code**
- **Found during:** Task 1, step 1
- **Issue:** `fmt --check` failed on `crates/debug-ui/src/lib.rs` (09-03) and `extensions/planetary/planetary-app/src/overlay_snapshot.rs` (09-02); per-plan verification had not included `fmt --check`.
- **Fix:** `cargo fmt --all`; only those two files changed, whitespace-only, no test weakened.
- **Commit:** 05c5213

**2. Hand-edited STATE.md / ROADMAP.md** because `gsd-sdk query state.advance-plan` fails (as instructed). Phase 9 milestone line ticked as well; no completed-phase convention was visible in the roadmap list.

## Known Stubs

None.

## Self-Check: PASSED

Commit 05c5213 exists; DEBT-01..05 checked (5), DEBT-06..09 unchecked (4).
