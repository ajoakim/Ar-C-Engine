---
phase: 2
slug: foundation-core-jobs-diagnostics-precision
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-11-25
---

# Phase 2 — Validation Strategy

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (libtest); unit tests in-crate plus `tests/` integration per crate |
| **Config file** | none |
| **Quick run command** | `cargo test -p engine-math -p engine-diagnostics -p engine-jobs -p engine-core` |
| **Full suite command** | `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo xtask check-deps` |
| **Estimated runtime** | ~60 seconds |

## Sampling Rate

- **After every task commit:** quick run for the touched crate
- **After every plan wave:** full suite command
- **Before `/gsd-verify-work`:** full suite green
- **Max feedback latency:** 120 seconds

## Per-Task Verification Map

| Req | Behavior | Test Type | Automated Command | File Exists |
|-----|----------|-----------|-------------------|-------------|
| CORE-01 | all 25 lifecycle transition pairs: legal ok, illegal Err | unit | `cargo test -p engine-core lifecycle` | ❌ W0 |
| CORE-02 | RON config parse, bad RON errors, module dep order/cycle/missing | unit | `cargo test -p engine-core config registry` | ❌ W0 |
| CORE-04 | priority order (1 worker), FIFO in class, cancel queued dropped, cancel running observed, shutdown joins | unit/integration | `cargo test -p engine-jobs` | ❌ W0 |
| CORE-05 | span timing via scoped subscriber, FrameStats frame time > 0, duplicate init errors | unit | `cargo test -p engine-diagnostics` | ❌ W0 |
| WORLD-02 | frame conversion round trip, cross-frame delta | unit | `cargo test -p engine-math frame` | ❌ W0 |
| WORLD-04 | 6.4e6 m rebase round trip within 1e-8 m; repeated rebases bounded drift | unit | `cargo test -p engine-math rebase` | ❌ W0 |

## Wave 0 Requirements

- [ ] Workspace deps added to root Cargo.toml and per-crate manifests
- [ ] Test modules in the four crates
- [ ] Optional lifecycle integration test in engine-core (fake module, jobs, diagnostics)

## Manual-Only Verifications

All phase behaviors have automated verification.

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
