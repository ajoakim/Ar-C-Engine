---
phase: 11-terrain-relief-normals
plan: 06A
subsystem: planetary-app
tags: [lighting, gap-closure, free-flight]
requires: [11-06]
provides: [free-flight sun follows camera, startup readout of generator/topology/sun elevation]
affects: [planetary-app]
key-files:
  modified:
    - extensions/planetary/planetary-app/src/lib.rs
    - extensions/planetary/planetary-app/examples/planetary_flight.rs
    - extensions/planetary/planetary-app/tests/free_flight_app.rs
    - extensions/planetary/planetary-app/README.md
key-decisions:
  - "Headlight-style fixed-elevation sun in free flight only; no day/night cycle (HLD 22.4)"
  - "Scripted descents and pose_override keep the fixed sun so tests/goldens stay deterministic"
requirements: [TERR-04]
completed: 2026-11-27
---

# Phase 11 Plan 06A: Free-flight sun follows the camera Summary

Fixes root cause 1 of 11-FLAT-DIAGNOSIS.md: the fixed sun was 14 degrees below the horizon at the +X free-flight start, so terrain looked flat and dark.

## What changed
- `PlanetaryApp::update` (free-flight branch only: no script, no pose override) sets `light.direction = sun_direction_for_target(camera position)` each frame, so the sun stands at `SUN_ELEVATION_DEG` (35) at the camera. The initial light, scripted descents and pose overrides are unchanged.
- New API: `AppSettings::set_generator`, `PlanetaryApp::{light_direction, sun_elevation_at_camera_deg, look_line}`. The example uses `set_generator` for `--generator N`.
- Startup line (real window) now ends with: `generator: v2 | topology: hex | sun elevation at camera: 35.0 deg` (v2/hex are the defaults; reflects `--generator`/`--topology`).
- README Lighting section and example header describe the follow-camera sun.
- Tests (free_flight_app.rs): elevation = 35 (>0) at +X, -X, +Y and off-axis; look line text; scripted descent and pose override keep the light unchanged; `--generator N` gives `generator_version == N` (and 4 is rejected without change).

## Not changed
LIGHT_AMBIENT, start direction, terrain constants, goldens, atmosphere_ring golden, image-diff goldens. No flight verdict, default flip, golden pin, requirement tick or push.

## Verification
- `cargo fmt --all -- --check` clean; `cargo clippy -p planetary-app --all-targets --locked -- -D warnings` clean.
- `cargo test -p planetary-app --locked`: all binaries ok (lib 44, app_headless 16, atmosphere_ring 2, descent 20, flicker 6, free_flight 11, free_flight_app 11, morph_handover 6, overlay_toggle 0, reselect_equality 7, sdk_only 8, streaming_lag 4, telemetry 8, doc 0).
- `git hash-object extensions/planetary/planetary-core/src/terrain.rs` = 07439dc86cd9273e3d4627b8e3b6625195936ee0 (unchanged).

## Deviations
None. The initial light is kept fixed (not following) until the first free-flight update, to avoid changing pose-override paths; `look_line` reports the elevation the following sun will have.

## Self-Check: PASSED
