---
phase: 08-hardening-decisions
reviewed: 2026-11-27T00:00:00Z
depth: standard
files_reviewed: 22
files_reviewed_list:
  - crates/debug-ui/src/lib.rs
  - crates/engine-diagnostics/src/snapshot.rs
  - crates/engine-render-wgpu/src/overlay.rs
  - crates/engine-render-wgpu/src/backend.rs
  - crates/engine-render-wgpu/tests/image_diff.rs
  - crates/engine-sdk/src/overlay.rs
  - crates/engine-sdk/src/run.rs
  - crates/engine-sdk/src/runtime.rs
  - extensions/planetary/planetary-app/src/atmosphere.rs
  - extensions/planetary/planetary-app/src/lib.rs
  - extensions/planetary/planetary-app/src/overlay_snapshot.rs
  - extensions/planetary/planetary-app/tests/sdk_only.rs
  - extensions/planetary/planetary-app/Cargo.toml
  - spikes/ecs-bench/src/lib.rs
  - spikes/ecs-bench/Cargo.toml
  - xtask/src/check.rs
  - xtask/src/layers.rs
  - deny.toml
  - Cargo.toml
  - .github/workflows/ci.yml
  - crates/engine-sdk/Cargo.toml
  - crates/engine-render-wgpu/Cargo.toml
findings:
  critical: 0
  warning: 5
  info: 3
  total: 8
status: issues_found
---

# Phase 8: Code Review Report

**Reviewed:** 2026-11-27
**Depth:** standard
**Files Reviewed:** 22
**Status:** issues_found

## Summary

No blockers. The overlay path is read-only, feature-gated and NaN-safe. Layering holds: egui is optional in `engine-sdk` and `engine-render-wgpu`, `planetary-app` reaches it only through `engine-sdk/egui-overlay`, and the `sdk_only` guard was not edited. The main weaknesses are enforcement gaps. Several confinement claims in comments and ADRs are not backed by `deny.toml` or `xtask`, and the `sdk_only` guard cannot see the new dev-dependencies. There is also a small egui clock bug. Known open items (float counts, 2026/1500, macOS goldens, non-blocking bench) are not re-reported.

## Warnings

### WR-01: egui and egui-wgpu confinement is not enforced by cargo-deny

**File:** `deny.toml:35`
**Issue:** `egui-wgpu` was added to the `wrappers` of the `wgpu` ban. That only tells cargo-deny that depending on wgpu through egui-wgpu is acceptable. No `deny` entry restricts who may depend on `egui-wgpu` itself, and none restricts `egui`. The CI negative fixture (`egui-wgpu-misplaced`) runs only `xtask check-deps`, not `cargo deny`. `xtask/src/check.rs` has a rule for `egui-wgpu` but none for `egui`, so nothing stops `engine-core` or `engine-render` from adding `egui`. The project constraint is "core must not depend on egui".
**Fix:**
```toml
{ crate = "egui-wgpu", wrappers = ["engine-render-wgpu"], reason = "egui painter confined to backend (ADR-0018)" },
{ crate = "egui", wrappers = ["debug-ui", "engine-sdk", "engine-render-wgpu", "egui-wgpu", "egui-winit"], reason = "UI toolkit confined to the overlay stack" },
```
Add a matching xtask rule for `egui` in tier 0-3 crates, plus a negative fixture.

### WR-02: "hecs only in spikes/ecs-bench" is claimed but not enforced

**File:** `Cargo.toml:72`
**Issue:** The comment says hecs is confined to the spike by `deny.toml`. `deny.toml` and `xtask` contain no hecs rule (grep finds none). `hecs` is a `[workspace.dependencies]` entry, so any crate can adopt it unchecked. That undermines ADR-0017 (no ECS in P0).
**Fix:** Add `{ crate = "hecs", wrappers = ["ecs-bench"], reason = "ECS decision spike only (ADR-0017)" }` to `deny.toml` bans. Alternatively drop the claim from the comment.

### WR-03: Font licences are allowed workspace-wide instead of per crate

**File:** `deny.toml:20-21`
**Issue:** `OFL-1.1` and `Ubuntu-font-1.0` are added to the global `allow` list. Any future dependency under those licences now passes silently. Only `epaint_default_fonts` needs them.
**Fix:** Remove them from `allow` and use a scoped exception:
```toml
[licenses]
exceptions = [{ crate = "epaint_default_fonts", allow = ["OFL-1.1", "Ubuntu-font-1.0"] }]
```

### WR-04: egui overlay clock can run backwards

**File:** `crates/debug-ui/src/lib.rs:111` (with `crates/engine-sdk/src/overlay.rs:26`)
**Issue:** `frame_shapes` feeds pass 0 at `time` and pass 1 at `time + 1/60`. `OverlayHost` advances `time` by the real `dt` per frame. Above 60 fps (`dt < 16.7 ms`) the next frame's first pass gets a timestamp earlier than the previous frame's second pass. egui sees a negative `unstable_dt`, which can stall or glitch animations and time-based state.
**Fix:** Make the clock monotonic. Use the same `time` for both passes (egui only needs the second pass to see the window), or have the host advance by `max(dt, 1/60)`. Or keep `time` for pass 1 and pass `time - 1/60` for pass 0.

### WR-05: `sdk_only` guard does not see the new dev-dependencies or test sources

**File:** `extensions/planetary/planetary-app/tests/sdk_only.rs:63-80` and `extensions/planetary/planetary-app/Cargo.toml`
**Issue:** The guard is intact. But `manifest_keys` reads only `[dependencies]`, and the source scans cover only `src/` and `examples/`. Phase 8 added `engine-render`, `engine-render-wgpu` and `png` as `[dev-dependencies]`, and `tests/atmosphere_ring.rs` (324 lines) uses them directly. Dev-dependencies and test code can now grow to name wgpu, winit and engine crates without tripping any guard. The same helper also ignores `[dependencies.foo]` table syntax.
**Fix:** Add a test that pins the dev-dependency key set (`criterion`, `engine-render`, `engine-render-wgpu`, `png`) and that `[dependencies]` stays unchanged. Also handle `[dependencies.x]` headers in `manifest_keys`. Do not relax the existing assertions.

## Info

### IN-01: Hard-coded toolchain version in CI

**File:** `.github/workflows/ci.yml:42,138`
**Issue:** `grep -F "1.99.0"` duplicates the pin in `rust-toolchain.toml`. A toolchain bump needs edits in two or more places, and a missed edit fails CI.
**Fix:** Derive the version from the file: `rustc --version | grep -F "$(sed -n 's/channel = "\(.*\)"/\1/p' rust-toolchain.toml)"`.

### IN-02: Overlay renderer is created once and ignores later changes to format and scale

**File:** `crates/engine-render-wgpu/src/overlay.rs:64`, `crates/engine-sdk/src/overlay.rs:18`
**Issue:** `egui_wgpu::Renderer` is built on first use with the then-current `format` and is never rebuilt. If the surface format changes on reconfigure, or the device is recreated, the painter is stale and the one-time font-atlas upload is lost. `pixels_per_point` is fixed at 1.0, so the overlay is tiny on HiDPI windows (documented as 1 point = 1 pixel).
**Fix:** Store the format with the renderer and rebuild when it changes. Optionally pass the window scale factor through `OverlayHost::build`.

### IN-03: Image-diff tolerance can hide small regressions

**File:** `crates/engine-render-wgpu/tests/image_diff.rs:27-31`
**Issue:** The images are 64x64 with `MAX_BAD_FRACTION = 0.5%`, which allows 20 differing pixels (about a 4x5 block) with no failure. A thin-edge or small-feature regression can pass. The negative controls cover only large changes.
**Fix:** Use an absolute bad-pixel cap (for example 4) or a tighter fraction, and keep the max-delta report.

---

_Reviewed: 2026-11-27_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
