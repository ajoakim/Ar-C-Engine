---
phase: 08-hardening-decisions
plan: 10
subsystem: rendering
tags: [atmosphere, limb-ring, planetary-app, adr-0019, image-diff]
requires:
  - phase: 08-03
    provides: software-adapter constructor and the golden/skip/require logic
  - phase: 08-08
    provides: overlay hand-off (unchanged here)
  - phase: 08-09
    provides: docs set (sdk-usage, limitations, dependency-direction) kept in sync
provides:
  - planetary-app atmosphere module (tangent-cone ring, ring alpha, sun-tinted sky)
  - AppSettings.atmosphere and the --no-atmosphere flag
  - atmosphere_ring image scene with a Windows WARP golden (planetary-app integration test)
  - ADR-0019 with the Deviation from CONTEXT section
affects: [08-11 CI image job and Linux/macOS goldens, 08-12 human flight and accept-or-defer decision]
tech-stack:
  added: []
  patterns: [pure-function atmosphere math, per-frame mesh rebuild through Resources, dev-only edges for extension tests]
key-files:
  created:
    - extensions/planetary/planetary-app/src/atmosphere.rs
    - extensions/planetary/planetary-app/tests/atmosphere_ring.rs
    - extensions/planetary/planetary-app/tests/golden/windows/atmosphere_ring.png
    - docs/architecture/adr/0019-minimal-atmosphere-limb-ring.md
  modified:
    - extensions/planetary/planetary-app/src/lib.rs
    - extensions/planetary/planetary-app/Cargo.toml
    - extensions/planetary/planetary-app/examples/planetary_flight.rs
    - extensions/planetary/planetary-app/tests/descent.rs
    - docs/architecture/adr/0015-scale-contrast-and-distance-haze.md
    - docs/architecture/adr/README.md
    - docs/limitations.md
    - docs/sdk-usage.md
    - docs/architecture/dependency-direction.md
    - xtask/tests/docs_policy.rs
    - Cargo.lock
key-decisions:
  - "Opaque limb-ring mesh plus clear-colour/fog change instead of a shader pass (ShaderId closed, REPLACE-only blend): ADR-0019, Deviation from CONTEXT"
  - "Image scene is a planetary-app integration test (R1 forbids engine-render-wgpu depending on extensions), with dev-only engine-render, engine-render-wgpu and png"
  - "Ring mesh rebuilt every frame (colours depend on altitude and sun); last frame's handle destroyed first"
requirements-completed: []
duration: ~3h wall (long test runs)
completed: 2026-11-27
---

# Phase 8 Plan 10: Minimal atmosphere (limb ring) Summary

An opaque camera-facing limb ring and a sun-elevation-tinted sky colour, entirely in `planetary-app`, on by default and switched off with `--no-atmosphere`. No engine-core, render or SDK change. PLAN-08 is NOT marked complete: it depends on the plan 12 human flight, which also decides accept-or-defer.

## What was built
- `atmosphere.rs`: `ring_inner_radius` (`r_in = R / sqrt(1 - (R/d)^2)`, `None` for `d <= R(1+1e-6)` and non-finite input), `ring_outer_ratio`, `ring_alpha` (0 at 20 km, 1 at 200 km, same bands as the haze), `sky_color`, `atmosphere_for_view` (bit-identical to `haze_for_altitude` when the sun sine is >= 0.30 or vectors are degenerate), `ring_vertices` (64 segments, 384 vertices as a triangle list, outer edge exactly the clear colour), `ring_frame`.
- `lib.rs`: `haze_for_altitude` now delegates to `haze_with_sky`; `apply_camera` uses `atmosphere_for_view` when `atmosphere` is on; `update_ring` runs in `gpu_frame` after the terrain rewrote `scene.items` (destroys last frame's ring mesh, builds and pushes the new item, skips if the draw list is full or the ring is invisible); `ring_buffer_count()` accessor.
- `planetary_flight`: `--no-atmosphere`; docs flag tables updated.
- Image scene `atmosphere_ring` (disc drawn first, ring drawn after, so only depth can hide the ring). Pixel assertions run whenever an adapter exists, independent of the golden.
- ADR-0019 (Status accepted, mandatory `## Deviation from CONTEXT` with "shader pass", "ShaderId", "REPLACE"), ADR-0015 status line points to it, README row, limitations text, `adr_0019_present` docs_policy test.

## Measured results (this session)
- Unit tests: 16 new tests in `atmosphere::tests`, all pass (lib: 30/30). They cover the tangent-cone radius against the closed form and `tan(asin(R/d))`, the vertex count, outer colour equals clear for altitudes 25 km to 1e12 m, finite output at d = 1e300 and at the epsilon edge, and `None` at d = R and d < R.
- Image scene on WARP ("Microsoft Basic Render Driver (Dx12)"): the pixel assertions pass (the overlap pixel equals the disc pixel, the ring pixel differs from both disc and clear and has blue > red, a pixel beyond the outer edge equals the clear colour). Golden generated twice with `ARC_UPDATE_GOLDENS=1`, byte-identical (`cmp`), then two verify runs with `ARC_REQUIRE_ADAPTER=1 ARC_REQUIRE_GOLDENS=1`: max channel delta 0, 0 pixels over tolerance.
- Release `--bench-descent` (hex, this session), worst-band p95 = 3.99 ms (100 m-1 km band) against the 16.7 ms budget, RESULT: PASS; per-band p95 0.60 / 0.90 / 2.17 / 3.70 / 3.99 / 3.98 ms; overall avg 2.24 ms, p95 3.91 ms, max 4.73 ms. The same bench with `--no-atmosphere`: worst-band p95 4.05 ms (1-10 km band), PASS. The two runs differ by less than the run-to-run noise; no overhead is claimed or ruled out beyond "both far under budget". This is a headless CPU measurement (NullBackend); the GPU cost of the ring was not measured.
- Checks: `check-deps` OK (20 packages), `check-canary` OK, `cargo deny check` (advisories, bans, licenses, sources ok), `docs_policy` 22/22, clippy `--workspace --all-targets --locked -D warnings` clean, `cargo fmt --all -- --check` clean, `sdk_only` 6/6 unchanged.
- `cargo test --workspace --locked`: 913 passed, 0 failed, 11 ignored (summed over all test binaries and doc-tests). No flaky failures; no test needed a re-run.

## Not judged (stated plainly)
Visual quality cannot be judged by the executor and was not. Nothing here says the atmosphere looks good. Only measurable facts and the image-scene pixel checks above are reported. The default descent places the sun 35 degrees above the target horizon, so the sky tint is zero in the default flight (sky equals the old haze there); the ring and tint are visible only away from that geometry. Plan 12's human flight judges look and decides whether to accept the ring or defer PLAN-08 (ADR-0019 Fallback).

## Deviations from Plan

**1. [Rule 3 - Blocking] Image scene lives in planetary-app, not image_diff.rs.** The plan's stated fallback: a core crate cannot dev-depend on an extension (R1). Added dev-only `engine-render`, `engine-render-wgpu`, `png` to planetary-app (`[dependencies]` untouched, `sdk_only` guard unmodified and passing), a copied compare/golden helper with the plan 03 skip/require env logic, golden at `planetary-app/tests/golden/windows/`. `dependency-direction.md` planetary-app row gained a "dev-only" note. Commit 1a32f71.

**2. [Rule 1 - Bug] `descent` leak tests counted exactly `held + classes` buffers.** The per-frame ring adds one legitimate buffer, so `descent_no_gpu_leak` and `hex_no_gpu_leak` failed (`f0 buffers`). Kept strict equality and added the ring explicitly (`held + classes + ring_buffers`, plus `ring_buffers <= 1`) via the new `PlanetaryApp::ring_buffer_count()`. This is an accounting change, not a loosened bound. Commit c7daeb1.

**3. [Rule 3] docs_policy `sdk_usage_names_exist_in_source`** requires every USAGE flag in `docs/sdk-usage.md`; added `--no-atmosphere` there and in the planetary-app README.

**4. `atmosphere_for_view` signature.** The plan wrote `(altitude, sun_dir, view_dir)`; implemented `(altitude, light, up)` where `up` is the local vertical (camera position vector), because the sun elevation needs the vertical, not the look direction. The flag is handled in the app (off uses `haze_for_altitude`), so the pure function takes no flag.

**5. TDD not strictly split.** The atmosphere module and its tests were written together and committed in one `feat` commit (no separate failing `test` commit for Task 1); the docs task had a genuine RED step (`adr_0019_present` failed before the ADR existed).

**6. Last ring mesh is not destroyed at shutdown.** `PlanetaryApp::shutdown` has no `Resources`, so the final ring buffer is released with the backend, as for the terrain meshes.

## Known limits / handed on
- Plan 11: the CI `image-diff` job and `08-VALIDATION.md` row 43 point at `-p engine-render-wgpu --test image_diff`; the atmosphere scene runs under `-p planetary-app --test atmosphere_ring` and must be added to the job. Linux and macOS goldens for `atmosphere_ring` do not exist (without them the golden half skips with a message; the pixel assertions still run on any adapter).
- Plan 12: accept-or-defer question for the ring (ADR-0019).
- Sdk guard: no change to `tests/sdk_only.rs`.

## Known Stubs
None.

## Threat Flags
None. T-08-20 (NaN/inf geometry) mitigated by `None` returns and tested at d = R, d < R, NaN and infinity; T-08-21 (core coupling) mitigated, no change under `crates/` and `check-deps` and `check-canary` pass.

## Commits
- c7daeb1 feat(08-10): atmosphere module, limb ring and sky tint wired into planetary-app
- 1a32f71 feat(08-10): --no-atmosphere flag and atmosphere_ring image scene
- 3bfcc34 docs(08-10): ADR-0019 minimal atmosphere, ADR-0015 status pointer, limitations

## Self-Check: PASSED
