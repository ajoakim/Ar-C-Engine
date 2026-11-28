# ADR-0020: Generator v3 Relief and Height-Derived Normals

- Status: proposed
- Date: 2026-11-27
- Requirement/Phase: TERR-01..06 / Phase 11

## Context

User feedback after the Phase 8 flight: "the surface is smooth. adding different elevations hills mountains". Generator v2 (ADR-0015) lands have a median inclination of about 0.3 degrees (derived from the measured `1 - cos` p50 of about 1e-5 in ADR-0015 item 5), so relief does not read under the directional sun. Phase 11 adds relief at several scales, checks that the existing per-vertex normals agree across patch and LOD seams, recalibrates the rock ramp, and keeps cost bounded (D-05). All work stays in the planetary extension; the generic render API gains nothing planetary. `TerrainConfig::default()` stays `demo_v2` until a human flight approves v3 (D-07).

## Decision

1. Layers, additive versioning. `TerrainParams.generator_version` accepts 1, 2 or 3; v1 and v2 paths are untouched (their goldens and the v2 mesh byte golden still pass). `TerrainParams::demo_v3()` is `demo_v2()` with version 3 and the constants below. Two new layers, `planet.hills` and `planet.mountains` (layer version 3, own seeds), are generated only for version >= 3. `compose_v3` is the v2 sum plus `hills * (1 - 0.6 g) + mountains * g`, with `g = smoothstep(0.44, 0.74, mask)` over the existing mask, floored at sea level. Test `v3_without_new_layers_equals_v2` guards that zero-amplitude v3 equals v2 bit for bit. There is no vertical-exaggeration parameter (D-02).

   | Layer | Amplitude | Frequency | Octaves |
   |-------|-----------|-----------|---------|
   | base (unchanged) | 3000 m | 1.5 | 5 |
   | detail (unchanged) | 800 m | 24 | 14 |
   | ridge (v3 retune) | 1800 m (v2: 2500) | 12 | 3 (v2: 5) |
   | hills (new) | 600 m | 200 | 4 |
   | mountains (new, `r*r`, gated) | 3500 m | 470 | 5 |
   | gate | smoothstep over mask 0.44..0.74 (width 0.30) | | |

   Octave total 34 for v3 (v2: 27, derived from the defaults). `max_relief()` is 9700 m.

2. Normals method. Payload v4 already carries a height-derived per-vertex normal (interior vertices: grid difference of the displaced surface; border vertices: face-independent 4-tap stencil), and `lit_textured.wgsl` already shades with it. Per D-01 there is no payload v5, the 52-byte vertex layout is unchanged and the shader is unchanged. The v3-only change is the border stencil step: v3 uses `sample_angle0 / 2^level` (function `border_normal_at`), generator < 3 calls the untouched v2 function. This is not a no-op on the hex grid: `HexSphere::new(16).sample_angle0() / (pi/2/16) = 0.442751`; on the cube `FRAC_PI_2 / n` equals `sample_angle0`, so the cube is unchanged.

3. Rock thresholds. `ROCK_SLOPE_LO_V3 = 0.01`, `ROCK_SLOPE_HI_V3 = 0.10` (`1 - cos`), from the measured land slope percentiles (p50 2.06 degrees, p90 14, p99 27). v2 keeps its old constants (guard test `v2_meshes_still_use_the_v2_thresholds`).

4. LodModel v3 bound. A v3 branch in `LodModel::from_terrain` adds proven terms: gate change `min(1, 1.5/(b-a) V_mask)`, hills `A_h (2 V_h + 0.6 dg)`, mountains `A_m (min(4 V_m', 1) + dg)`, residual capped at the global span (derivation in the doc comment). A 2000-pair soundness test (levels 1..12, cube and hex) is strict and passes; v1 and v2 residual arrays are pinned bit-equal.

5. Cross-LOD normal gate. Normals are not morphed (ADR-0014). The test gates on per-class median <= 0.5 degrees and p99 <= 12 degrees, with a negative control (a 15 degree tilt fails the gate).

6. v3 morph colour. The v3 test of morph target colour against the parent mesh uses an absolute bound of 0.15 (v2 keeps 0.05); measured worst row difference 0.1115.

## Alternatives Considered

- Payload v5 with a header flag for normals: rejected (D-01); the normal already exists in v4 and v1/v2 goldens must stay bit-identical.
- Packed or octahedral normals: rejected for this phase; they need a generic `VertexLayout` change.
- Analytic-gradient normals: rejected; about 5x the sample cost against the D-05 bound (design value, from research).
- A new gradient-noise primitive: rejected; the existing value-noise fbm and ridge shapes meet the slope bands.
- Fewer octaves to reduce the cross-LOD normal tail (mountains 2 + hills 2, detail 14 to 6, ridge 3 to 1): tried, max angle stayed 24.9 to 27.5 degrees and the D-02 slope band would break; constants kept.
- Vertical exaggeration knob: rejected by D-02.

## Consequences

- Worker mesh time grows by 18 to 26 percent over v2 (gen3/gen2 1.180 to 1.264, limit 1.5). Hex descent worst-band p95 is 4.06 ms against 4.12 ms for v2 (limit 8 ms); selection grows (hex max nodes 942 to 1439 in the 10-100 km band).
- Known limitation, cross-LOD normal tail: median 0.22 to 0.34 degrees, p99 9.5 to 11.8 degrees, but p99.9 16 to 21 degrees and max 21 to 32 degrees are not gated. The source is the f470 mountains layer at grid scale and crease outliers at fine levels. Normals are not morphed (ADR-0014); Phase 12 is where normals blend across levels.
- v3 morph colour of near-coincident vertices differs from the parent mesh by up to 0.1115 of the ramp (bound 0.15), because the child stencil leaves the child grid and takes the parent border stencil.
- The measured v3 LOD residual is much larger than v2 (cube L10 2238 m vs 68 m, L12 561 m vs 18 m) but relative looseness is lower (v3 2.5 to 8.5, v2 8 to 15).
- Bump detail (colour-based or procedural fine detail normal perturbation) is deferred (D-06); height-based normals only.
- Packed normals and normal morphing are to be considered with Phase 12.
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
