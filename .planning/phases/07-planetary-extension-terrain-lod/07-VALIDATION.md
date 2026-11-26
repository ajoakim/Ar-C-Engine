---
phase: 7
slug: planetary-extension-terrain-lod
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-11-26
---

# Phase 7 — Validation Strategy

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (libtest); no new framework |
| **Quick run command** | `cargo test -p planetary-core` (then `-p planetary-terrain`, `-p planetary-app`) |
| **Full suite command** | `cargo test --workspace --locked && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo run --locked -p xtask -- check-deps && cargo run --locked -p xtask -- check-canary` |
| **Estimated runtime** | ~120 seconds |

## Sampling Rate

- **After every task commit:** `cargo test -p <crate touched>` (exhaustive adjacency L<=6, seam L<=4 with N=8)
- **After every plan wave:** full suite + cross `cargo check --target x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`
- **Before `/gsd-verify-work`:** full suite green, then manual window run (human verification)
- **Max feedback latency:** 180 seconds

## Per-Task Verification Map

| Req | Behavior | Automated Command | File Exists |
|-----|----------|-------------------|-------------|
| PLAN-02 | PatchId pack/unpack/parent/children/validation | `cargo test -p planetary-core --test patch_id` | ❌ W0 |
| PLAN-02 | Adjacency reciprocity + integer-corner oracle, L<=6, all faces/edges/corners | `cargo test -p planetary-core --test adjacency` | ❌ W0 |
| PLAN-03 | SSE selection, hysteresis, leaf cap | `cargo test -p planetary-core --test lod_select` | ❌ W0 |
| PLAN-03 | 2:1 balance incl. cross-face; partition sum 4^-L = 6 | `cargo test -p planetary-core --test lod_balance` | ❌ W0 |
| PLAN-04 | Same-level border bit-equality (12 edges, 8 corners); mixed-level gap <= skirt; seam cameras; negative control | `cargo test -p planetary-core --test seam` | ❌ W0 |
| PLAN-04 | Effective-cut stays partition + balanced under partial residency | `cargo test -p planetary-terrain --test effective_cut` | ❌ W0 |
| PLAN-05 | `T(s)` exactness/monotone/odd; layer purity; goldens at 1 and N threads; banned ops | `cargo test -p planetary-core --test generator --test banned_ops` | ❌ W0 |
| PLAN-05 | Winding/normals; border normal equality | `cargo test -p planetary-core --test mesh` | ❌ W0 |
| PLAN-06 | Camera: parallel transport, altitude-scaled speed, clearance, near plane | `cargo test -p planetary-terrain --test camera` | ❌ W0 |
| PLAN-07 | Scripted descent on simulated clock: counted caps, determinism, no leaks, no wall-clock | `cargo test -p planetary-app --test descent` | ❌ W0 |
| APP-02 | App uses only SDK + planetary roots | `cargo test -p planetary-app --test sdk_only` | ❌ W0 |
| support | SDK `gpu_frame` hook + `destroy_mesh`; basic_scene unchanged | `cargo test -p engine-sdk` | extend |
| support | Layer table, docs, canary with new crates | `cargo test -p xtask` and `check-canary` | extend |

## Wave 0 Requirements

- [ ] All test files above (none exist for planetary crates)
- [ ] `planetary-terrain`, `planetary-app` scaffolded + LAYERS + docs + CI edits
- [ ] SDK hook + `destroy_mesh` + tests; ADR for it and for the LOD technique
- [ ] Banned-ops and no-wall-clock source scans for planetary crates

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Fly orbit→surface in a window; no visible cracks/popping | PLAN-04, PLAN-06, APP-02 | Needs display + GPU | `cargo run -p planetary-app --example planet` (exact name from plan) |
| Goldens identical on Linux/macOS | PLAN-05 | Hosted CI blocked (billing) | Run CI matrix |

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] No 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
