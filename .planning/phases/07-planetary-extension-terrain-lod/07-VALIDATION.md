---
phase: 7
slug: planetary-extension-terrain-lod
status: complete
nyquist_compliant: true
wave_0_complete: true
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
| PLAN-02 | PatchId pack/unpack/parent/children/validation | `cargo test -p planetary-core --test patch_id` | ✅ |
| PLAN-02 | Adjacency reciprocity + integer-corner oracle, L<=6, all faces/edges/corners | `cargo test -p planetary-core --test adjacency` | ✅ |
| PLAN-03 | SSE selection, hysteresis, leaf cap | `cargo test -p planetary-core --test lod_select` | ✅ |
| PLAN-03 | 2:1 balance incl. cross-face; partition sum 4^-L = 6 | `cargo test -p planetary-core --test lod_balance` | ✅ |
| PLAN-04 | Same-level border bit-equality (12 edges, 8 corners); mixed-level gap <= skirt; seam cameras; negative control | `cargo test -p planetary-core --test seam` | ✅ |
| PLAN-04 | Effective-cut stays partition + balanced under partial residency | `cargo test -p planetary-terrain --test effective_cut` | ✅ |
| PLAN-05 | `T(s)` exactness/monotone/odd; layer purity; goldens at 1 and N threads; banned ops | `cargo test -p planetary-core --test generator --test banned_ops` | ✅ |
| PLAN-05 | Winding/normals; border normal equality | `cargo test -p planetary-core --test mesh` | ✅ |
| PLAN-02 | Cube-sphere mapping | `cargo test -p planetary-core --test mapping` | ✅ |
| PLAN-03 | Terrain streaming through the SDK | `cargo test -p planetary-terrain --test terrain_stream` | ✅ |
| APP-02 | Headless app on the SDK runtime | `cargo test -p planetary-app --test app_headless` | ✅ |
| PLAN-06 | Camera: parallel transport, altitude-scaled speed, clearance, near plane | `cargo test -p planetary-terrain --test camera` | ✅ |
| PLAN-07 | Scripted descent on simulated clock: counted caps, determinism, no leaks, no wall-clock | `cargo test -p planetary-app --test descent` | ✅ |
| APP-02 | App uses only SDK + planetary roots | `cargo test -p planetary-app --test sdk_only` | ✅ |
| support | SDK `gpu_frame` hook + `destroy_mesh`; basic_scene unchanged | `cargo test -p engine-sdk` | ✅ |
| support | Layer table, docs, canary with new crates | `cargo test -p xtask` and `check-canary` | ✅ |

## Wave 0 Requirements

- [x] All test files above (none exist for planetary crates)
- [x] `planetary-terrain`, `planetary-app` scaffolded + LAYERS + docs + CI edits
- [x] SDK hook + `destroy_mesh` + tests; ADR for it and for the LOD technique
- [x] Banned-ops and no-wall-clock source scans for planetary crates

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Fly orbit→surface in a window; no visible cracks/popping | PLAN-04, PLAN-06, APP-02 | Needs display + GPU | `cargo run -p planetary-app --example planetary_flight` (pending human checkpoint 07-10 Task 3) |
| Goldens identical on Linux/macOS | PLAN-05 | Hosted CI blocked (billing) | Run CI matrix (pending hosted CI) |

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] No 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved (automated); manual window run pending human checkpoint 07-10 Task 3
