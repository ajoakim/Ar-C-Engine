---
phase: 07-planetary-extension-terrain-lod
plan: 10
subsystem: planetary-app
tags: [descent, validation, adr, gate]
requires: [07-09]
provides: [PLAN-07 descent suite, ADR-0010 accepted, 07-VALIDATION signed off]
key-files:
  created:
    - extensions/planetary/planetary-app/tests/descent.rs
  modified:
    - crates/engine-streaming/src/manager.rs
    - extensions/planetary/planetary-terrain/src/system.rs
    - docs/architecture/adr/0010-terrain-lod-skirts-and-shared-border-sampling.md
    - docs/architecture/adr/README.md
    - xtask/tests/docs_policy.rs
    - .planning/phases/07-planetary-extension-terrain-lod/07-VALIDATION.md
requirements: [PLAN-07, PLAN-04, PLAN-06, APP-02]
status: automated tasks complete; human checkpoint pending (human_needed)
completed: 2026-11-25
---

# Phase 7 Plan 10: Scripted descent, ADR-0010 acceptance, validation sign-off Summary

Seven-test scripted orbit-to-surface descent suite on a simulated 1/60 s clock, ADR-0010 accepted with measured evidence, 07-VALIDATION signed off, full local gate green. The windowed `planetary_flight` run (Task 3) is NOT performed: **human_needed**.

## Commits
- 5745e5f: fix(07-10) deterministic stream mode blocks on jobs started in the frame (engine-streaming + terrain)
- 2c83d19: feat(07-10) descent.rs (7 tests)
- 1621cd5: docs(07-10) ADR-0010 accepted, README, docs_policy, VALIDATION, clippy fix in descent.rs

## Descent results (tests/descent.rs, 600 frames = 10 simulated s, N = 16)
- Max per frame: started 8, uploads 4, uploaded_bytes 76656, nodes_visited 13618, drawn 612, held 1820, tau_eff 10.125.
- Partition + 2:1 balance every frame after warm-up; max_starts = 1 degradation holds; drawn never empty after warm-up.
- Convergence (drawn == wanted) at frame 590, 110 frames after the descent end (bound CONVERGE_WITHIN = 240). Run length max(600, 720) = 720.
- Determinism: identical across runs (3 repeated full-suite runs stable, identical numbers).
- No leak: buffer_count == 2 * held every frame; detail budget 60 forced 1473 evictions, invariant intact.
- Production (no settle) path: 600 frames, caps and max jump <= 1 hold, no timing assertions.
- Runtime: about 80-110 s for the whole suite in debug (7 tests in parallel).
- Final tunables: FRAMES 600, DESCENT_END_FRAME 480, WARM_UP 10, NODE_VISIT_CAP 50_000, UPLOADS_PER_FRAME 4, REQUESTS_PER_FRAME 16, CONVERGE_WITHIN 240, LOD_MAX_LEVEL 13 (changed from the plan's 16), stream max_pending 256 / starts 8 / completions 8.

## Deviations from Plan

**1. [Rule 1 - Bug] Nondeterministic per-frame work in settle mode.**
`StreamManager::update` started jobs then polled non-blockingly, so a fast job completed in the same frame and a slow one in the next; with `settle_each_frame` only the post-update settle was blocking. Per-frame `completions_applied` differed between runs, failing the determinism test. Fix: `StreamManager::set_deterministic(bool)`; in that mode step 4 uses `wait_all` instead of `poll`. `TerrainSystem::new` enables it when `settle_each_frame` is set. Production path untouched. Files: crates/engine-streaming/src/manager.rs, extensions/planetary/planetary-terrain/src/system.rs. Commit 5745e5f. All engine-streaming/terrain/app tests still pass.

**2. [Tuning] Test LOD max level 16 -> 13.**
At max_level 16 the descent produced about 3200 wanted leaves at the surface even with tau escalated to the cap (tau 51 px at 8 steps, and 583 px with 14 steps): node bounds are inflated by `max_relief` (about 6 km), so every patch within that distance of the camera splits to max level, and tau escalation cannot reduce it. Result: node visits 60k-88k (over the 50k cap), drawn about 180 vs wanted 3200, never converging. Level 13 yields about 610-680 wanted leaves (under max_leaves 700), 13.6k visits and convergence in 110 frames. Documented in the test constants and ADR-0010 Consequences. Invariants (partition, balance, no-leak, drawn == wanted) were not loosened.

**3. [Rule 3] Clippy** `unnecessary_cast` in descent.rs fixed.

## IMPORTANT for the human flight / follow-up
The demo `planetary_flight` uses the default `LodParams` (max_level 19, max_leaves 700). By the same analysis as Deviation 2, near the surface the wanted set will vastly exceed max_leaves and the drawn set will lag (coarse fallback patches, slow refinement) rather than crack. This is a selector design limit (relief-inflated bounds), not a seam bug. Consider, for a gap-closing plan, lowering the demo max_level (about 13-14), or making the relief inflation distance-aware / giving it a per-level bound, before expecting clean refinement at low altitude.

## Gate (all on Windows, toolchain 1.99.0)
- `cargo fmt --all -- --check`: OK
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: OK
- `cargo test --workspace --locked`: all pass, 0 failures
- `xtask check-deps`: OK (18 packages); `check-canary`: OK (289 packages, none planetary)
- `cargo deny check`: advisories, bans, licenses, sources ok
- `planetary_flight --headless 30`: OK (30 frames, 72 draws, 94 meshes)
- Cross-target `cargo check --workspace --all-targets --locked` for x86_64-unknown-linux-gnu and aarch64-apple-darwin: OK
- Canary job simulation in a throwaway `git worktree` (temp dir, own target dir, debug info off): planetary removed, Cargo.toml has no planetary reference, `cargo build --workspace`, `cargo test --workspace --exclude xtask`, clippy conventional-3d, `basic_scene --headless 5`, `check-canary`: all OK. No disk-space failures (free space stayed >= 5.5 GB); worktree and its target dir removed, main target untouched.
- The CI step "test --workspace --exclude planetary-*" was not run separately (subset of the full workspace test run).
- ADR-0010: `Status: accepted` count 1, capitalized `Pending` count 0, mentions `descent.rs`; docs_policy passes.
- 07-VALIDATION.md: status complete, nyquist_compliant true, wave_0_complete true; cross-platform-goldens row and window-flight row remain manual/pending.

## Human checkpoint (Task 3) - PENDING (human_needed)
Not performed per instruction (no window opened). The example builds (`cargo run ... --headless 30` compiled it). Steps for the human are in 07-10-PLAN.md Task 3 (`cargo run -p planetary-app --example planetary_flight`, then `-- --descent`). Note the follow-up above when judging low-altitude refinement. Result: not yet recorded.

## Known Stubs
None.

## Self-Check: PASSED
descent.rs exists; commits 5745e5f, 2c83d19, 1621cd5 present.
