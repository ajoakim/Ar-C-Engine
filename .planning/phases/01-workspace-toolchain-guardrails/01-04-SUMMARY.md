---
phase: 01-workspace-toolchain-guardrails
plan: 04
subsystem: infra
tags: [cargo-deny, github-actions, ci, guardrails]
requires: [01-03]
provides:
  - deny.toml (wgpu/winit wrapper bans, license allowlist, sources, advisories)
  - .github/workflows/ci.yml (ubuntu + windows matrix, deny job)
key-files:
  created: [deny.toml, .github/workflows/ci.yml]
requirements-completed: [WS-01, WS-03]
completed: 2026-11-25
---

# Phase 1 Plan 4: cargo-deny and CI Summary

cargo-deny 0.22.1 (user-level install) with wrapper bans for wgpu/winit, plus a GitHub Actions workflow running every guardrail on ubuntu and windows with the pinned 1.99.0 toolchain.

## Commits
- 64c41af feat(01-04): deny.toml
- ddf22ca ci(01-04): add GitHub Actions matrix and cargo-deny job

## Results
- `cargo deny check bans licenses sources` exits 0 on the workspace; `cargo deny check advisories` is ok.
- Negative fixture (wgpu-winit-misplaced) exits 2 with `bans FAILED`: `error[banned]: crate 'wgpu = 0.0.0' is explicitly banned`, with `direct parent 'engine-core = 0.0.0' of banned crate 'wgpu = 0.0.0' was not marked as a wrapper`. winit is also rejected via the dev-dependency on `engine-world`.
- Local run of the CI commands passed: fmt, clippy -D warnings, tests, `check-deps` (14 packages OK), and both xtask negative fixtures fail as expected.

## Deviations from Plan
**1. [Rule 3 - Blocking] cargo-deny 0.22.1 CLI argument order.** `--config` is a global option and is rejected after `check`. The negative command is `cargo deny --manifest-path <fixture> --config deny.toml check bans`. Fixed in ci.yml (the plan's text had `--config` after `check`).

deny.toml was written exactly as specified; no key renames or license additions were needed. cargo-deny install succeeded on the first attempt, so the fallback was not used.

## Notes
- cargo-deny warns about unused license allowances and unused wrappers on the real workspace (expected, no violations).
- actionlint is not available; ci.yml was validated with PyYAML (parses) plus grep checks (no `@main`/`@master`, `contents: read`, windows-latest, pinned 1.99.0 assertion).

## Manual verification pending
Push to GitHub and confirm both build matrix legs (ubuntu-latest, windows-latest) and the cargo-deny job are green (VALIDATION.md manual-only item). No remote exists, so nothing was pushed.

## Self-Check: PASSED
