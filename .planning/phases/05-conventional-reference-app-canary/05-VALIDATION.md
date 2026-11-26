---
phase: 5
slug: conventional-reference-app-canary
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-11-26
---

# Phase 5 — Validation Strategy

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (libtest); xtask tests via `cargo test -p xtask` |
| **Quick run command** | `cargo test -p engine-render -p engine-sdk -p conventional-3d --locked` |
| **Full suite command** | `cargo test --workspace --locked && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo run --locked -p xtask -- check-deps && cargo run --locked -p xtask -- check-canary` |
| **GPU (manual, local)** | `cargo test -p engine-render-wgpu -- --ignored` |
| **Estimated runtime** | ~120 seconds |

## Sampling Rate

- **After every task commit:** `cargo test -p <touched crates> --locked` + clippy for them
- **After every plan wave:** full suite command
- **Before `/gsd-verify-work`:** full suite green + `cargo deny check` + ignored GPU test locally
- **Max feedback latency:** 180 seconds

## Per-Task Verification Map

| Req | Behavior | Automated Command | File Exists |
|-----|----------|-------------------|-------------|
| REND-04 | Lit packet carries light/material/normal matrix; reverse-Z kept; bad texture/layout rejected | `cargo test -p engine-render` | ❌ W0 |
| REND-04 | Uniform sizes/offsets; WGSL entry points/bindings; depth state Greater | `cargo test -p engine-render-wgpu --lib` | ❌ W0 |
| REND-04 | Lit textured cube: lit side brighter than shaded side (GPU, ignored) | `cargo test -p engine-render-wgpu -- --ignored` | ❌ W0 |
| REND-05 | glTF fixture loads (mesh/vertex/index counts, texture dims, material, node transform) | `cargo test -p conventional-3d --test gltf_import` | ❌ W0 |
| REND-05 | Malformed/oversized glTF/image rejected without panic | `cargo test -p conventional-3d --test gltf_import` | ❌ W0 |
| REND-05 | Import runs on JobSystem, registered in AssetRegistry | `cargo test -p conventional-3d --test model_loader` | ❌ W0 |
| APP-01 | Headless full frame via Runtime<NullBackend>; fly camera from synthetic InputEvents | `cargo test -p conventional-3d --test headless_scene` | ❌ W0 |
| APP-01 | Example imports only engine_sdk / conventional_3d | `cargo test -p conventional-3d --test sdk_only` | ❌ W0 |
| APP-01 | `basic_scene` example builds | `cargo build -p conventional-3d --example basic_scene` | ❌ W0 |
| PLAN-01 | No planetary crate in conventional-3d transitive closure; negative fixture fails | `cargo run -p xtask -- check-canary` and negated fixture run | ❌ W0 |
| PLAN-01 | Workspace builds/tests with `extensions/planetary` removed (CI script) | script in ci.yml, run locally | ❌ W0 |
| WS | gltf/image confined by cargo-deny wrappers | `cargo deny check bans licenses sources advisories` | extend deny.toml |

## Wave 0 Requirements

- [ ] engine-render lit-packet tests; engine-render-wgpu GPU lit test + `read_offscreen_rgba8`
- [ ] conventional-3d glTF fixture (generated once, committed) and test files
- [ ] xtask `check-canary` subcommand + `xtask/fixtures/canary-leak`
- [ ] gltf/image workspace deps and deny.toml wrapper entries; ADR-0007

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Window shows lit textured scene; WASD, mouse look (RMB), shift boost | APP-01, REND-04 | Needs display + GPU | `cargo run -p conventional-3d --example basic_scene` |

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] No 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
