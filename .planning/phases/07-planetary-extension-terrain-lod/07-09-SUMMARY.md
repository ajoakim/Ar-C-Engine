---
phase: 07-planetary-extension-terrain-lod
plan: 09
subsystem: planetary-app
tags: [app, sdk, descent, policy-test]
requires: [07-06, 07-08]
provides: [PlanetaryApp, DescentScript, planetary_flight example, APP-02 policy test]
key-files:
  created:
    - extensions/planetary/planetary-app/src/descent.rs
    - extensions/planetary/planetary-app/examples/planetary_flight.rs
    - extensions/planetary/planetary-app/tests/app_headless.rs
    - extensions/planetary/planetary-app/tests/sdk_only.rs
  modified:
    - extensions/planetary/planetary-app/src/lib.rs
    - .github/workflows/ci.yml
requirements: [APP-02, PLAN-06]
completed: 2026-11-25
---

# Phase 7 Plan 09: Planetary app, example and SDK-only guard Summary

`PlanetaryApp` (an `engine_sdk::App`) joins `PlanetaryCamera` and `TerrainSystem`, with a pure simulated-time `DescentScript`, a window/headless `planetary_flight` example, an APP-02 policy test and a CI headless step.

## Commits
- 6d24379: PlanetaryApp, DescentScript, app_headless tests (5)
- Task 2 commit: example, sdk_only tests (5), ci.yml step

## Verification
- `cargo test -p planetary-app --locked`: 1 unit + 5 app_headless + 5 sdk_only pass.
- `planetary_flight --headless 30`: 30 frames, 72 draws in last frame, 94 meshes (exit 0). Window path only built, never launched (07-10 human checkpoint).
- clippy -D warnings and fmt for planetary-app clean; check-deps and check-canary OK.
- The no-wall-clock scan skips banned_ops.rs and sdk_only.rs, builds tokens at runtime, asserts >= 10 files.
- Workspace-wide clippy/test were not re-run (disk constraint); only the crate-scoped checks above.

## Deviations
- Added a small unit test in descent.rs (not required by plan).
- In scripted mode, Escape and CloseRequested still set a quit flag so the window can be closed.

## Known Stubs
None.

## Self-Check: PASSED
