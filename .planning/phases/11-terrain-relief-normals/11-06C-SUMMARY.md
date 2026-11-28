---
phase: 11-terrain-relief-normals
plan: 06C
subsystem: planetary-core terrain
tags: [terrain, generator-v3, crease, sawtooth, gap-fix]
requires: [11-06B]
provides: [round_abs, ridge_shape_v3, ridge_at_v3, crease_v3 regression test]
key-files:
  modified:
    - extensions/planetary/planetary-core/src/terrain.rs
    - extensions/planetary/planetary-core/tests/generator_v3.rs
    - extensions/planetary/planetary-core/tests/relief_scale.rs
    - docs/architecture/adr/0020-generator-v3-relief-and-height-derived-normals.md
  created:
    - extensions/planetary/planetary-core/tests/crease_v3.rs
requirements: [TERR-01, TERR-03]
---

# Phase 11 Plan 06C: Crease rounding for v3 mountains and ridge Summary

The v3 mountains shape and a new v3-only ridge shape replace `|x|` with `sqrt(x*x + 0.1^2) - 0.1`, which removes the C0 crest crease that produced the sawtooth at high `--relief`. The crease interpolation error per vertex step drops by 10-30x.

## What changed
- `terrain.rs`: `CREASE_EPS = 0.1`, `round_abs`, `ridge_shape_v3`, `ridge_at_v3`; `mountains_at` uses `round_abs`; `sample()` and the cube `RidgeLayer` use the v3 ridge for `generator_version >= 3`. v2 `ridge_shape`/`ridge_at` and all v1/v2 paths are untouched. Only `*`, `+`, `-`, `sqrt` (not in the `banned_ops` list; scan passes). `ax` is 1-Lipschitz and `r` stays in [0, 1], so the LodModel terms and `max_relief` bounds are unchanged and sound.
- Retune: after the rounding the land slope p99 fell to 24.10 deg (cube L8), below the 25-35 band. Mountains amplitude 3500 -> 3900 m (max_relief 9700 -> 10100 m, inside 9-11 km). Nothing loosened.
- `relief_scale.rs` fingerprint re-recorded: 0x1b54f6ebf399a5d9 -> 0x611242bdbf33ae2a (relief 1.0 stays an exact no-op; not a v3 golden).
- `generator_v3.rs`: the inert-layer test compared v3 with v2, which no longer holds because the ridge differs by design; it now compares against `compose_v2` of base, detail, mask and the v3 ridge value. The stack-equals-pure test uses `ridge_at_v3`.
- New `tests/crease_v3.rs` (2 tests): 320 mountain-mask sites, true heights interpolated at 17 m and 67 m steps (3 sub-points per interval, 32 intervals), bound `0.05 * relief_scale` on max error / step. Measured new max: 0.0053 / 0.0102 / 0.0152 at step 17 and 0.0223 / 0.0442 / 0.0667 at step 67 for relief 1 / 2 / 3. Negative control (old abs shape reconstructed inside the test by inverting the rounding): 0.248 / 0.496 / 0.743 at step 17, exceeds the bound.

## Re-verification (release for benches, debug for tests)
- relief_v3 at relief 1: land slope p50 1.81 / 2.04 / 1.80 deg, p99 26.48 / 32.84 / 27.04 deg (cube L8 / cube L10 / hex L8); plains 0.297; mountain share 0.266; land range 7228 m (x1.85 v2); observed max 7029 m <= 10100 m. All bands pass.
- LOD soundness (2000 pairs, cube and hex, relief 1, 2, 3) strict pass; looseness-vs-v2 pass.
- Normals: seam bit-equality pass; row jump cube mean 2.277 (baseline 2.303), hex 1.883 (1.954); cross-LOD p50 0.22-0.33, p99 8.6-10.2 deg (gate 0.5 / 12); morph colour test pass (0.15 bound).
- Mesh ratio (`rm -rf target/criterion/mesh_build`, fresh bench): `mesh-ratio-check.sh 3 2 1.5` = 1.197 / 1.214 / 1.271 / 1.294 PASS.
- Strict descent, worst-band p95 ms (gate 8): hex 4.67 / 3.69 / 3.73, cube 3.74 / 4.19 / 3.85 at relief 1 / 2 / 3, all PASS.
- fmt --check clean; clippy -D warnings clean (planetary-core, planetary-app); `cargo test -p planetary-core --locked` and `-p planetary-app --locked` all green. v2 mesh byte golden, GOLDEN_HEIGHTS_V1/V2 and v1/v2 residual pins pass unchanged.

## Deviations from Plan
- [Rule 1 - test invariant] "v3 with zero hills/mountains equals v2" cannot hold once the v3 ridge is rounded; the test was rewritten as described above.
- [Re-tune] Mountains amplitude 3500 -> 3900 m to restore the slope p99 band (documented in ADR-0020 item 8). The Default `TerrainParams` carries the same value but it is unused for v1/v2.

## Not done on purpose
No flight verdict, no default flip, no v3 goldens, no requirement ticks, no push. `terrain.rs` changed (blob `f7b5095beddcfe574509ef396951a782956a2522`), so the 11-06 flight approval is void: the user must re-fly with `--generator 3 --relief 1`, `2` and `3`. ADR-0020 stays proposed.

## Known Stubs
None.

## Self-Check: PASSED
