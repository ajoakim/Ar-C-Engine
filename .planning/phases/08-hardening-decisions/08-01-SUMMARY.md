---
phase: 08-hardening-decisions
plan: 01
subsystem: ci
tags: [ci, goldens, cross-os, evidence]
requires: []
provides:
  - "Hosted CI evidence for 07.1 commits (run 45947523739, sha 5d98afe)"
affects: [08-05]
key-files:
  created:
    - .planning/phases/08-hardening-decisions/08-CI-EVIDENCE.md
  modified: []
decisions: []
metrics:
  completed: 2026-11-27
  tasks: 3
---

# Phase 8 Plan 01: Hosted CI Confirmation Summary

Hosted CI run 45947523739 on 5d98afe is green on ubuntu, windows, macos, canary and cargo-deny, and the v2 height, hex mesh (v3/v4), hex dirs, mesh and cube-select CPU goldens pass on Linux and macOS with no drift.

## Tasks

1. Confirm 07.1 commits on origin: 2d4d1f2, d89895a, 5d98afe are all ancestors of origin/master (5d98afe). No push needed; none made.
2. Identify and record the run: found by headSha match (installed `gh` lacks `--commit` on `run list`), watched to completion, results written to 08-CI-EVIDENCE.md.
3. Fix failures: skipped, no failures.

## Deviations from Plan

- The `gh run list --commit` flag in the plan's verify command does not exist in the installed gh; the run was pinned by filtering `headSha` instead. No code change.
- The plan said morph/geomorph goldens; there is no hashed morph golden, only invariant tests. Recorded as such in the evidence file.
- STATE.md / ROADMAP.md bookkeeping (state.advance-plan, roadmap update, requirements mark-complete for QA-01) was intentionally not run: plan 08-02 runs in parallel and edits those files. The orchestrator should run it after the wave.

## Commits

Docs commit for this plan (evidence and summary) only; no source changes.

## Self-Check: PASSED
