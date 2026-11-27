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
status: COMPLETE (user decisions applied: percentile gate for cross-LOD, 0.15 bound for v3 morph colour)
completed: 2026-11-27
---

# Phase 11 Plan 05: v3 normals and rock ramp Summary

The v3 stencil step, v3 rock thresholds and all normal/ramp tests are in. By user decision the cross-LOD normal test gates on median and p99 (not the max), and the v3 morph colour test uses an absolute bound of 0.15; the tails are recorded as known limitations for ADR-0020.

## Commits

- 4d959ed feat(11-05): v3 border stencil step and v3 rock-slope thresholds (Task 1)
- 7f33d29 test(11-05): seam, row, height-derived, ramp tests
- follow-up test commit: cross-LOD percentile gate, v3 morph colour test, bookkeeping

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

## Cross-LOD normal agreement (decision applied: percentile gate)

User decision: gate on percentiles, not the max. `v3_cross_level_normal_agreement` (cube n32 and hex n16, levels 3..14, 300 pairs each) asserts per class `median <= MEDIAN_LOD_DEG = 0.5` (measured 0.22-0.34 plus margin) and `p99 <= P99_LOD_DEG = 12.0`, and a negative control: tilting the compared normals by 15 degrees makes the gate fail (median becomes 15.0) for every class. The old unused 8.0 `TOL_LOD_DEG` constant was removed.

| class | n | p50 | p99 | p99.9 | max |
|-------|---|-----|-----|-------|-----|
| cube parent border | 9900 | 0.237 | 9.967 | 16.032 | 21.034 |
| cube parent interior | 76800 | 0.223 | 9.462 | 16.260 | 27.513 |
| hex parent border | 18559 | 0.307 | 10.408 | 15.909 | 25.983 |
| hex parent interior | 50400 | 0.344 | 11.822 | 21.227 | 32.349 |

Cube by level (p99.9 / max): L3 2.2/2.9, L4 6.1/6.8, L5 9.8/10.7, L6 14.8/16.8, L7 18.9/23.7, L8 17.7/21.0, L9 20.3/27.5, L10 17.3/21.9, L11 13.2/15.8, L12 12.3/13.6, L13 9.1/11.3, L14 18.5/20.1.

KNOWN LIMITATION (for ADR-0020): p99.9 16-21 degrees and max 21-32 degrees are not gated. They come from the f470 mountains layer at grid scale (levels 5..10) and ridge/crease outliers at fine levels. Fewer octaves (mountains 2 + hills 2: max 24.9; detail 14 to 6: max 27.4; ridge 3 to 1: max 27.5) did not remove the tail, and would break the D-02 slope band, so relief bands and octaves are untouched. ADR text: normals are not morphed (ADR-0014); Phase 12 is where normals blend across levels.

## v3 morph target colour vs parent mesh (decision applied: 0.15 bound)

`v3_morph_target_colour_matches_the_parent_mesh` (copy of the v2 test, whose 0.05 bound is kept for v2): exact 6786, near 150, worst row difference 0.1115, asserted `< 0.15` (absolute). Reason (for ADR-0020): the near class is parent-interior vertices whose child stencil leaves the child grid and so use the parent-level border stencil instead of the parent's own grid difference; on rough v3 terrain the two stencils differ more than on v2.

## Verification

v1/v2 goldens and v2 mesh golden unchanged and passing (banned_ops, v2_mesh_golden, mesh, hex_mesh, surface_v2, seam, hex_seam, morph, cell_payload run after Task 1). `cell_payload.rs` untouched (D-01, payload stays v4). fmt and clippy -D warnings clean. TERR-03 and TERR-04 wording speaks of adding normals to patches and shading with them; per D-01 the normals already exist in payload v4 and `lit_textured.wgsl`, so these requirements are to be satisfied by verification of the existing normal path plus these tests (REQUIREMENTS.md not edited, not marked complete here; plan 11-10 does that, after Open Question 1 is resolved).

## Deviations from Plan

- Tasks 2/3 first stopped on two tolerances over their limits (cross-LOD max, v3 morph colour); resolved by the user decisions above (percentile gate; 0.15 bound). No mitigation changed terrain constants.
- Requirements are not ticked here (plan 11-10 does that). TERR-03 is satisfied by verification of the existing normal path (D-01) plus these tests.

## Known Stubs

None.
