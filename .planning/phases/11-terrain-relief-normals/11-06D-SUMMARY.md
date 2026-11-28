---
phase: 11-terrain-relief-normals
plan: 06D
subsystem: planetary-core terrain
tags: [terrain, generator-v3, height-distribution, range-height, ambient, gap-fix]
requires: [11-06C]
provides: [range_height_at, mountains_shape, MOUNTAIN_SHAPE_SLOPE, height_dist_v3 test]
key-files:
  modified:
    - extensions/planetary/planetary-core/src/terrain.rs
    - extensions/planetary/planetary-core/src/lod/model.rs
    - extensions/planetary/planetary-core/tests/relief_scale.rs
    - extensions/planetary/planetary-core/tests/crease_v3.rs
    - extensions/planetary/planetary-app/src/lib.rs
    - docs/architecture/adr/0020-generator-v3-relief-and-height-derived-normals.md
    - docs/architecture/adr/0015-scale-contrast-and-distance-haze.md
  created:
    - extensions/planetary/planetary-core/tests/height_dist_v3.rs
requirements: [TERR-01]
---

# Phase 11 Plan 06D: Bell-shaped v3 mountain heights Summary

The v3 mountains term is now multiplied by a bounded, approximately normal range-height field, so crest heights vary (cv 0.036 -> 0.161 on the mountain shape, 0.158 -> 0.208 on total crest heights) instead of all sitting at the amplitude; app ambient light is 0.10.

## Task 1: measurement (before, 11-06C v3, relief 1)
- `mountains_at / A`: mean 0.796, median 0.840, p75 0.953, p95 0.998, skew -0.83. The brief's hypothesis was right but stronger: it is a plateau at the ceiling over most of the gated region, not only equal crest lines. Crest value of the shape: 0.992 of A, cv 0.036, 97 percent of crests above 0.9 A.
- Total crest heights above sea (700 hill-climbed crests, gate above 0.9): mean 5242 m, std 826 (cv 0.158), skew -0.31, p5 3873, p25 4670, p50 5320, p75 5872, p95 6485, p99 6904, max 7342.
- Land above sea (60000 directions): mean 1990, std 1382, skew 0.89, kurtosis 3.2, p5 267, p25 933, p50 1666, p75 2790, p95 4774, p99 5812.
- Cause: a 5-octave fbm stays near 0.5, so `1 - round_abs(2 f - 1)` is near 1 almost everywhere and `A r^2` saturates; nothing varied heights at range scale.

## Task 2: fix (generator >= 3 only)
- `range_height_at` (three one-octave fields at 40, 120, 320, seeds from `splitmix64` of the mountains seed, mean, `M = 0.35 + 0.65 clamp(0.55 + 1.8 (avg - 0.5), 0, 1)`, range [0.35, 1], mean about 0.7); `mountains_shape = r^2 * M`; `mountains_at = A * mountains_shape`. Exact ops only; pure function of the direction. `max_relief` formula unchanged (M r^2 <= 1).
- LodModel mountains term: `A rs min(4 c V_m + (1 - lo) G mean(V_i), 1) + A rs dg` with `c = MOUNTAIN_SHAPE_SLOPE = 0.8` (true max 0.784, verified on a 2e6 grid by a test). The tighter constant was needed: the looseness-vs-v2 gate failed at cube L10 (17.45 vs 11.87) with the generic 4.
- Retune within bands: mountains amplitude 3900 -> 4700 m (max_relief 10900 m), gate 0.44..0.74 -> 0.42..0.72 (mountain share 0.325). A crest sharpening gain (1.2 to 2) was tried and dropped (slope p99 39 deg at cube L10 at gain 2; LOD bound grew). No ridged-multifractal weighting added.
- After (relief 1): land mean 2357 m, std 1646, skew 0.77, p50 1964, p95 5588, p99 6646; crest heights mean 4913, std 1023 (cv 0.208), skew -0.26, p5 3119, p25 4229, p50 4964, p75 5614, p95 6491, p99 7091; crest shape mean 0.758, cv 0.161, 12 percent above 0.9 A; in a 300 km mountain window crest cv 0.101 vs 0.034.
- New `tests/height_dist_v3.rs` (4 tests): bell/unimodal/peaks-differ bounds, negative control with the old shape (violates crest cv, crest fraction, peak cv, peak p99/p50), shape-slope constant guard. `crease_v3.rs` control now rebuilds the `|x|` shape directly from the fbm. `relief_scale.rs` fingerprint re-recorded 0x611242bdbf33ae2a -> 0x786bbfbf93f304fe.

## Task 3: re-verification (all pass, nothing loosened)
- relief_v3 at relief 1: slope p50 2.02 / 2.33 / 2.00, p99 25.14 / 31.00 / 26.03 deg (cube L8 / L10, hex L8), plains 0.276, share 0.325, range 7856 m (x2.01 v2), observed max 7657 m <= 10900 m. The cube L8 p99 margin is thin (0.14 deg).
- lod_bounds_v3: 2000-pair soundness strict at relief 1, 2, 3 cube and hex; looseness vs v2 pass, tightest cube L10 11.60 vs 11.87 (thin).
- normals_v3: seam bit-equality, row jump (cube 2.231 vs baseline 2.252, hex 1.961 vs 2.034), cross-LOD p50 0.24-0.37 / p99 8.0-9.5 deg (gate 0.5 / 12), morph colour test pass. crease_v3 max error / step 0.004-0.076 against bound 0.05 * relief.
- Mesh ratio (`rm -rf target/criterion/mesh_build`, fresh bench) gen3/gen2: 1.298 / 1.324 / 1.443 / 1.425 PASS (limit 1.5; up from 1.20-1.29, cube is close to the limit).
- Strict descent, worst-band p95 ms (gate 8) relief 1 / 2 / 3: hex 5.83 / 4.03 / 4.46, cube 3.80 / 3.88 / 3.98, all PASS.
- Renders (offscreen software adapter, 960x540, scratchpad `png_before` / `png_after`, `hm_transects.png`): at 10 to 120 km the haze and the lack of contrast make single frames show only a subtle difference; I judge the after frames more varied (lower, softer, differing summits) but not dramatic. A 300 km height transect shows the before crests clipped at one ceiling (about 6000 m) and the after crests spread between about 4500 and 6700 m.

## Task 4: ambient
`LIGHT_AMBIENT` 0.025 -> 0.10. Checked: the only golden rendering (atmosphere_ring) uses `DirectionalLight::default()`, no other test pins pixels of the app light; the unit test compares against the constant. All planetary-app tests pass. No hemispheric term (needs a shader change that would touch render-wgpu goldens).

## Gates
fmt --check clean; clippy -D warnings clean (planetary-core, planetary-app, all targets); `cargo test -p planetary-core --locked` 35 test binaries ok, `cargo test -p planetary-app --locked` 14 ok, no failures. v2 mesh byte golden, GOLDEN_HEIGHTS_V1/V2 and v1/v2 residual pins unchanged.

## Deviations from Plan
- [Rule 1] The `crease_v3` negative control inverted the rounding of `mountains_at`, which no longer holds with the multiplier; it now rebuilds the `|x|` shape from the fbm.
- [Rule 3] Looseness gate forced the tighter shape-slope proof constant (above). Gate and amplitude retuned (bands kept).
- Peak-variation bounds are on total crest heights and on the mountain-shape crest (the advisor-noted point that total crest cv alone does not separate old from new); old shape crest cv 0.036 vs new 0.161.

## Not done on purpose
No flight verdict, no default flip, no v3 goldens, no requirement ticks, no push. `terrain.rs` changed (blob `2dcfbda10305ff2351f64d3425c32f6fcce64111`), so any flight approval is void. Re-fly with `planetary_flight --generator 3 --relief 1`, `2` and `3`. ADR-0020 stays proposed.

## Known Stubs
None.

## Self-Check: PASSED
