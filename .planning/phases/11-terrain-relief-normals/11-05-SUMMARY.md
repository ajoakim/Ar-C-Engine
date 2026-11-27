---
phase: 11-terrain-relief-normals
plan: 05
subsystem: planetary-core normals and surface ramp (generator v3)
tags: [generator-v3, normals, seams, rock-ramp]
requires: [11-04]
provides:
  - border_normal_at / border_normal_step_f64 (v3-only true-grid-angle border stencil)
  - ROCK_SLOPE_LO_V3 / ROCK_SLOPE_HI_V3 / ramp_value_v3
  - tests/normals_v3.rs (seam, row, height-derived, negative control)
  - tests/surface_v3.rs (ramp endpoints, real-land shares, v2 dispatch guard)
affects: [11-06, 11-07, 11-10]
key-files:
  created:
    - extensions/planetary/planetary-core/tests/normals_v3.rs
    - extensions/planetary/planetary-core/tests/surface_v3.rs
  modified:
    - extensions/planetary/planetary-core/src/mesh.rs
    - extensions/planetary/planetary-core/src/hex/mesh.rs
    - extensions/planetary/planetary-core/src/hex/morph.rs
    - extensions/planetary/planetary-core/src/hex/topology.rs
    - extensions/planetary/planetary-core/src/topology/cube.rs
    - extensions/planetary/planetary-core/src/lib.rs
requirements-completed: []
status: PARTIAL - two measured tolerances exceed their limits (open questions below)
completed: 2026-11-27
---

# Phase 11 Plan 05: v3 normals and rock ramp Summary

The v3 stencil step and v3 rock thresholds are in and verified. The cross-LOD normal tolerance (TERR-03 evidence) and the v3 morph-colour check could NOT be met and were not committed as passing tests; the numbers are below and need a user decision.

## Commits

- 4d959ed feat(11-05): v3 border stencil step and v3 rock-slope thresholds (Task 1)
- test commit for Tasks 2/3 (normals_v3.rs, surface_v3.rs) and this SUMMARY: see git log

## A7 (stencil step)

`HexSphere::new(16).sample_angle0() / (pi/2/16) = 0.442751`. Not a no-op: the cube is unchanged (`FRAC_PI_2 / n` is exactly the cube `sample_angle0`), the hex v3 border stencil step is 0.443 of the old constant step. Implemented as `border_normal_at(terrain, level, d, sample_angle0)`: `generator_version < 3` calls the untouched `border_normal_f64`; v3 uses `sample_angle0 / 2^level`. Cube callers pass `FRAC_PI_2 / n`; hex callers use `hex_sample_angle0(n)` (new `pub(crate)` helper that `HexSphere::sample_angle0` now also uses). Morph targets (`uv_at_level`, `uv_sampled`) carry the parent-level step through the same function.

## Rock thresholds

`ROCK_SLOPE_LO_V3 = 0.01`, `ROCK_SLOPE_HI_V3 = 0.10`, calibrated from the plan 03 land slope percentiles (1 - cos: p50 2.06 deg = 6.5e-4, p90 14 deg = 0.030, p99 27 deg = 0.11). Measured shares on real v3 land (slope from stored normals):

| mesh | land n | rock_t == 0 | rock_t == 1 | between |
|------|--------|-------------|-------------|---------|
| cube L8 | 127460 | 0.776 | 0.012 | 0.212 |
| cube L10 | 127605 | 0.689 | 0.041 | 0.271 |
| hex L8 | 108306 | 0.751 | 0.012 | 0.237 |

Asserted: dry >= 0.20, full <= 0.50, between >= 0.05. v2 uses the old constants (guard test `v2_meshes_still_use_the_v2_thresholds`).

## Measured and passing

- Same-level shared border normals bit-equal on cube (levels 0..2 all + 12 seeded level-8 patches) and hex (levels 0..2 incl. pentagons + 12 level-8 cells); 0.1 degree perturbation fails the same harness (negative control).
- Height-derived: median angle(normal, radial) on land is 3.34 deg (cube), 1.89 deg (hex), both > 1; interior normals equal the grid-difference normal of the displaced positions to 3.0e-8 per component (f32 payload).
- Row jump (border row vs first interior row), degrees: cube mean 2.527 (p50 0.881, p99 17.3, max 60.3) vs interior baseline mean 2.569; hex mean 2.071 (p50 0.735, p99 15.0, max 32.7) vs baseline 2.161. `TOL_ROW_DEG = 4.0` (cap) asserted on the mean; the border row is no worse than the interior baseline.

## OPEN QUESTION 1: cross-LOD normal tolerance exceeds the 8 degree cap

Coincident vertex normals of a level-L mesh vs its level-(L+1) child (cube n32: 300 patches over levels 3..14; hex n16: same sampling via lattice points), degrees:

| class | n | p50 | p99 | p99.9 | max |
|-------|---|-----|-----|-------|-----|
| cube parent border | 9900 | 0.237 | 9.967 | 16.032 | 21.034 |
| cube parent interior | 76800 | 0.223 | 9.462 | 16.260 | 27.513 |
| hex parent border | 18559 | 0.307 | 10.408 | 15.909 | 25.983 |
| hex parent interior | 50400 | 0.344 | 11.822 | 21.227 | 32.349 |

Cube by level (p99.9 / max): L3 2.2/2.9, L4 6.1/6.8, L5 9.8/10.7, L6 14.8/16.8, L7 18.9/23.7, L8 17.7/21.0, L9 20.3/27.5, L10 17.3/21.9, L11 13.2/15.8, L12 12.3/13.6, L13 9.1/11.3, L14 18.5/20.1. The medians are tiny (< 0.5 deg); the tail comes from the f470 mountains layer at grid scale (levels 5..10) and ridge/crease outliers at fine levels.

Mitigation (1) tried (cube, same harness): mountains 2 oct + hills 2 oct: max 24.9, p99.9 14.0; mountains 5 oct, hills 4, detail 14->6: max 27.4; ridge 3->1: max 27.5. None gets near the cap (8 deg max, p99.9 <= 5.33), and reducing mountain/hill octaves would break the D-02 slope band (`relief_v3`, p99 25..35 deg), which must not be weakened. Per the plan I stopped here and did not raise the cap. The cross-level test is NOT committed (it would be red; marking it ignored would be weakening). Its full source is saved at `C:\Users\Arpa\AppData\Local\Temp\claude\C--development-Ar-c-engine\6313538b-fdb3-47ea-acfa-fa8e7e203d5d\scratchpad\cross_level_section.rs` (needs the helpers of normals_v3.rs). `TOL_LOD_DEG = 8.0` remains in normals_v3.rs as the unused cap constant.

Options for the user: accept a recorded limitation with measured percentiles (ADR-0014 style; p50 0.2-0.3 deg, p99 about 10 deg), pin tolerances to the measured values (about 33 deg max / 21.3 p99.9), reshape the v3 mountains (lower high-octave amplitude, which needs a new D-02 band decision), or restrict the tolerance to a percentile (for example p99 <= 12) instead of the max.

## OPEN QUESTION 2: v3 morph target colour vs parent mesh

`v3_morph_target_colour_matches_the_parent_mesh` (copy of the v2 test, source at `...\scratchpad\v3_morph_colour_test.rs`): exact 6786, near 150, worst row difference 0.1115 (v2 cap in the copied test is 0.05). The `near` class is the existing design (parent interior vertex whose child stencil leaves the child grid, so the parent-level border stencil is used instead of the parent's own grid difference); on rough v3 terrain these stencils differ more than on v2. Not committed; needs a decision on the bound (0.15 would cover the measurement) or making those vertices sample the parent's exact grid stencil (extra samples in morph code).

## Verification

v1/v2 goldens and v2 mesh golden unchanged and passing (banned_ops, v2_mesh_golden, mesh, hex_mesh, surface_v2, seam, hex_seam, morph, cell_payload run after Task 1). `cell_payload.rs` untouched (D-01, payload stays v4). fmt and clippy -D warnings clean. TERR-03 and TERR-04 wording speaks of adding normals to patches and shading with them; per D-01 the normals already exist in payload v4 and `lit_textured.wgsl`, so these requirements are to be satisfied by verification of the existing normal path plus these tests (REQUIREMENTS.md not edited, not marked complete here; plan 11-10 does that, after Open Question 1 is resolved).

## Deviations from Plan

- Cross-LOD test (Task 2 Test 3) and the v3 morph colour test (Task 3 Test 4) not committed (see open questions); plan's mitigation order exhausted for Test 3 without being able to meet the cap.
- ROADMAP tick for 11-05 NOT made (plan goal not fully achieved).

## Known Stubs

None.
