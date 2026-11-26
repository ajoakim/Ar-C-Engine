---
phase: 6
slug: streaming-procedural-framework
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-11-26
---

# Phase 6 — Validation Strategy

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (libtest), Rust 1.99.0 |
| **Quick run command** | `cargo test -p engine-streaming -p engine-procedural` |
| **Full suite command** | `cargo test --workspace --locked && cargo run --locked -p xtask -- check-deps && cargo run --locked -p xtask -- check-canary` |
| **Estimated runtime** | ~90 seconds |

## Sampling Rate

- **After every task commit:** quick run command
- **After every plan wave:** full suite
- **Before `/gsd-verify-work`:** full suite green + check-deps + check-canary
- **Max feedback latency:** 180 seconds

## Per-Task Verification Map

| Req | Behavior | Automated Command | File Exists |
|-----|----------|-------------------|-------------|
| STRM-01 | Request carries key/priority/class/cancel; priority order; queue cap | `cargo test -p engine-streaming request` | ✅ |
| STRM-01 | Cancel frees budget immediately; late completion stale | `cargo test -p engine-streaming cancel` | ✅ |
| STRM-02 | Slow generator: caps hold every frame, parent served until child resident, no blocking | `cargo test -p engine-streaming slow_load` | ✅ |
| STRM-02 | Pinned parent survives LRU pressure; unpinned on all terminal paths | `cargo test -p engine-streaming pins_lru` | ✅ |
| PROC-01 | Layer seed isolation; add/remove/reorder invariance; purity | `cargo test -p engine-procedural seed` | ✅ |
| PROC-01 | Hash known-answer tests (FNV, SplitMix64) | `cargo test -p engine-procedural hash` | ✅ |
| PROC-02 | Golden hash equal at 1 vs N workers, shuffled order, matches constant | `cargo test -p engine-procedural --test golden` | ✅ |
| PROC-02 | Cross-platform identical | CI matrix on 3 OSes | CI (first hosted run pending) |
| Policy | Layer table / docs / ADR coverage | `cargo test -p xtask` | ✅ |

## Wave 0 Requirements

- [x] Both crate scaffolds plus workspace/xtask/docs registration (first)
- [x] `engine-streaming/tests/{slow_load,cancel,pins_lru}.rs`, `engine-procedural/tests/golden.rs`
- [x] ADR-0008 file and README row

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Golden hashes match on Linux and macOS | PROC-02 | Needs hosted CI (billing-blocked) | Run the CI matrix once billing is fixed |

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] No 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-11-26 (local Windows; CI matrix pending)
