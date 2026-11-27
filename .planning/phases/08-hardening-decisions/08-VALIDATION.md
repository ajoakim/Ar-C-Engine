---
phase: 8
slug: hardening-decisions
status: complete-with-caveats
nyquist_compliant: true
wave_0_complete: false
created: 2026-11-26
---

# Phase 8 - Validation Strategy

Stub derived from the "Validation Architecture" section of 08-RESEARCH.md. Plan 08-12 Task 1 updates this file with gate results and Task 3 signs it off; it is not created there.

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (libtest, 863 passed / 11 ignored in the 07.1-18 gate); criterion 0.8.2 (new, benches only); no proptest/insta required |
| **Config files** | workspace `Cargo.toml`, `.github/workflows/ci.yml`, `xtask/tests/*.rs` |
| **Quick run command** | `cargo test -p xtask --test docs_policy --locked` and `cargo test -p engine-render-wgpu --locked` |
| **Full suite command** | `cargo test --workspace --locked` (about 20 minutes locally per 07.1-18) |
| **Bench command (report only, never asserted)** | `cargo bench --workspace --benches --locked -- --noplot --quick`; per crate: `cargo bench -p <crate> --benches --locked -- --noplot --quick` or `--bench <name>` |
| **Feature-path commands** | `cargo test -p engine-render-wgpu --features egui-overlay --test image_diff`; `cargo clippy -p planetary-app --features debug-overlay --all-targets -- -D warnings`; `cargo test -p planetary-app --features debug-overlay` |

## Sampling Rate

- **After every task commit:** `cargo test -p <crate touched> --locked` plus `cargo fmt --all -- --check`
- **After every plan wave:** `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `check-deps`, `check-canary`, negative fixtures, `cargo deny check`
- **Before `/gsd-verify-work`:** hosted CI green on all OS jobs including image-diff and the non-blocking bench job, then the human flight (08-12 Task 2)

## Image-diff semantics

- No adapter: clear SKIP message and pass, unless `ARC_REQUIRE_ADAPTER=1` (Linux and Windows CI) which fails.
- Missing golden with `ARC_UPDATE_GOLDENS` unset: clear skip, unless `ARC_REQUIRE_GOLDENS=1` which fails. Plan 11 enables `ARC_REQUIRE_GOLDENS=1` in ci.yml in the same commit that adds the goldens.
- `ARC_UPDATE_GOLDENS=1` only in the `workflow_dispatch` update step; never in push/PR jobs.
- macOS: no software Metal adapter, skip-allowed.

## Per-Requirement Verification Map

| Req | Behavior | Test Type | Automated Command | File Exists | Result |
|-----|----------|-----------|-------------------|-------------|--------|
| PLAN-08 | atmosphere colour monotone/finite, ring radius formula, no-NaN, fades to 0 below/above bands | unit | `cargo test -p planetary-app atmosphere --locked` | W0 (new `atmosphere.rs`, 08-10) | pass: inside `cargo test --workspace --locked` (exit 0) and `--features debug-overlay` run (exit 0) |
| PLAN-08 | ring renders on a software adapter and is occluded by the planet disc | image-diff | `cargo test -p planetary-app --test atmosphere_ring` | W0 (08-10) | pass: `atmosphere_ring_scene` ok on Microsoft Basic Render Driver (Dx12), exit 0 |
| QA-01 | image tests render, compare to goldens with tolerance, skip with message / fail under `ARC_REQUIRE_ADAPTER` / `ARC_REQUIRE_GOLDENS` | integration | `cargo test -p engine-render-wgpu --test image_diff -- --nocapture` | W0 (08-03) | pass: 9 tests ok on real adapter Microsoft Basic Render Driver (Dx12), no skips, exit 0 |
| QA-01 | negative control: perturbed golden or shifted geometry is detected | integration | same | W0 (08-03) | pass: `negative_control_shifted_geometry_fails_on_gpu` ok, exit 0 |
| QA-01 | benches compile and run | bench | `cargo bench --workspace --benches --locked -- --noplot --quick` | W0 (08-04, 08-05) | pass: `--bench spatial --bench select --bench mesh --bench stream --bench ecs` completes, exit 0 (report only) |
| QA-02 | snapshot builder yields histogram from `drawn_levels()` and budgets; overlay produces non-empty tessellation headless | unit | `cargo test -p debug-ui -p planetary-app --features debug-overlay` | W0 (08-06) | pass: `-p planetary-app --features debug-overlay` exit 0; debug-ui covered by workspace run |
| QA-02 | overlay pixels appear on a software adapter (corner region differs from baseline) | image-diff | `cargo test -p engine-render-wgpu --features egui-overlay --test image_diff` | W0 (08-08) | pass: `overlay_corner_differs` ok on Dx12 software adapter, exit 0 |
| QA-02 | check-deps and deny still pass with egui crates; negative fixture for misplaced `egui-wgpu` fails | policy | `cargo run -p xtask -- check-deps`; `cargo deny check bans licenses`; negative CI steps | extend existing (08-06, 08-07) | pass: check-deps exit 0; 3 negative fixtures fail as required; `cargo deny check bans licenses sources advisories` exit 0; deny negative fixture fails as required |
| QA-03 | each doc exists and contains required literals | policy | `cargo test -p xtask --test docs_policy` | W0 (08-09) | pass: `docs_policy` inside workspace test run, exit 0 |
| WS-05 | ADR-0017 (ECS) and amendments to 0005/0006/0010 present and cite bench/test names | policy | `cargo test -p xtask --test docs_policy` | W0 (08-05) | pass: `docs_policy` inside workspace test run, exit 0 |
| WS-05 | ECS bench runs | bench | `cargo bench -p ecs-bench --benches --locked -- --noplot --quick` | W0 (08-05) | pass: `--bench ecs` completes, exit 0 |

## Local Gate Results (08-12 Task 1)

Run on Windows 11, rustc 1.99.0, free disk 132G at start. Hosted CI was already green in run 46042013890 (see 08-CI-EVIDENCE.md). Bench commands use `--bench <name>` per crate instead of `--benches -- --noplot --quick` (as in ci.yml, with `--noplot --quick` kept). Image tests use `--test-threads=1` as ci.yml does. Durations are informational. No failures and no flaky tests; nothing was fixed or re-run.

| Gate | Exit | Time |
|------|------|------|
| `cargo fmt --all -- --check` | 0 | 0s |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | 1s (cached) |
| `cargo test --workspace --locked` (913 passed, 0 failed, 11 ignored) | 0 | 1398s |
| `cargo test -p engine-render-wgpu --locked --test image_diff -- --nocapture --test-threads=1` (adapter: Microsoft Basic Render Driver Dx12) | 0 | 4s |
| `cargo test -p planetary-app --locked --test atmosphere_ring -- --nocapture --test-threads=1` (same adapter) | 0 | 1s |
| `cargo bench --workspace --bench spatial --bench select --bench mesh --bench stream --bench ecs --locked -- --noplot --quick` (completes; not asserted) | 0 | 46s |
| `cargo run --locked -p xtask -- check-deps` | 0 | 1s |
| `! check-deps` fixtures core-depends-on-ext, wgpu-winit-misplaced, egui-wgpu-misplaced (each fails as required) | 0 | 1s total |
| `cargo run --locked -p xtask -- check-canary` | 0 | 1s |
| `! check-canary` fixture canary-leak | 0 | 0s |
| `cargo build --locked -p conventional-3d --example basic_scene` | 0 | 6s |
| `cargo run --locked -p planetary-app --example planetary_flight -- --headless 30` | 0 | 7s |
| same with `--features debug-overlay` | 0 | 9s |
| `cargo test -p engine-render-wgpu --locked --features egui-overlay --test image_diff -- --nocapture --test-threads=1` | 0 | 2s |
| `cargo clippy -p planetary-app --features debug-overlay --all-targets --locked -- -D warnings` | 0 | 2s |
| `cargo test -p planetary-app --features debug-overlay --locked` | 0 | 807s |
| `cargo test --workspace --exclude planetary-core --exclude planetary-terrain --exclude planetary-app --locked` (487 passed) | 0 | 11s |
| `cargo deny check bans licenses sources advisories` | 0 | 1s |
| `! cargo deny ... xtask/fixtures/wgpu-winit-misplaced ... check bans` (fails as required) | 0 | 0s |
| `cargo check --workspace --locked --target x86_64-unknown-linux-gnu` | 0 | 15s |
| `cargo check --workspace --locked --target aarch64-apple-darwin` | 0 | 14s |

## Wave 0 Requirements

- [ ] `crates/engine-render-wgpu/tests/image_diff.rs` and golden dirs per OS, software constructor (08-03)
- [ ] `crates/debug-ui/` crate, layer row, doc row, deny wrapper (08-06), xtask rule and fixture (08-07)
- [ ] `engine-spatial/benches`, `planetary-core/benches`, `planetary-app/benches` (08-04), `spikes/ecs-bench` (08-05)
- [ ] `xtask/tests/docs_policy.rs` tests for ADR-0017, new docs, and amended ADRs
- [ ] CI jobs: image-diff (apt lavapipe on Linux, `ARC_REQUIRE_ADAPTER`), bench (non-blocking, artifact), feature-path steps, golden-update artifact upload (08-03, 08-07, 08-11)

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions | Result |
|----------|-------------|------------|-------------------|--------|
| F3 overlay shows LOD histogram, counts, queue, budgets, frame/prepare ms and updates live | QA-02 | Needs display and human judgement | 08-12 Task 2 | CONFIRMED by user screenshots (2026-11-27): panel shows frame/prepare ms, selected, drawn, queue pending/in flight/awaiting upload, detail patches, uploads/frame, starts/frame, LOD drawn and wanted histograms; values change between alt 728.5 km (hex L7) and alt 974 m (hex L16). Cosmetic findings: counts print as floats (593.00); detail patches 2026/1500 and 1938/1500 exceed the displayed cap (todo) |
| Atmosphere limb ring (opaque ring, deviates from CONTEXT "shader pass"; see ADR-0019) acceptable, or PLAN-08 deferred | PLAN-08 | Visual judgement | 08-12 Task 2 | user: "accept the ring"; ADR-0019 deviation accepted, PLAN-08 complete, not deferred |
| Plan 26 present mode and window-title readout (07.1-18 caveat C2) | PERF-02 carry-over | Needs window | 08-12 Task 2 | CONFIRMED for immediate (user paste, 2026-11-27): startup line `present=PresentInfo { requested: Immediate, active: "Immediate", supported: ["Immediate", "Fifo"], frame_latency: 2, fell_back: false }` on Intel Arc B580 (Vulkan). Window title is by design `alt | speed | hex Ln` (present info is in the startup line). Not shown: the fifo run and the monitor refresh figure |
| HLD.md disposition (commit / gitignore / leave) | n/a | User decision | 08-12 Task 2 | user: "leave HLD.md untracked" (option c); nothing committed or ignored |

### Human flight (08-12 Task 2), user's words

- After flying: "impressive progress i still see texture blurring and then sharpening when moving from one level to another, i think there should be more of transition or opacity boundary between textures... the surface is smooth. adding different elevations hills mountains, maybe a gridded bump mapping based on color would help. i would like to have more degrees of freedom when free flying"
- Atmosphere: "accept the ring". HLD.md: "leave HLD.md untracked".
- Caveats: C2 mostly closed (immediate mode confirmed via startup line; fifo run and monitor refresh not reported), numeric telemetry. F3 overlay confirmed from screenshots; cosmetic follow-ups in the todo.
- Visual feedback is a follow-up, not a Phase 8 failure: `.planning/todos/pending/2026-11-27-post-phase8-visual-feedback.md`.

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] No 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] `nyquist_compliant: true` set in frontmatter (08-12 Task 3)

**Approval:** complete-with-caveats, 2026-11-27
