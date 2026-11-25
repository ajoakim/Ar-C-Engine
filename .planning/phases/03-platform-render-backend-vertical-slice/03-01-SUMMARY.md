---
phase: 03-platform-render-backend-vertical-slice
plan: 01
subsystem: build/dependencies
tags: [wgpu, winit, cargo-deny, adr]
requires: []
provides: [pinned Phase 3 deps, ADR-0005]
affects: [engine-platform, engine-render, engine-render-wgpu, engine-sdk]
tech-stack:
  added: [wgpu 30.0.2, winit 0.30.13, raw-window-handle 0.6.2, pollster 1.0.3, bytemuck 1.25.2]
key-files:
  modified: [Cargo.toml, Cargo.lock, crates/engine-platform/Cargo.toml, crates/engine-render/Cargo.toml, crates/engine-render-wgpu/Cargo.toml, crates/engine-sdk/Cargo.toml, deny.toml, docs/architecture/adr/README.md]
  created: [docs/architecture/adr/0005-rendering-api-wgpu-behind-trait.md]
decisions:
  - "wgpu behind engine-owned RenderBackend trait (ADR-0005); multiple-versions stays warn"
metrics:
  completed: 2026-11-25
---

# Phase 3 Plan 01: Dependency pinning and ADR-0005 Summary

Pinned wgpu/winit/raw-window-handle/pollster/bytemuck once in the workspace, wired them into the four Phase 3 crates, and recorded ADR-0005.

## Commits
- 3e4d005: pin deps and manifests, Cargo.lock regenerated
- 0eb5bac: deny.toml comment recording multiple-versions=warn decision
- 4552be6: ADR-0005 and README index row

## Verification
- `cargo check --workspace --locked`, `cargo xtask check-deps` (14 packages OK) pass.
- winit resolves to 0.30.13; only engine-platform declares winit, only engine-render-wgpu declares wgpu.
- `cargo deny check bans licenses sources advisories` passes with 0 unmatched-wrapper warnings; the negative fixture still fails (exit 2).
- clippy `-D warnings`, fmt check and `cargo test -p xtask` pass.

## Deviations from Plan
None. pollster 1.0.3 resolved and compiled, so the 0.4.0 fallback was not needed. No license allow-list changes were required.

## Known Stubs
None.

## Self-Check: PASSED
