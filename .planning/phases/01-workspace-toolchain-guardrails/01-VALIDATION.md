---
phase: 1
slug: workspace-toolchain-guardrails
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-11-25
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (libtest) |
| **Config file** | rust-toolchain.toml, deny.toml |
| **Quick run command** | `cargo test -p xtask` |
| **Full suite command** | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo xtask check-deps && cargo deny check bans licenses` |
| **Estimated runtime** | ~60 seconds |

## Sampling Rate

- **After every task commit:** `cargo test -p xtask` and `cargo check --workspace`
- **After every plan wave:** full suite command
- **Before `/gsd-verify-work`:** full suite green locally (CI green on both OSes once hosted)
- **Max feedback latency:** 120 seconds

## Per-Task Verification Map

| Req | Behavior | Test Type | Automated Command | File Exists |
|-----|----------|-----------|-------------------|-------------|
| WS-01 | Pinned toolchain active; workspace builds | smoke | `cargo --version`; `cargo build --workspace` | ❌ W0 |
| WS-01 | rust-version >= 1.95, edition 2024 on all members | unit | `cargo test -p xtask workspace_metadata_policy` | ❌ W0 |
| WS-02 | Real workspace passes layer rules | unit | `cargo test -p xtask real_workspace_ok` | ❌ W0 |
| WS-02 | Dependency doc lists every crate | unit | `cargo test -p xtask docs_cover_all_crates` | ❌ W0 |
| WS-03 | core→extension violation detected | negative | `cargo test -p xtask fixture_core_depends_on_extension_fails` | ❌ W0 |
| WS-03 | wgpu/winit misplacement detected | negative | `cargo test -p xtask fixture_wgpu_winit_misplaced_fails` | ❌ W0 |
| WS-03 | cargo-deny passes | CI | `cargo deny check bans licenses` | ❌ W0 |
| WS-04 | ADR template and ADR-0001 present with required sections | unit | `cargo test -p xtask adr_conventions_present` | ❌ W0 |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

## Wave 0 Requirements

- [ ] `xtask/` crate with `check_deps` library fn and tests
- [ ] `xtask/fixtures/*` violating mini-workspaces (excluded from workspace)
- [ ] `.cargo/config.toml` alias for `cargo xtask`
- [ ] cargo-deny install (user-level `cargo install cargo-deny --locked`)

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| CI matrix passes on ubuntu + windows | WS-01, WS-03 | Needs hosted GitHub Actions | Push and check the workflow run |

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
