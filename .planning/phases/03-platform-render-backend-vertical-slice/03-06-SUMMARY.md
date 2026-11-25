---
phase: 03-platform-render-backend-vertical-slice
plan: 06
subsystem: sdk
tags: [sdk, facade, runtime, run_app, headless-tests]
requires: [03-03, 03-04, 03-05]
provides:
  - "engine-sdk facade: App trait, Runtime<B: RenderBackend>, SdkError, run_app/AppConfig, prelude"
  - "Headless NullBackend frame tests and facade guard tests"
  - "SDK-only triangle example (built, not run)"
affects: [03-07]
tech-stack:
  added: []
  patterns: ["backend-generic frame driver shared by windowed and headless paths", "facade guard tests reading source/manifest text"]
key-files:
  created:
    - crates/engine-sdk/src/error.rs
    - crates/engine-sdk/src/app.rs
    - crates/engine-sdk/src/runtime.rs
    - crates/engine-sdk/src/run.rs
    - crates/engine-sdk/tests/headless.rs
    - crates/engine-sdk/tests/facade.rs
    - crates/engine-sdk/examples/triangle.rs
  modified:
    - crates/engine-sdk/src/lib.rs
key-decisions:
  - "#![allow(clippy::result_large_err)] in lib.rs, tests and example (consistent with engine-core) rather than boxing EngineError"
requirements-completed: [CORE-03, REND-02, REND-03, REND-01]
duration: ~15 min
completed: 2026-11-25
---

# Phase 3 Plan 06: SDK facade Summary

Public SDK facade: `App` trait, backend-generic `Runtime` (update -> tick -> extract -> graph -> submit), `run_app` wiring engine-platform to `WgpuBackend`, headless tests on `NullBackend` at 6.4e6 m, and an SDK-only triangle example.

## Commits
- d3bbdc9: App trait, Runtime, SdkError, re-exports, 6 headless tests
- a919b6a: run_app, facade guard tests (3), triangle example

## Verification
- `cargo test -p engine-sdk --locked`: 6 headless + 3 facade pass
- `cargo build -p engine-sdk --example triangle --locked`: OK (not run; manual run is plan 03-07)
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean
- `cargo xtask check-deps`: OK; `cargo deny check bans`: ok

## Deviations from Plan
- [Rule 3 - Lint] Added `#![allow(clippy::result_large_err)]` to lib.rs, tests/facade.rs, tests/headless.rs and the triangle example because `SdkError` wraps the large `EngineError` (per orchestrator instruction).
- `create_mesh` converts vertex count with `u32::try_from` (let-else) instead of a separate cast check; same behavior.

## Known Stubs
None.

## Self-Check: PASSED
