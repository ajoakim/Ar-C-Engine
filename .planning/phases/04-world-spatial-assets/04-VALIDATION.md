---
phase: 4
slug: world-spatial-assets
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-11-26
---

# Phase 4 — Validation Strategy

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (libtest); `cargo xtask check-deps` for layering |
| **Quick run command** | `cargo test -p engine-world -p engine-spatial -p engine-assets` |
| **Full suite command** | `cargo test --workspace --locked && cargo clippy --workspace --all-targets -- -D warnings && cargo xtask check-deps` |
| **Estimated runtime** | ~60 seconds |

## Sampling Rate

- **After every task commit:** `cargo test -p <crate touched>`
- **After every plan wave:** quick run plus `cargo xtask check-deps`
- **Before `/gsd-verify-work`:** full suite green (CI on three OSes once hosted)
- **Max feedback latency:** 120 seconds

## Per-Task Verification Map

| Req | Behavior | Automated Command | File Exists |
|-----|----------|-------------------|-------------|
| WORLD-01 | spawn/despawn, stale id → None, slot reuse gives different id | `cargo test -p engine-world entity` | ❌ W0 |
| WORLD-01 | recursive despawn; cycle/self-parent/stale-parent rejected | `cargo test -p engine-world hierarchy` | ❌ W0 |
| WORLD-01 | child world transform = parent ∘ local; dirty propagation | `cargo test -p engine-world transform` | ❌ W0 |
| WORLD-01 | 6.4e6 m root + 1 mm child precision; rebase keeps position (1e-8 m) | `cargo test -p engine-world --test precision` | ❌ W0 |
| WORLD-05 | queries vs brute-force oracle (frustum, region, radius) | `cargo test -p engine-spatial --test oracle` | ❌ W0 |
| WORLD-05 | hash grid insert/update/remove/dedup/oversize | `cargo test -p engine-spatial hash_grid` | ❌ W0 |
| WORLD-05 | trait object usable with second trivial backend | `cargo test -p engine-spatial trait_object` | ❌ W0 |
| ASSET-01 | AssetId golden value, key normalization | `cargo test -p engine-assets id` | ❌ W0 |
| ASSET-01 | typed Handle<T>, stale handle → None | `cargo test -p engine-assets handle` | ❌ W0 |
| ASSET-02 | exhaustive 8×8 transition matrix | `cargo test -p engine-assets residency_state` | ❌ W0 |
| ASSET-02 | synthetic job loader drives states; cancel; panic → Failed | `cargo test -p engine-assets --test loader` | ❌ W0 |
| ASSET-03 | budgets reject over-budget; per-frame upload cap reset by begin_frame | `cargo test -p engine-assets budget` | ❌ W0 |
| ASSET-03 | LRU order, in-flight excluded, accounting invariant | `cargo test -p engine-assets eviction` | ❌ W0 |

## Wave 0 Requirements

- [ ] `slotmap`, `uuid` added to workspace deps and manifests; cargo-deny run
- [ ] Test files above; tiny deterministic PRNG helper for oracle tests
- [ ] ADR-0006 (spatial backend: hash grid first, benchmark later) and ADR index row

## Manual-Only Verifications

All phase behaviors have automated verification.

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] No 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
