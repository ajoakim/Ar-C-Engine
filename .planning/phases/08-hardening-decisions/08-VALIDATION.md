---
phase: 8
slug: hardening-decisions
status: draft
nyquist_compliant: false
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
| PLAN-08 | atmosphere colour monotone/finite, ring radius formula, no-NaN, fades to 0 below/above bands | unit | `cargo test -p planetary-app atmosphere --locked` | W0 (new `atmosphere.rs`, 08-10) | pending |
| PLAN-08 | ring renders on a software adapter and is occluded by the planet disc | image-diff | `cargo test -p engine-render-wgpu --test image_diff` | W0 (08-10) | pending |
| QA-01 | image tests render, compare to goldens with tolerance, skip with message / fail under `ARC_REQUIRE_ADAPTER` / `ARC_REQUIRE_GOLDENS` | integration | `cargo test -p engine-render-wgpu --test image_diff -- --nocapture` | W0 (08-03) | pending |
| QA-01 | negative control: perturbed golden or shifted geometry is detected | integration | same | W0 (08-03) | pending |
| QA-01 | benches compile and run | bench | `cargo bench --workspace --benches --locked -- --noplot --quick` | W0 (08-04, 08-05) | pending |
| QA-02 | snapshot builder yields histogram from `drawn_levels()` and budgets; overlay produces non-empty tessellation headless | unit | `cargo test -p debug-ui -p planetary-app --features debug-overlay` | W0 (08-06) | pending |
| QA-02 | overlay pixels appear on a software adapter (corner region differs from baseline) | image-diff | `cargo test -p engine-render-wgpu --features egui-overlay --test image_diff` | W0 (08-08) | pending |
| QA-02 | check-deps and deny still pass with egui crates; negative fixture for misplaced `egui-wgpu` fails | policy | `cargo run -p xtask -- check-deps`; `cargo deny check bans licenses`; negative CI steps | extend existing (08-06, 08-07) | pending |
| QA-03 | each doc exists and contains required literals | policy | `cargo test -p xtask --test docs_policy` | W0 (08-09) | pending |
| WS-05 | ADR-0017 (ECS) and amendments to 0005/0006/0010 present and cite bench/test names | policy | `cargo test -p xtask --test docs_policy` | W0 (08-05) | pending |
| WS-05 | ECS bench runs | bench | `cargo bench -p ecs-bench --benches --locked -- --noplot --quick` | W0 (08-05) | pending |

## Wave 0 Requirements

- [ ] `crates/engine-render-wgpu/tests/image_diff.rs` and golden dirs per OS, software constructor (08-03)
- [ ] `crates/debug-ui/` crate, layer row, doc row, deny wrapper (08-06), xtask rule and fixture (08-07)
- [ ] `engine-spatial/benches`, `planetary-core/benches`, `planetary-app/benches` (08-04), `spikes/ecs-bench` (08-05)
- [ ] `xtask/tests/docs_policy.rs` tests for ADR-0017, new docs, and amended ADRs
- [ ] CI jobs: image-diff (apt lavapipe on Linux, `ARC_REQUIRE_ADAPTER`), bench (non-blocking, artifact), feature-path steps, golden-update artifact upload (08-03, 08-07, 08-11)

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions | Result |
|----------|-------------|------------|-------------------|--------|
| F3 overlay shows LOD histogram, counts, queue, budgets, frame/prepare ms and updates live | QA-02 | Needs display and human judgement | 08-12 Task 2 | pending |
| Atmosphere limb ring (opaque ring, deviates from CONTEXT "shader pass"; see ADR-0019) acceptable, or PLAN-08 deferred | PLAN-08 | Visual judgement | 08-12 Task 2 | pending |
| Plan 26 present mode and window-title readout (07.1-18 caveat C2) | PERF-02 carry-over | Needs window | 08-12 Task 2 | pending |
| HLD.md disposition (commit / gitignore / leave) | n/a | User decision | 08-12 Task 2 | pending |

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] No 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] `nyquist_compliant: true` set in frontmatter (08-12 Task 3)

**Approval:** pending
