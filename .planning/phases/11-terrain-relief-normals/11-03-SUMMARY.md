---
phase: 11-terrain-relief-normals
plan: 03
subsystem: planetary-core terrain calibration and LOD bounds
tags: [generator-v3, calibration, lod-bounds]
requires: [11-02]
provides:
  - calibrated v3 constants (TerrainParams default v3 fields, demo_v3)
  - tests/relief_v3.rs (slope, mask, relief bands)
  - LodModel v3 branch and tests/lod_bounds_v3.rs
affects: [11-04, 11-07, 11-10]
key-files:
  created:
    - extensions/planetary/planetary-core/tests/relief_v3.rs
    - extensions/planetary/planetary-core/tests/lod_bounds_v3.rs
  modified:
    - extensions/planetary/planetary-core/src/terrain.rs
    - extensions/planetary/planetary-core/src/lod/model.rs
requirements-completed: []
completed: 2026-11-27
---

# Phase 11 Plan 03: Relief calibration and v3 LOD bounds Summary

Generator v3 constants calibrated to the D-02 bands (no exaggeration knob) and a sound v3 branch in `LodModel` with proven terms.

## Commits

- d19b399 feat(11-03): calibrate v3 relief constants, relief_v3 bands
- 7271f7a feat(11-03): v3 LOD bounds with proven terms, relative looseness test

## Calibrated constants

| Constant | Plan 02 start | Calibrated |
|----------|---------------|------------|
| hills | 600 m, f64, 4 oct | 600 m, f200, 4 oct |
| mountains | 3500 m, f40, 5 oct | 3500 m, f470, 5 oct |
| gate | 0.50..0.70 | 0.44..0.74 (width 0.30) |
| ridge (demo_v3) | 1800 m, 3 oct | unchanged |

Octave total 34. `max_relief()` = 9700 m. Not golden-pinned (plan 07).

## Measured bands (240 seeded directions, real meshes, land vertices)

| mesh | p50 deg | p90 | p99 | max | plains <1 deg |
|------|---------|-----|-----|-----|---------------|
| cube L8 n32 | 2.06 | 14.13 | 26.69 | 45.75 | 0.280 |
| cube L10 n32 | 2.41 | 19.05 | 33.87 | 53.54 | 0.263 |
| hex L8 n16 | 1.88 | 13.33 | 26.02 | 45.06 | 0.300 |

Mask on land: p10 0.345, p25 0.429, p50 0.518, p75 0.598, p90 0.671, p99 0.766. Mountain share (g > 0.5) 0.271. Land height range 6659 m vs v2 3900 m (x1.71); p99 above sea x1.64; observed max 6460 m <= 9700.

## Iteration log (hills f / mountains f / gate -> cube L8)

1. 64 / 40 / 0.50..0.70: p99 3.68 deg, plains 0.77, gate width 0.2 (fail)
2. 300 / 120 / 0.44..0.74: p99 9.82, plains 0.226 (slope fail)
3. 300 / 400 / 0.44..0.74: p99 23.94, plains 0.179 (fail)
4. 200 / 500 / 0.44..0.74: p99 27.13, cube L10 p99 34.95 (too close to edge)
5. 200 / 470 / 0.44..0.74: all bands pass (committed)

## LOD bound terms (proved in the `from_terrain` doc comment)

Gate change `min(1, 1.5/(b-a) V_mask)`; hills `A_h (2 V_h + 0.6 dg)`; mountains `A_m (min(4 V_m', 1) + dg)`; v3 residual capped at the global span. v1/v2 blocks untouched; their residual arrays and `height.max` are pinned bit-equal in `v1_and_v2_residuals_are_unchanged`. The 2000-pair soundness test (levels 1..12, cube and hex) is strict and passes.

## Looseness (residual / max measured over 2000 pairs, same harness)

Deviation: the plan's absolute `ratio <= 6` was an unmeasured guess; the accepted demo_v2 baseline sits at 8-15. By decision of the coordinator it was replaced by: v3 ratio <= v2 ratio at every level 4..=12 on both topologies (all ratios printed). `noise_gradient_bound` untouched; no v3-only gradient constant.

| level | cube v3 | cube v2 | hex v3 | hex v2 |
|-------|---------|---------|--------|--------|
| 4 | 2.83 | 7.90 | 2.66 | 8.91 |
| 5 | 2.65 | 11.69 | 2.55 | 10.91 |
| 6 | 2.70 | 11.93 | 3.22 | 11.55 |
| 7 | 2.57 | 10.90 | 3.76 | 11.45 |
| 8 | 4.69 | 11.26 | 8.10 | 11.81 |
| 9 | 6.71 | 14.03 | 7.32 | 14.47 |
| 10 | 6.39 | 11.87 | 6.44 | 12.83 |
| 11 | 7.45 | 14.29 | 7.65 | 15.11 |
| 12 | 8.46 | 11.63 | 8.36 | 12.00 |

Residuals (m), v3 vs v2: cube L10 2238 vs 68, cube L12 561 vs 18, hex L12 249 vs 8.

## Risk for plan 11-04

The absolute v3 residual is far larger than v2 (560 m at L12 and 2238 m at L10 vs 18 m and 68 m). It follows from the f470 mountains layer the slope band requires. It inflates derived height ranges and so LOD selection cost; it is the real risk for the plan 04 gates (mesh ratio <= 1.5, hex descent p95 < 8 ms, v2 baseline 4.2-4.4 ms). Cost is measured and gated in plan 04, not here.

An extra test assertion (not plan-mandated): v3 residual > v2 residual for levels 6..=24 (below that both saturate at the global span).

## Verification

fmt and clippy -D warnings clean; whole `cargo test -p planetary-core --locked` run before the final commit; existing lod_bounds, lod_select, lod_balance, select_slack unmodified and passing; v1/v2 goldens and v2 mesh golden unchanged.

## Known Stubs

None.

## Self-Check: PASSED
