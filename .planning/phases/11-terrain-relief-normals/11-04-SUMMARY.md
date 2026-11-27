---
phase: 11-terrain-relief-normals
plan: 04
subsystem: planetary cost gates
tags: [bench, descent, d-05]
requires: [11-03]
provides: [gen3 bench axis, planetary_flight --generator 3, v3 cost evidence]
key-files:
  modified:
    - extensions/planetary/planetary-core/benches/mesh.rs
    - extensions/planetary/planetary-app/examples/planetary_flight.rs
requirements-completed: []
completed: 2026-11-27
---

# Phase 11 Plan 04: v3 cost bounds Summary

Both D-05 bounds pass on the first measurement with the plan 03 constants; no retuning was needed.

## Commits

- 2b5f6ac feat(11-04): gen3 bench axis and --generator 3

## Mesh cost (one session: `rm -rf target/criterion/mesh_build; cargo bench -p planetary-core --bench mesh --locked`)

| id | gen1 ms | gen2 ms | gen3 ms | gen3/gen2 | gen2/gen1 |
|----|---------|---------|---------|-----------|-----------|
| hexagon l8 n16 | 2.806 | 4.244 | 5.109 | 1.204 | 1.513 |
| pentagon l8 n16 | 1.992 | 3.309 | 4.135 | 1.250 | 1.661 |
| cube level4 n32 | 1.535 | 1.851 | 2.392 | 1.292 | 1.206 |
| cube level10 n32 | 1.633 | 1.912 | 2.477 | 1.295 | 1.171 |

`bash scripts/mesh-ratio-check.sh 3 2 1.5` output:

```
ratio gen3/gen2 hex_cell?hexagon_l8_n16 = 1.204 (limit 1.5)
ratio gen3/gen2 hex_cell?pentagon_l8_n16 = 1.250 (limit 1.5)
ratio gen3/gen2 cube_patch?level4_n32 = 1.292 (limit 1.5)
ratio gen3/gen2 cube_patch?level10_n32 = 1.295 (limit 1.5)
MESH RATIO PASS
```

Note: the v2 numbers drifted between this session and 11-01 (hexagon gen2 4.24 vs 5.12 ms), which is why v2 was re-measured back to back.

## Hex descent (gated), `--bench-descent --frames 600 --generator 3 --budget-ms 8 --strict`, exit 0

```
band | frames | avg ms | p95 ms | max ms | fps | stalls | max nodes | max wanted | max drawn | max held | max pending
>1000 km | 61 | 0.32 | 0.64 | 0.90 | 3077.7 | 0 | 307 | 226 | 12 | 13 | 0
100-1000 km | 75 | 0.73 | 1.03 | 1.19 | 1367.2 | 0 | 534 | 377 | 24 | 33 | 0
10-100 km | 76 | 1.52 | 3.10 | 3.88 | 657.8 | 0 | 1439 | 893 | 44 | 76 | 0
1-10 km | 76 | 3.61 | 4.09 | 4.69 | 276.7 | 0 | 1439 | 900 | 109 | 176 | 0
100 m-1 km | 76 | 3.61 | 4.09 | 4.25 | 277.1 | 0 | 1327 | 900 | 120 | 277 | 0
<100 m | 236 | 2.93 | 3.86 | 4.03 | 341.5 | 0 | 1340 | 900 | 171 | 529 | 0
budget: p95 <= 8 ms per band
RESULT: PASS
```

## Recorded runs (not gated)

p95 ms per band (>1000 km, 100-1000 km, 10-100 km, 1-10 km, 100 m-1 km, <100 m); max nodes / max wanted in the worst-selection bands:

| run | p95 per band | max nodes (10-100 km / 1-10 km) | max wanted |
|-----|--------------|-------------------------------|------------|
| hex v2 | 0.56, 0.98, 2.18, 3.93, 4.15, 4.10 | 942 / 1362 | 775 / 900 |
| hex v3 | 0.64, 1.03, 3.10, 4.09, 4.09, 3.86 | 1439 / 1439 | 893 / 900 |
| cube v2 | 0.35, 0.67, 1.26, 3.04, 1.94, 1.66 | 622 / 994 | 900 / 900 |
| cube v3 | 0.33, 0.60, 1.41, 3.56, 3.14, 2.70 | 934 / 1046 | 900 / 900 |

Selection grows with v3 (hex max nodes visited 1362 -> 1439 at most, 942 -> 1439 in the 10-100 km band) as the plan 03 risk predicted, but the worst-band p95 is essentially unchanged (hex 4.15 -> 4.09 ms) and well under 8 ms. Cube v3 worst band 3.56 ms vs 3.04 ms v2.

Plan 03 LOD bound looseness (unchanged here): v3 ratio 2.5-8.5 vs v2 8-15 across levels 4..12 on cube and hex.

## D-05 result

PASS for mesh ratio (max 1.295 <= 1.5) and hex descent (worst p95 4.09 ms < 8 ms). No constants, bands, or budgets changed. No golden recorded.

## Deviations from Plan

None. Clippy required `matches!(v, 1..=3)` instead of `1 | 2 | 3` (manual_range_patterns); semantics identical.

## Known Stubs

None.

## Self-Check: PASSED
