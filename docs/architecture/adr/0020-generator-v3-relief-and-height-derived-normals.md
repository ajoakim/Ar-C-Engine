# ADR-0020: Generator v3 Relief and Height-Derived Normals

- Status: proposed
- Date: 2026-11-27
- Requirement/Phase: TERR-01..06 / Phase 11

## Context

User feedback after the Phase 8 flight: "the surface is smooth. adding different elevations hills mountains". Generator v2 (ADR-0015) lands have a median inclination of about 0.3 degrees (derived from the measured `1 - cos` p50 of about 1e-5 in ADR-0015 item 5), so relief does not read under the directional sun. Phase 11 adds relief at several scales, checks that the existing per-vertex normals agree across patch and LOD seams, recalibrates the rock ramp, and keeps cost bounded (D-05). All work stays in the planetary extension; the generic render API gains nothing planetary. `TerrainConfig::default()` stays `demo_v2` until a human flight approves v3 (D-07).

## Decision

1. Layers, additive versioning. `TerrainParams.generator_version` accepts 1, 2 or 3; v1 and v2 paths are untouched (their goldens and the v2 mesh byte golden still pass). `TerrainParams::demo_v3()` is `demo_v2()` with version 3 and the constants below. Two new layers, `planet.hills` and `planet.mountains` (layer version 3, own seeds), are generated only for version >= 3. `compose_v3` is the v2 sum plus `hills * (1 - 0.6 g) + mountains * g`, with `g = smoothstep(0.44, 0.74, mask)` over the existing mask, floored at sea level. Test `v3_without_new_layers_equals_v2` guards that zero-amplitude v3 equals v2 bit for bit. D-02 originally excluded any vertical-exaggeration parameter; it was superseded by a user decision, see item 7.

   | Layer | Amplitude | Frequency | Octaves |
   |-------|-----------|-----------|---------|
   | base (unchanged) | 3000 m | 1.5 | 5 |
   | detail (unchanged) | 800 m | 24 | 14 |
   | ridge (v3 retune) | 1800 m (v2: 2500) | 12 | 3 (v2: 5) |
   | hills (new) | 600 m | 200 | 4 |
   | mountains (new, `r*r`, gated; 3500 m before 11-06C) | 3900 m | 470 | 5 |
   | gate | smoothstep over mask 0.44..0.74 (width 0.30) | | |

   Octave total 34 for v3 (v2: 27, derived from the defaults). `max_relief()` is 10100 m (9700 m before 11-06C).

2. Normals method. Payload v4 already carries a height-derived per-vertex normal (interior vertices: grid difference of the displaced surface; border vertices: face-independent 4-tap stencil), and `lit_textured.wgsl` already shades with it. Per D-01 there is no payload v5, the 52-byte vertex layout is unchanged and the shader is unchanged. The v3-only change is the border stencil step: v3 uses `sample_angle0 / 2^level` (function `border_normal_at`), generator < 3 calls the untouched v2 function. This is not a no-op on the hex grid: `HexSphere::new(16).sample_angle0() / (pi/2/16) = 0.442751`; on the cube `FRAC_PI_2 / n` equals `sample_angle0`, so the cube is unchanged.

3. Rock thresholds. `ROCK_SLOPE_LO_V3 = 0.01`, `ROCK_SLOPE_HI_V3 = 0.10` (`1 - cos`), from the measured land slope percentiles (p50 2.06 degrees, p90 14, p99 27). v2 keeps its old constants (guard test `v2_meshes_still_use_the_v2_thresholds`).

4. LodModel v3 bound. A v3 branch in `LodModel::from_terrain` adds proven terms: gate change `min(1, 1.5/(b-a) V_mask)`, hills `A_h (2 V_h + 0.6 dg)`, mountains `A_m (min(4 V_m', 1) + dg)`, residual capped at the global span (derivation in the doc comment). A 2000-pair soundness test (levels 1..12, cube and hex) is strict and passes; v1 and v2 residual arrays are pinned bit-equal.

5. Cross-LOD normal gate. Normals are not morphed (ADR-0014). The test gates on per-class median <= 0.5 degrees and p99 <= 12 degrees, with a negative control (a 15 degree tilt fails the gate).

6. v3 morph colour. The v3 test of morph target colour against the parent mesh uses an absolute bound of 0.15 (v2 keeps 0.05); measured worst row difference 0.1115.

7. Relief exaggeration (`relief_scale`), user decision of 2026-11-27 that supersedes D-02. After flying v3 with the sun fix (11-06A) the user saw only gentle rolling hills and asked for hills and mountains to read clearly, with the height settable by config. `TerrainParams.relief_scale: f64` (default 1.0; v3 validation requires finite and `0.1..=8.0`) multiplies the ridge, hills and mountains contributions in `compose_v3` and nothing else (base, detail, mask and the gate are unscaled). `max_relief()` is `base + detail + (ridge + hills + mountains) * relief_scale`, and the v3 `LodModel` residual terms for those three layers carry the same factor (they are linear in the amplitudes), so the global height range and the selection bounds stay sound. The generator version stays 3 (a parameter, not a new generator) and `TerrainParams` stays `Copy`. v1 and v2 ignore the field. At 1.0 every height, mask, `max_relief` and residual is bit-identical to the v3 before the field existed (fingerprint test `scale_one_is_bit_identical_to_pre_change_v3`; v1/v2 residual pins and the v2 mesh byte golden unchanged). Plumbing: `AppSettings::set_relief_scale` and the `planetary_flight --relief <scale>` flag (usage error when non-finite or out of range); the startup line prints `relief scale`. The soundness test (2000 direction pairs, levels 1..12, cube and hex) runs strictly at 1, 2 and 3. Because base and detail are not scaled, the land height range grows by less than the scale (measured x1.73 at 2, x2.47 at 3). The calibrated slope and relief bands of `relief_v3.rs` are defined at 1.0 only. This change alters `terrain.rs`, so the earlier flight approval of 11-06 is void and the flight must be repeated.

8. Crease rounding (11-06C), found after the user flew high relief and saw a regular sawtooth along mountain tops. Root cause (`11-SAWTOOTH-DIAGNOSIS.md`): the mountains shape `A r^2` with `r = 1 - |2 fbm - 1|` has a C0 crease at every crest; linear interpolation between mesh vertices cuts a crest by about `slope_jump * step` and the vertex row crosses the crest at a varying phase, so a row of teeth with the vertex spacing appears. The relative error (error / step) does not shrink with a finer mesh and grows linearly with `relief_scale` (diagnosis: max 0.5 / 1.0 / 1.5 step at relief 1 / 2 / 3, about 4 / 8 / 12 px at the 2 px LOD budget). Fix (user-approved ranked option 1): in the v3 shapes only, `|x|` is replaced by `ax = sqrt(x*x + eps*eps) - eps`, `eps = 0.1` (`terrain::round_abs`, `CREASE_EPS`). `ax` is 1-Lipschitz, `0 <= ax <= |x|`, so `r = 1 - ax` stays in `[0, 1]`, `r*r <= 1`, and the LOD residual and height bounds (items 4 and 7) stay sound and `max_relief` is unaffected by the rounding. It uses only `*`, `+`, `-` and `sqrt` (IEEE correctly rounded, not in the `banned_ops` list), inside the border-critical region; it is a pure function of the direction, so shared-border and cross-LOD consistency hold. The same rounding applies to the v3 ridge through the new `ridge_shape_v3` / `ridge_at_v3` (the cube `RidgeLayer` uses it when `generator_version >= 3`); v2 `ridge_shape` / `ridge_at` and every v1/v2 path are untouched (v2 mesh byte golden, `GOLDEN_HEIGHTS_V1/V2` and the v1/v2 residual pins pass unchanged). Layer names, versions and seeds are unchanged. Consequences: v3 heights change at relief 1.0 (v3 is not golden and was never approved; the `relief_scale.rs` fingerprint was re-recorded: 0x1b54f6ebf399a5d9 -> 0x611242bdbf33ae2a), and the rounding lowered the land slope p99 below the D-02 band (24.10 deg at cube L8), so the mountains amplitude was re-tuned 3500 -> 3900 m (within the approved bands; max_relief 9700 -> 10100 m, inside 9-11 km). The inert-layer test now compares v3 with `compose_v2` of the v3 ridge value (the ridge differs from v2 by design). Regression guard: `tests/crease_v3.rs` interpolates true heights at 17 m and 67 m vertex steps along lines through 320 mountain-mask sites and bounds error / step at `0.05 * relief_scale`; measured (max over sites) 0.005 / 0.010 / 0.015 at step 17 and 0.022 / 0.044 / 0.067 at step 67 for relief 1 / 2 / 3, against 0.25 / 0.50 / 0.74 for the old abs shape (the negative control, kept only inside the test, fails the bound). This change alters `terrain.rs`, so any earlier flight approval is void and the flight must be repeated at `--relief 1`, `2` and `3`.

## Alternatives Considered

- Payload v5 with a header flag for normals: rejected (D-01); the normal already exists in v4 and v1/v2 goldens must stay bit-identical.
- Packed or octahedral normals: rejected for this phase; they need a generic `VertexLayout` change.
- Analytic-gradient normals: rejected; about 5x the sample cost against the D-05 bound (design value, from research).
- A new gradient-noise primitive: rejected; the existing value-noise fbm and ridge shapes meet the slope bands.
- Fewer octaves to reduce the cross-LOD normal tail (mountains 2 + hills 2, detail 14 to 6, ridge 3 to 1): tried, max angle stayed 24.9 to 27.5 degrees and the D-02 slope band would break; constants kept.
- Vertical exaggeration knob: rejected by D-02, then adopted on 2026-11-27 (item 7).

## Consequences

- Worker mesh time grows by 18 to 26 percent over v2 (gen3/gen2 1.180 to 1.264, limit 1.5). Hex descent worst-band p95 is 4.06 ms against 4.12 ms for v2 (limit 8 ms); selection grows (hex max nodes 942 to 1439 in the 10-100 km band).
- Known limitation, cross-LOD normal tail: median 0.22 to 0.34 degrees, p99 9.5 to 11.8 degrees, but p99.9 16 to 21 degrees and max 21 to 32 degrees are not gated. The source is the f470 mountains layer at grid scale and crease outliers at fine levels. Normals are not morphed (ADR-0014); Phase 12 is where normals blend across levels.
- v3 morph colour of near-coincident vertices differs from the parent mesh by up to 0.1115 of the ramp (bound 0.15), because the child stencil leaves the child grid and takes the parent border stencil.
- The measured v3 LOD residual is much larger than v2 (cube L10 2238 m vs 68 m, L12 561 m vs 18 m) but relative looseness is lower (v3 2.5 to 8.5, v2 8 to 15).
- Bump detail (colour-based or procedural fine detail normal perturbation) is deferred (D-06); height-based normals only.
- Packed normals and normal morphing are to be considered with Phase 12.
- Cost at higher relief scales (11-06B, release, `--bench-descent --frames 600 --generator 3 --budget-ms 8 --strict`, worst-band p95; mesh build cost is per vertex and does not depend on the scale, `mesh-ratio-check.sh 3 2 1.5` at 1.0: 1.146 / 1.154 / 1.252 / 1.276, PASS):

  | relief scale | hex worst-band p95 ms | hex result | cube worst-band p95 ms | cube result |
  |--------------|-----------------------|------------|------------------------|-------------|
  | 1 | 4.04 | PASS | 3.76 | PASS |
  | 2 | 3.94 | PASS | 3.99 | PASS |
  | 3 | 3.98 | PASS | 3.90 | PASS |
  | 8 | 4.17 | PASS | 3.27 | PASS |

  Higher scales raise the selection node count slightly (hex max nodes 1439 at 1, 1481 at 2, 1459 at 3, 1556 at 8) but the p95 stays near 4 ms against the unchanged 8 ms gate. One 8.48 ms single-frame max occurred at scale 8 on hex (1-10 km band, p95 4.17 ms); the gate is on p95. The gate is not loosened. Supported range: the whole accepted range 0.1..=8.0 passes the gate on this machine, with 1 to 3 the intended and measured-in-depth range (the soundness test runs at 1, 2 and 3 only).
- Cost after 11-06C (release; fresh criterion, `mesh-ratio-check.sh 3 2 1.5`: 1.197 / 1.214 / 1.271 / 1.294 PASS; strict descent, worst-band p95 ms):

  | relief scale | hex | cube |
  |--------------|-----|------|
  | 1 | 4.67 PASS | 3.74 PASS |
  | 2 | 3.69 PASS | 4.19 PASS |
  | 3 | 3.73 PASS | 3.85 PASS |

  The 8 ms gate is unchanged. Sawtooth fix measurements: land slope p99 26.48 / 32.84 / 27.04 deg (cube L8 / cube L10 / hex L8), p50 1.81 / 2.04 / 1.80; mountain share 0.266; plains 0.297; land range 7228 m (x1.85 v2), observed max 7029 m <= 10100 m; cross-LOD normal p99 8.6-10.2 deg (gate 12), median 0.22-0.33 (gate 0.5).
- Open: the default flip, v3 goldens and acceptance of this ADR wait for the human flight and plan 07.

## Evidence

Calibrated relief, 240 seeded directions, land vertices (11-03):

| mesh | slope p50 deg | p90 | p99 | max | plains <1 deg |
|------|---------------|-----|-----|-----|---------------|
| cube L8 n32 | 2.06 | 14.13 | 26.69 | 45.75 | 0.280 |
| cube L10 n32 | 2.41 | 19.05 | 33.87 | 53.54 | 0.263 |
| hex L8 n16 | 1.88 | 13.33 | 26.02 | 45.06 | 0.300 |

Land height range 6659 m versus v2 3900 m (x1.71); observed max 6460 m <= 9700 m.

Rock shares on real v3 land (11-05): rock_t == 0 is 0.776 / 0.689 / 0.751, rock_t == 1 is 0.012 / 0.041 / 0.012, between 0.212 / 0.271 / 0.237 (cube L8, cube L10, hex L8).

LOD bound looseness (residual / max measured, v3 vs v2; 11-03): cube L4 2.83 vs 7.90, L8 4.69 vs 11.26, L12 8.46 vs 11.63; hex L4 2.66 vs 8.91, L8 8.10 vs 11.81, L12 8.36 vs 12.00.

Normals (11-05): same-level shared border normals bit-equal on cube and hex (negative control fails at 0.1 degree); median angle to radial on land 3.34 degrees (cube) and 1.89 (hex); interior normals equal the grid-difference normal to 3.0e-8; border-row jump mean 2.527 degrees (cube) and 2.071 (hex) against interior baselines 2.569 and 2.161.

Cross-LOD normal angle, degrees (11-05):

| class | p50 | p99 | p99.9 | max |
|-------|-----|-----|-------|-----|
| cube parent border | 0.237 | 9.967 | 16.032 | 21.034 |
| cube parent interior | 0.223 | 9.462 | 16.260 | 27.513 |
| hex parent border | 0.307 | 10.408 | 15.909 | 25.983 |
| hex parent interior | 0.344 | 11.822 | 21.227 | 32.349 |

Cost, same session (11-06): mesh build gen3/gen2 hexagon 1.264, pentagon 1.180, cube L4 1.243, cube L10 1.185 (`MESH RATIO PASS`, limit 1.5). Strict hex v3 descent p95 per band 0.60, 1.38, 3.24, 4.06, 3.83, 3.92 ms (limit 8), `RESULT: PASS`. Hex v2 for reference 0.56, 0.98, 2.13, 3.99, 4.12, 4.11 ms. Cube p95 worst band 3.74 ms (v3) vs 2.96 ms (v2).

Flown constants: `terrain.rs` blob 07439dc86cd9273e3d4627b8e3b6625195936ee0 (to be re-confirmed by the verdict line in 11-06-SUMMARY.md).
