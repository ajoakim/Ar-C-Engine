# Phase 8 Plan 01: Hosted CI Evidence

Recorded by plan 08-01. Source: `gh run view 45947523739` (hosted GitHub Actions, workflow `ci.yml`). No results below are inferred; each comes from the run's job and step conclusions and from the Test step log.

## 07.1 commits on origin (Task 1)

After `git fetch origin`, `git merge-base --is-ancestor <sha> origin/master` succeeded for all three:

- 2d4d1f2 (07.1-18 complete in state and roadmap): on origin
- d89895a (07.1-18 roadmap labels): on origin
- 5d98afe (README for phase 7.1 / phase 8 status): on origin

`origin/master` = `5d98afe60b4954604513eb6a44f498d340cd1e31`. No push was needed or made.

## Pinned run (Task 2)

- Run id: **45947523739**
- Head sha: **5d98afe60b4954604513eb6a44f498d340cd1e31** ("docs: update README for phase 7.1 completion and phase 8 status")
- URL: https://github.com/ajoakim/Ar-C-Engine/actions/runs/45947523739
- Final status: completed, conclusion **success**
- Pinning: the installed `gh` has no `--commit` flag on `gh run list`, so the run was selected by matching `headSha` to 5d98afe in `gh run list --json headSha`. It was in progress when found and was watched to completion.

| Job | Job id | Conclusion | Test step | Wall time (log first/last line) |
|-----|--------|------------|-----------|-------------------------------|
| build (ubuntu-latest) | 140335532865 | success | success | ~38 min (Test finished 06:21 UTC) |
| build (windows-latest) | 140335532888 | success | success | ~38 min (Complete job 06:24 UTC) |
| build (macos-latest) | 140335532850 | success | success | ~57 min (Complete job 06:40 UTC) |
| canary (planetary crates absent) | 140335532609 | success | n/a | under 1 min |
| cargo-deny | 140335532941 | success | n/a | under 1 min |

The API `completedAt` fields for the build jobs were not reliable (all showed 05:44:06Z), so durations are taken from the log timestamps. Every step in every job concluded success or skipped. The Test step logs show 170 `test result: ok` lines per OS and no `FAILED` or `panicked` lines. The negative dependency-direction steps, the `basic_scene` build and the `planetary_flight` headless run also succeeded on all three OS jobs.

## CPU goldens cross-OS

The tests assert the stored constants, so a pass means the Linux and macOS values equal the Windows-recorded constants. The `eprintln!` value lines are not in the CI log because tests ran without `--nocapture`; the pass itself is the evidence.

| Golden (constant) | Test | ubuntu | windows | macos |
|-------------------|------|--------|---------|-------|
| GOLDEN_HEIGHTS_V1 / GOLDEN_DIRS_V1 | `goldens_match_at_1_and_4_threads_v1` (planetary-core/tests/generator.rs) | pass | pass | pass |
| GOLDEN_HEIGHTS_V2 = 0x3096173351c359fb | `goldens_match_at_1_and_4_threads_v2` (generator.rs) | pass | pass | pass |
| GOLDEN_HEX_DIRS | `golden_hex_dirs` (hex_mapping.rs) | pass | pass | pass |
| GOLDEN_HEX_MESH_V3 | `golden_hex_mesh_v3` (hex_mesh.rs) | pass | pass | pass |
| GOLDEN_HEX_MESH_V4 | `golden_hex_mesh_v4` (hex_mesh.rs) | pass | pass | pass |
| GOLDEN_MESH | `deterministic_and_golden` (mesh.rs) | pass | pass | pass |
| GOLDEN_HASH / GOLDEN_COUNT | `cube_select_matches_pre_refactor_goldens` (topology_cube.rs) | pass | pass | pass |

Other golden-named tests in `golden.rs`, `id_golden_values` and `noise_golden_bits` also passed on all three OS.

Morph/geomorph: there is no hashed morph golden constant (`rg GOLDEN_` finds none in morph code). Morph coverage is invariant tests (for example `tests/morph.rs`, `morph_handover.rs`, `hex_seam.rs`, `descent.rs`); the 52 test lines containing "morph" passed on each of ubuntu, windows and macos. The hex prepare path is covered by the hex mesh goldens above; no separate prepare golden exists.

Result: no cross-OS drift. The "cross-platform CI pending" notes on GOLDEN_MESH and the 07.1 v2/hex/morph items can be closed by run 45947523739 (plan 05 ADR amendment cites this run).

## Fixes applied

None. All jobs succeeded; Task 3 skipped with "no failures". No test was changed, ignored or weakened, and no files outside this evidence file were modified.

## Final green run

Run 45947523739 on commit 5d98afe60b4954604513eb6a44f498d340cd1e31.

Note: later local commits (Phase 8 docs) are ahead of origin and have no CI run; this evidence covers 5d98afe only.
