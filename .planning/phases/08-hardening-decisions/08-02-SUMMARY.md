---
phase: 08-hardening-decisions
plan: 02
subsystem: planning, workspace
tags: [carry-over, workspace-deps, bookkeeping]
requires: []
provides: ["07-VERIFICATION.md gaps closed", "STATE.md reflects Phase 8", "workspace pins png, criterion, egui, egui-wgpu, hecs"]
affects: [08-04, 08-05, 08-06, 08-08]
key-files:
  modified:
    - .planning/phases/07-planetary-extension-terrain-lod/07-VERIFICATION.md
    - .planning/STATE.md
    - .planning/ROADMAP.md
    - Cargo.toml
requirements: [QA-03, QA-01]
metrics:
  tasks: 2
  completed: 2026-11-26
---

# Phase 8 Plan 02: Carry-overs and workspace pins Summary

07-VERIFICATION.md now reports `gaps_closed` with G1-G3 closed by Phase 07.1, STATE.md points at Phase 8, and five dependencies are pinned in root `[workspace.dependencies]` with no lockfile change.

## Commits
- 6000d04 docs: 07-VERIFICATION gaps closed
- 9601e7f docs: STATE focus, position, metrics
- 089bde7 chore: workspace pins

## What was done
- **07-VERIFICATION.md:** frontmatter `status: gaps_closed`; G1, G2, G3 each `status: closed` with a `closed_by:` line citing 07.1-01/02/03/05/17/18/20/24/27 and 07.1-DIAGNOSIS.md. History kept. An "Update (Phase 8)" section records the human flight approval with the user's verbatim caveat and C1-C4, states that the closure is bounded by those caveats, and points the Linux/macOS CI item at `08-CI-EVIDENCE.md` (plan 08-01; file may not exist yet, and the frontmatter says "see 08-01").
- **STATE.md:** Current focus is Phase 8; status executing; stopped_at, position block and Session Continuity updated; completed_phases 8, total_plans 90, completed_plans 79, 88%; per-phase table now has rows for 7, 07.1 (27) and 8 (1/12). Decisions list untouched.
- **ROADMAP.md:** 08-02 ticked; Phase 8 progress row 1/12 In progress.
- **Cargo.toml:** `png = "0.18.1"`, `criterion` (default-features off, `cargo_bench_support`), `egui` (default-features off, `default_fonts`), `egui-wgpu` (default-features off), `hecs = "0.11.2"`, under a comment line. No member uses them, so nothing enters Cargo.lock. deny.toml was not edited: its wrapper rules for egui-wgpu belong to plan 08-07 / 08-06.

## Checks
- `cargo metadata --locked --no-deps --format-version 1` ok and `git diff --exit-code Cargo.lock` clean.
- `cargo metadata --locked` (full resolve) ok.
- `check-deps` OK (18 packages); `check-canary` OK (289 packages, none planetary); `cargo test -p xtask --test docs_policy` 13 passed.
- Task 1 verify: no `gaps_found` left in 07-VERIFICATION.md; "Phase 8" present in STATE.md.

## Deviations
- STATE.md edited by hand rather than via `state.advance-plan` (known to fail on STATE.md parsing; not attempted). The `total_plans`/`completed_plans` counts were set from counting PLAN and SUMMARY files on disk (90 / 79).
- Observed, not fixed: Phase 3 has 7 plans but 6 summaries on disk; the existing STATE "By Phase" table was already inconsistent for earlier phases.

## Not run
- Full `cargo build`/test of the workspace (no code changed), `cargo deny`, fmt/clippy.
- `gsd-sdk` state helpers (hand edit as above).
- Nothing pushed.

## Self-Check: PASSED
