---
phase: 09-overlay-ci-test-hygiene
plan: 01
subsystem: ci-test-hygiene
tags: [ci, image-diff, DEBT-01, DEBT-03]
requirements: [DEBT-01, DEBT-03]
key-files:
  modified:
    - .github/workflows/ci.yml
    - crates/engine-render-wgpu/tests/image_diff.rs
decisions:
  - "MAX_BAD_PIXELS = 4 (absolute cap, 0.1% of 64x64) replaces the 0.5% fraction (20 pixels)"
metrics:
  completed: 2026-11-27
---

# Phase 9 Plan 01: CI toolchain single-sourcing and absolute image-diff cap Summary

CI derives the pinned rustc version from rust-toolchain.toml in both jobs, and image_diff.rs uses an absolute 4-pixel cap instead of a 0.5% (20 pixel) fraction.

## Commits
- 8305803 chore(09-01): derive CI toolchain assertion from rust-toolchain.toml
- e5576af test(09-01): absolute bad-pixel cap with few-pixel negative controls

## Changes
- ci.yml: both "Assert pinned toolchain is active" steps use `shell: bash`, extract `channel` with sed, `test -n "$want"` guards against empty extraction (T-09-01), then `grep -F "$want"`. No `1.99` literal remains; extraction yields 1.99.0 locally.
- image_diff.rs: `MAX_BAD_PIXELS: usize = 4`; `compare(..., max_bad: usize)`; failure message reports bad count, tol, cap and `max delta`; `DiffStats.max_delta` retained. New tests `negative_control_small_block_fails` (3x3 block, 9 px, delta 40, message contains "max delta 40") and `bad_pixel_cap_boundary` (cap passes with bad_pixels==cap and max_delta==40, cap+1 fails). `check_scene` prints `image_diff {scene}: bad_pixels=N max_delta=D`. CHANNEL_TOLERANCE stays 2; no goldens touched.

## Calibration (cap rationale)
Adapter: Microsoft Basic Render Driver (Dx12, WARP software). Observed on this machine:

| scene | bad_pixels | max_delta |
|-------|-----------|-----------|
| unlit_triangle | 0 | 0 |
| lit_quad_directional | 0 | 0 |
| fog_gradient | 0 | 0 |
| morph_half | 0 | 0 |
| overlay_corner (egui-overlay) | 0 | 0 |

All scenes match goldens exactly here. The cap of 4 is therefore calibrated only on local WARP; CI also runs lavapipe (Linux) and macOS adapters, whose observed bad_pixels are unknown. If CI shows more than 4 on a legitimate scene, raise it to the smallest value covering the observation, never above 10, and record it. The 9-pixel control must keep failing (it would at any cap <= 8).

## Results
- `cargo test -p engine-render-wgpu --locked --test image_diff`: 11 passed
- with `--features egui-overlay`: 12 passed
- Not run: temporarily raising the cap to 20 to confirm the controls fail (follows directly from 9 > 4 and cap+1 logic).

## Deviations
None. rustfmt was applied to image_diff.rs only. Requirements DEBT-01/03 not marked complete (plan 09-04). STATE.md/ROADMAP.md hand-edited as instructed.

## Self-Check: PASSED
