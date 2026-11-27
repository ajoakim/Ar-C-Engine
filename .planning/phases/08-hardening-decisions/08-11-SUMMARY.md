---
phase: 08-hardening-decisions
plan: 11
subsystem: ci
tags: [ci, github-actions, image-diff, lavapipe, warp, criterion, goldens]
requires:
  - phase: 08-03
    provides: image-diff harness, Windows WARP goldens, workflow_dispatch update path
  - phase: 08-04
    provides: criterion benches
  - phase: 08-08
    provides: egui-overlay feature and overlay_corner image test
  - phase: 08-10
    provides: atmosphere_ring planetary-app image test
provides:
  - non-blocking bench job, blocking feature-paths job
  - hosted Linux (lavapipe) goldens, ARC_REQUIRE_GOLDENS=1 on Linux and Windows
  - hosted run evidence and ADR-0005 run reference
affects: [08-12]
tech-stack:
  added: []
  patterns: [hosted golden generation by workflow_dispatch artifact, human-reviewed commit]
key-files:
  created:
    - crates/engine-render-wgpu/tests/golden/linux/ (5 PNGs)
    - extensions/planetary/planetary-app/tests/golden/linux/atmosphere_ring.png
  modified:
    - .github/workflows/ci.yml
    - .planning/phases/08-hardening-decisions/08-CI-EVIDENCE.md
    - .planning/phases/08-hardening-decisions/08-VALIDATION.md
    - docs/architecture/adr/0005-rendering-api-wgpu-behind-trait.md
    - xtask/tests/docs_policy.rs
key-decisions:
  - "Keep goldens per platform: lavapipe differs from WARP by up to 6/255 on a few pixels, so tolerances were NOT widened"
  - "Windows goldens unchanged: hosted WARP output was pixel-identical to the local WARP goldens"
  - "Serialise image tests (--test-threads=1) after one lavapipe SIGSEGV flake"
requirements-completed: [QA-01]
duration: about 3 h wall (dominated by hosted CI waits)
completed: 2026-11-27
---

# Phase 8 Plan 11: Bench job, hosted goldens and CI evidence Summary

Hosted CI now runs unit, integration, criterion (non-blocking) and image-diff tests on software adapters (lavapipe, WARP), with Linux goldens committed and missing goldens failing on Linux and Windows; pinned run 46042013890 on c44af29 is green.

## What was done

- **Task 1** (`d13973d`): `bench` job (ubuntu, `continue-on-error`, 30 min, uploads `target/criterion` as artifact `criterion`) using the explicit form `cargo bench --workspace --bench spatial --bench select --bench mesh --bench stream --bench ecs --locked -- --noplot --quick` (plan 04 carry-over: `--benches` hits libtest lib targets). Blocking `feature-paths` job (debug-overlay clippy and tests). The image-diff job gained the egui-overlay image test and `cargo test -p planetary-app --test atmosphere_ring` (plan 10 carry-over) with the same skip and require env; the update step covers both and uploads both golden directories. The 08-VALIDATION PLAN-08 row now points at the `atmosphere_ring` test.
- **Push 1** (authorised): `5d98afe..d13973d`, the 42 unpushed Phase 8 commits plus Task 1.
- **Task 3**: generated goldens through `workflow_dispatch` (run 46041602756), reviewed visually and numerically, committed Linux goldens together with the `ARC_REQUIRE_GOLDENS=1` flip for Linux and Windows only (`c44af29`). macOS stays skip-allowed with no goldens.
- **Push 2** (authorised): `d13973d..c44af29` (`1267107` CI fix and `c44af29`).
- **Task 5**: evidence appended to 08-CI-EVIDENCE.md, ADR-0005 amendment bullet with run id and adapter names, docs_policy requires "llvmpipe" and "46042013890".

## Hosted runs

| Run | Head | Result |
|-----|------|--------|
| 46037063494 (push) | d13973d | image-diff (3 OS), bench, canary, deny, build ubuntu success; feature-paths cancelled by its 30 min limit; other builds cancelled by the dispatch |
| 46041279045 (dispatch) | d13973d | ubuntu image-diff update failed with SIGSEGV on llvmpipe; others passed; cancelled |
| 46041602756 (dispatch) | d13973d | image-diff all success; goldens artifacts reviewed |
| **46042013890 (push)** | c44af29 | **success**: all 10 jobs, including image-diff on ubuntu (llvmpipe) and windows (WARP) with `ARC_REQUIRE_ADAPTER=1` and `ARC_REQUIRE_GOLDENS=1`, macOS skip with logged reason, feature-paths, bench |

Windows first-push risk (hosted WARP drifting from local goldens) did not materialise: run 46037063494 compared and passed, and hosted regeneration was pixel-identical.

## Golden review

Each PNG was viewed and compared numerically (details in 08-CI-EVIDENCE.md). Hosted WARP vs local WARP: identical on all six images. Lavapipe vs WARP: max delta 1 to 6/255, at most 1.56 percent of pixels over 2/255 (anti-aliased quad edge, morph edge). Content correct on each image (triangle, lit quad, fog bands, half morph, overlay label in the top-left corner, atmosphere disc with limb ring). Only Linux PNGs were added.

## Local gates before the first push

fmt ok; clippy `--workspace --all-targets --locked -D warnings` clean; `cargo test --workspace --locked`: **913 passed, 0 failed**; check-deps OK (20 packages); check-canary OK; docs_policy 22 passed (22 after the ADR-0005 extension). Bench targets compile with the explicit form (`--no-run`).

## Deviations from Plan

**1. [Rule 3 - Blocking] Explicit bench targets instead of `--benches`.** Plan 04 carry-over (lib targets reject criterion flags). Plan text and verify command updated in effect by using the explicit form in ci.yml.

**2. [Rule 2 - Missing] atmosphere_ring and egui-overlay golden handling.** The plan only named `image_diff`. The update step now also runs the egui-overlay image test (so the `overlay_corner` golden is produced) and the `atmosphere_ring` test; both golden dirs are uploaded.

**3. [Rule 1 - Bug] feature-paths timeout.** The planned 30 min limit cancelled the job; raised to 90 (commit 1267107). Root cause from logs: the debug-build `planetary-app` tests take about 27 min.

**4. [Rule 1 - Bug, mitigation] lavapipe SIGSEGV flake.** One update run died with signal 11 in `image_diff` on llvmpipe; the identical re-dispatch passed. Image test steps now run with `--test-threads=1` (commit 1267107). Cause not proven; mitigation validated by one green run only.

**5. Windows goldens not rewritten.** Hosted WARP was identical to the committed local goldens, so the plan's "commit hosted Linux and Windows goldens" produced Linux files only.

**6. Pushes bundled.** The second push carried the CI fix commit `1267107` along with the goldens commit, both inside the authorised second push point.

**7. The dispatch run was started twice** (first hit the SIGSEGV), and a push-triggered run was cancelled by the dispatch via the workflow concurrency group; there is therefore no completed full-build run for d13973d. Evidence for those jobs comes from run 46042013890.

## Issues, not run, caveats

- `python -I -c "import yaml..."` plan verify was not run (PyYAML missing); GitHub parsing and executing the workflow served as validation.
- Criterion `--quick` logs `base/sample.json` errors in the bench job; the step exits 0 and timings print.
- macOS has no goldens and skips by design (no software Metal adapter).
- The evidence, ADR and SUMMARY/state commits made after the second push are local only; no third push was authorised and none was made. `git log origin/master..HEAD` shows them.
- Untracked and untouched: HLD.md, flight.csv, .planning/config.json, the 07.1 .gitkeep.

## Requirements

QA-01 marked complete: unit and integration tests (build jobs, 184 `test result: ok` per OS), criterion benches (bench job, artifact `criterion`) and image-diff on software adapters (lavapipe and WARP, with goldens) all run in hosted CI and were confirmed green by run 46042013890. Plan 12 still lists QA-01 for its final sign-off.

## Self-Check: PASSED

Files verified present: the six Linux golden PNGs, ci.yml, evidence, this summary. Commits d13973d, 1267107, c44af29 are on origin/master; run ids confirmed through `gh run view`.
