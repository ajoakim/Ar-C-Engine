---
phase: 09-overlay-ci-test-hygiene
verified: 2026-11-27T00:00:00Z
status: passed
score: 5/5 must-haves verified
caveats:
  - "MAX_BAD_PIXELS=4 calibrated only on local WARP; hosted CI not yet run (commits not pushed)"
  - "Painter rebuild-on-format-change has no GPU-level test (no code path changes the format); only needs_rebuild logic is unit-tested"
---

# Phase 9: Overlay, CI & Test Hygiene Verification Report

**Goal:** Overlay accurate and robust; CI and image-diff stop hiding small regressions.
**Re-verification:** No (initial). Verified against code, not SUMMARYs.

| # | Success criterion | Status | Evidence |
|---|---|---|---|
| 1 | Toolchain version has a single source | VERIFIED | No `1.99` in `.github/`; ci.yml uses `rustup toolchain install` and `sed` of `rust-toolchain.toml` channel (ci.yml lines 39-44, 141-146). The only literal is in `rust-toolchain.toml`. |
| 2 | Counts are integers, times and ratios keep decimals | VERIFIED | `fmt_count` and `fmt_ms` in `crates/debug-ui/src/lib.rs`; `cargo test -p debug-ui`: 6 passed. |
| 3 | Held-mesh line never exceeds its cap or is labelled, and a test pins it | VERIFIED | `held_mesh_cap()` = base budget + detail budget (`planetary-terrain/src/system.rs:1321`); overlay label "held meshes (base+detail)"; `overlay_snapshot` tests 3/3 pass, including `held_meshes_at_cap_is_not_over_cap`. |
| 4 | Painter rebuilds on format change and a scale factor is accepted | VERIFIED (see caveat) | `needs_rebuild` and `renderer_format` in `engine-render-wgpu/src/overlay.rs`; scale factor is sanitised in debug-ui and wired from `window.scale_factor()` in `engine-sdk/src/run.rs:94`. |
| 5 | Image-diff fails on a few-pixel change and still reports max delta | VERIFIED | `MAX_BAD_PIXELS = 4` in `image_diff.rs`; negative controls plus the boundary test pass (12/12, `--test-threads=1`); `compare` still returns stats with max delta. |

## Requirements

| Req | Status | Evidence |
|---|---|---|
| DEBT-01 | SATISFIED | CI derives toolchain from `rust-toolchain.toml`. |
| DEBT-02 | SATISFIED (caveat) | Rebuild on format change plus scale factor; no GPU-level test. |
| DEBT-03 | SATISFIED (caveat) | Cap of 4 bad pixels, negative controls pass locally. |
| DEBT-04 | SATISFIED | `fmt_count` and `fmt_ms`. |
| DEBT-05 | SATISFIED | Combined cap in the overlay, test-pinned. |

## Guards

- `cargo test -p planetary-app --test sdk_only`: 8 passed.
- `deny.toml`: egui is confined to debug-ui, engine-render-wgpu and engine-sdk; egui-wgpu to engine-render-wgpu.
- No orphaned requirements: DEBT-01..05 are the only ones mapped to Phase 9.

## Anti-patterns
None found in the targeted checks.

## Caveats (non-blocking)
1. Calibrate the cap of 4 on hosted CI runners once pushed; WARP vs other software adapters may differ.
2. The rebuild-on-format-change path is untested at the GPU level.
3. The full 20-minute suite was not re-run here; the 09-04 gate results are taken from the SUMMARY.
