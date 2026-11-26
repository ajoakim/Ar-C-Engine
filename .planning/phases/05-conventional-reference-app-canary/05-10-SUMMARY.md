---
phase: 05-conventional-reference-app-canary
plan: 10
subsystem: ci
tags: [ci, canary, gpu, verification]
requires: ["05-09", "05-02"]
provides: [ci-canary-job, local-ci-evidence]
affects: [.github/workflows/ci.yml]
key-files:
  modified: [.github/workflows/ci.yml]
metrics:
  completed: 2026-11-25
---

# Phase 5 Plan 10: CI canary wiring and local verification Summary

CI now has the three canary layers (check-canary pass step, canary-leak negative fixture, `--exclude planetary-core` test) and a `canary` job that deletes `extensions/planetary` and builds, tests, lints and runs `basic_scene --headless 5`. Every CI command was run locally and passed.

Status: Tasks 1-2 complete. Task 3, the windowed run, is **pending (human_needed)**.

## Commits
- 1c832b6: feat(05-10): wire canary steps and planetary-absent canary job into CI

## Task 1
The CI edits match the plan. One change from the plan's text, made on review: the removal step uses `if grep -q planetary Cargo.toml; then echo ...; exit 1; fi` instead of `! grep -q ...`. Under `bash -e` a bare `!` command never fails the step, so the original assertion did nothing. The YAML parses (PyYAML). `check-canary` appears 3 times, `canary:` once, and `contents: read` once.

## Task 2: local results (RUSTFLAGS="-D warnings")
Main matrix, all exit 0:
- `rustc` is 1.99.0
- fmt, clippy `--workspace --all-targets --locked`, build and test `--workspace --locked`
- `check-deps` reports OK for 14 packages
- The two check-deps negative fixtures fail as expected
- `basic_scene` example build
- `test --workspace --exclude planetary-core --locked`
- `check-canary` reports OK (conventional-3d reaches 287 packages, none planetary)
- The canary-leak negative fixture fails as expected

Deny job, all exit 0:
- `cargo deny check bans licenses sources advisories` reports ok on all four
- The wgpu-winit-misplaced negative fails as expected

Canary job, in a throwaway `git worktree --detach` under a temp dir with its own `CARGO_TARGET_DIR`:
- The removal assertions pass.
- `cargo build --workspace` (no `--locked`), `cargo test --workspace --exclude xtask`, and clippy on conventional-3d all exit 0.
- `basic_scene --headless 5` prints `5 frames, 10 draws in last frame, 5 submits`.
- `check-canary` reports OK with 287 packages.
- After `git worktree remove --force` and `git worktree prune`, `git worktree list` shows only the main worktree. `git status` shows only the pre-existing untracked `.planning/config.json` and `HLD.md`. `extensions/planetary` and `Cargo.lock` are intact.

GPU: `cargo test -p engine-render-wgpu --locked -- --ignored --nocapture` ran on `Intel(R) Arc(TM) B580 Graphics (Vulkan)`. All 6 tests passed:
- `gpu_offscreen_triangle_submits`
- `gpu_reverse_z_nearer_wins`
- `gpu_default_white_texture_when_none`
- `gpu_indexed_and_unindexed_mix`
- `gpu_lit_side_brighter_than_shaded_side`
- `gpu_texture_tints_output`

## Deviations from Plan
- **[Rule 3 - Environment]** The C: drive was 100% full, so the first canary build failed with os error 112. I deleted the worktree's partial target dir and the main checkout's regenerable, gitignored `target/debug` (including incremental), which freed about 17 GB. The canary steps were then rerun from the start of the build and passed. No source changes. The next local build in the main checkout will be a cold rebuild.
- The `! grep` to `if grep` fix described above came from the plan review.

## Pending: Task 3 (human_needed)
The windowed `cargo run --locked -p conventional-3d --example basic_scene` check was not performed, as instructed. A human still needs to confirm the lit textured cubes, directional shading, ground plane, depth, no jitter, and the WASD/RMB-look/Shift/Esc controls, using the steps in the plan. The user's response is not yet recorded.

## Self-Check: PASSED
