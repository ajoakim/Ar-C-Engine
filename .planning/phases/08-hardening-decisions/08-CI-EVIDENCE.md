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

# Plan 08-11: image-diff, feature-paths and bench evidence

Recorded by plan 08-11 from real hosted runs. Pinning: the installed `gh` has no `--commit` flag on `gh run list`, so runs were selected by matching `headSha`.

## Pushes

1. `5d98afe..d13973d` (`git push origin master`): all local Phase 8 commits plus the CI commit `d13973d` (bench job, `feature-paths` job, egui-overlay and `atmosphere_ring` image steps).
2. `d13973d..c44af29`: `1267107` (CI fix, below) and `c44af29` (Linux goldens plus `ARC_REQUIRE_GOLDENS=1` for Linux and Windows, in one commit).

## Run 46037063494 (push, head d13973d, cancelled by a later dispatch)

| Job | Result |
|-----|--------|
| image-diff ubuntu / windows / macos | success. Ubuntu: lavapipe, every scene `SKIP ... no golden` (goldens absent). Windows: WARP compared against the committed local WARP goldens and passed (9 + 10 + 2 tests ok). macOS: skip with reason |
| bench (non-blocking) | success |
| canary, cargo-deny | success |
| build ubuntu | success |
| feature paths | **cancelled**: the 30 min `timeout-minutes` was reached while the debug-overlay Test step was still running (clippy passed; the `planetary-app` descent test alone took 327 s). Fixed by raising the limit to 90 (commit 1267107) |
| build windows / macos | cancelled by the follow-up workflow_dispatch (same concurrency group) |

## workflow_dispatch update runs

- 46041279045: `image-diff (ubuntu)` failed in the update step with `signal: 11, SIGSEGV` inside the `image_diff` test binary on llvmpipe, after writing three goldens. Windows and macOS jobs passed. It did not reproduce in the next dispatch or in any verify run, so it is treated as a rare lavapipe flake; the mitigation is `--test-threads=1` on every image test step (commit 1267107). Run 46042013890 passed with it, which is weak evidence only (one run); if the SIGSEGV recurs it is open evidence.
- 46041602756: all three image-diff jobs succeeded; artifacts `goldens-ubuntu-latest`, `goldens-windows-latest`, `goldens-macos-latest` downloaded and reviewed (below). The run was cancelled when the goldens push began.

## Golden review (T-08-22)

Each hosted PNG was viewed (64x64) and compared numerically with the local WARP goldens (per-channel max delta, pixels over the 2/255 tolerance):

| Scene | hosted WARP vs local WARP | lavapipe vs hosted WARP |
|-------|---------------------------|-------------------------|
| unlit_triangle | identical (0 px differ) | max 1, 198 px differ by 1, none over 2 |
| lit_quad_directional | identical | max 4, 64 px over 2 (1.56%, anti-aliased quad edge) |
| fog_gradient | identical | max 1, none over 2 |
| morph_half | identical | max 6, 32 px over 2 (0.78%, interpolation at morph edge) |
| overlay_corner | identical | max 2, none over 2 |
| atmosphere_ring (planetary-app) | identical | max 1, 1180 px differ by 1 |

Visual content matched expectations on the Linux images viewed and the local atmosphere_ring: vertex-coloured triangle, lit gradient quad, four-step fog gradient, half-morphed quad, the "ab" overlay label in a yellow top-left corner over the triangle, and a pale planet disc with a limb ring and sky tint. Because hosted WARP equals local WARP pixel for pixel, the Windows goldens were not rewritten. Only Linux goldens were added (`crates/engine-render-wgpu/tests/golden/linux/` five PNGs, `extensions/planetary/planetary-app/tests/golden/linux/atmosphere_ring.png`). The lavapipe-vs-WARP differences exceed the 2/255 tolerance for a few pixels on two scenes, which is why goldens are per platform; lavapipe renders were compared with lavapipe goldens in run 46042013890 and passed, so tolerances were not widened. The macOS artifact contained only the committed Windows PNGs (the macOS job skipped); no macOS goldens exist.

## Final pinned run 46042013890 (push, head c44af2928bf5829d814dad02108a5c3cdd371ea3)

URL: https://github.com/ajoakim/Ar-C-Engine/actions/runs/46042013890. Status completed, conclusion **success**.

| Job | Job id | Conclusion | Notes |
|-----|--------|------------|-------|
| build (ubuntu-latest) | 140651957382 | success | 184 `test result: ok`, 0 failures |
| build (windows-latest) | 140651957447 | success | 184 ok, 0 failures |
| build (macos-latest) | 140651957223 | success | 184 ok, 0 failures |
| image-diff (ubuntu-latest) | 140651956915 | success | llvmpipe (LLVM 21.1.8, 256 bits) (Vulkan); `ARC_REQUIRE_ADAPTER=1`, `ARC_REQUIRE_GOLDENS=1`; image_diff 9 ok, egui-overlay image_diff 10 ok, atmosphere_ring 2 ok |
| image-diff (windows-latest) | 140651957487 | success | Microsoft Basic Render Driver (Dx12); both env flags 1; 9 / 10 / 2 ok |
| image-diff (macos-latest) | 140651957422 | success (skips) | both env flags empty; every GPU test prints `SKIP ...: no software adapter: backend error: no software (fallback) adapter: No suitable graphics adapter found; ... metal had no fallback adapters`; 9 / 10 / 2 ok |
| feature paths (debug-overlay) | 140651957424 | success | clippy `-D warnings` and `cargo test -p planetary-app --features debug-overlay`, 12 `test result: ok` |
| bench (non-blocking) | 140651957469 | success | explicit `--bench spatial --bench select --bench mesh --bench stream --bench ecs`; artifact `criterion` (184 KB). The log contains `Criterion.rs ERROR ... base/sample.json: No such file` lines from `--quick` mode with no baseline; the step exited 0 and timings were printed |
| canary | 140651957462 | success | |
| cargo-deny | 140651957410 | success | |

The bench job is `continue-on-error` and has no `needs`, so it cannot block. The plan's local `yaml.safe_load` check was not run (PyYAML is not installed); GitHub parsed and ran the workflow instead.
