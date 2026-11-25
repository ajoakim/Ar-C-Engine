---
phase: 3
slug: platform-render-backend-vertical-slice
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-11-25
---

# Phase 3 — Validation Strategy

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (libtest) |
| **Quick run command** | `cargo test -p engine-render -p engine-platform` |
| **Full suite command** | `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo xtask check-deps && cargo deny check` |
| **Estimated runtime** | ~90 seconds |

## Sampling Rate

- **After every task commit:** `cargo test -p <touched crate>` plus clippy
- **After every plan wave:** full suite command
- **Before `/gsd-verify-work`:** full suite green + manual triangle smoke run
- **Max feedback latency:** 180 seconds

## Per-Task Verification Map

| Req | Behavior | Test Type | Automated Command | File Exists |
|-----|----------|-----------|-------------------|-------------|
| REND-01 | NullBackend records frame; rejects invalid handle | unit | `cargo test -p engine-render null` | ❌ W0 |
| REND-01 | wgpu/winit not outside designated crates | static | `cargo xtask check-deps && cargo deny check bans` | ✅ |
| REND-01 | wgpu backend constructs and draws | manual/ignored | `cargo test -p engine-render-wgpu -- --ignored` (needs GPU) | ❌ W0 |
| REND-02 | winit key/mouse → InputEvent mapping | unit | `cargo test -p engine-platform input` | ❌ W0 |
| REND-02 | Window opens and receives input | manual | `cargo run -p engine-sdk --example triangle` | ❌ W0 |
| REND-03 | extract() output camera-relative and geometry-agnostic | unit | `cargo test -p engine-render extract` | ❌ W0 |
| REND-03 | graph topo sort, cycle/missing-resource errors | unit | `cargo test -p engine-render graph` | ❌ W0 |
| WORLD-03 | camera at 6.4e6 m, sub-mm stability, naive f32 contrast | unit | `cargo test -p engine-render --test precision` | ❌ W0 |
| CORE-03 | SDK-only app compiles; headless frame via null backend | integration | `cargo test -p engine-sdk` | ❌ W0 |

## Wave 0 Requirements

- [ ] `crates/engine-render/tests/precision.rs`, null backend, graph and extract tests
- [ ] `crates/engine-platform` input-translation tests (no window needed)
- [ ] `crates/engine-sdk/tests/headless.rs` with NullBackend
- [ ] Workspace deps added and `cargo deny check` run on the real workspace

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Window opens, triangle renders, input events received | REND-01, REND-02 | Needs display + GPU | `cargo run -p engine-sdk --example triangle` |

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] No 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
