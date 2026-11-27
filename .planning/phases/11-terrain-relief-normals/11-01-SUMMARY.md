---
phase: 11-terrain-relief-normals
plan: 01
subsystem: planetary measurement scaffolding
tags: [bench, golden, v2-baseline, cli]
requires: []
provides:
  - generator_version axis in benches/mesh.rs (ids suffixed /gen{1,2})
  - scripts/mesh-ratio-check.sh <num> <den> <max>
  - planetary_flight --generator <1|2>
  - tests/v2_mesh_golden.rs (v2 byte golden)
  - v2 baselines for plans 04, 06
affects: [11-02, 11-04, 11-06, 11-10]
key-files:
  created:
    - scripts/mesh-ratio-check.sh
    - extensions/planetary/planetary-core/tests/v2_mesh_golden.rs
  modified:
    - extensions/planetary/planetary-core/benches/mesh.rs
    - extensions/planetary/planetary-app/examples/planetary_flight.rs
requirements-completed: []
completed: 2026-11-27
---

# Phase 11 Plan 01: Measurement scaffolding and v2 guard Summary

Generator-version axis on the mesh bench, `--generator <1|2>` on `planetary_flight`, a ratio-check script and a v2 mesh byte golden, plus the v2 baselines measured in one session.

Phase 11 base: a7d2730ee3a3085f6f23abc2ae18213e761a398e (HEAD before the first edit of this plan; plan 10 diffs against it)

## Commits

- 3564181 feat(11-01): generator_version bench axis, --generator flag, mesh ratio script
- f03c368 test(11-01): pin v2 hex-cell and cube-patch mesh byte golden

## v2 golden constants (recorded from unmodified src, Windows)

- GOLDEN_V2_HEX_HEXAGON = 0xd5fadc4ad97d1a7a (level 8, n=16, encoded payload v4)
- GOLDEN_V2_HEX_PENTAGON = 0x934f59917ac65daa
- GOLDEN_V2_CUBE_L4 = 0x018d5b5c0c91e7be (level 4, n=32)
- GOLDEN_V2_CUBE_L10 = 0xbe19670e7a02a316

Note: `PatchMesh` carries no morph targets, so the cube hash covers id, origin, bound, skirt, height range, positions, normals, uv and indices. Hex hashes use the v4 payload, which includes morph targets.

## Baselines (this machine, 2026-11-27, one session, release)

Commands: `rm -rf target/criterion/mesh_build; cargo bench -p planetary-core --bench mesh --locked`; `cargo run --release -p planetary-app --locked --example planetary_flight -- --bench-descent --frames 600 --generator 2 [--topology cube]`.

Mesh build, criterion median:

| id | gen1 ms | gen2 ms | gen2/gen1 | worker limit for v3 (1.5 x gen2) ms |
|----|---------|---------|-----------|------------------|
| hexagon l8 n16 | 2.840 | 5.119 | 1.803 | 7.68 |
| pentagon l8 n16 | 2.040 | 3.628 | 1.778 | 5.44 |
| cube level4 n32 | 1.570 | 1.923 | 1.225 | 2.88 |
| cube level10 n32 | 1.631 | 2.061 | 1.264 | 3.09 |

Observation: hex gen2/gen1 is about 1.8 here, higher than the roughly 1.23 expected from 11-RESEARCH (cube matches it). It is the same-session measurement, so later v3/v2 ratios use this table, not the research figure.

Descent (`--bench-descent --frames 600 --generator 2`), per band p95 ms:

| band | hex run 1 | hex run 2 | cube |
|------|-----------|-----------|------|
| >1000 km | 0.57 | 0.55 | 0.29 |
| 100-1000 km | 1.05 | 0.98 | 0.62 |
| 10-100 km | 2.22 | 2.28 | 1.08 |
| 1-10 km | 4.01 | 4.07 | 2.91 |
| 100 m-1 km | 4.40 | 4.18 | 1.81 |
| <100 m | 4.24 | 4.23 | 1.70 |
| worst band | 4.40 | 4.23 | 2.91 |

Overall hex perf line: p95 4.21 ms / 4.15 ms (spread about 0.06 ms). Hex descent p95 ceiling for v3: 8 ms (D-05).

## Verification

- fmt check, clippy -D warnings on planetary-core and planetary-app: clean.
- `cargo test -p planetary-core --test v2_mesh_golden`: 3 passed. `generator_flag_is_in_usage`: passed. `--generator 7` exits non-zero. `bash -n` on the script passes; the script ran against the real criterion output (PASS at limit 99, ratios above).
- No file under planetary-core/src or existing tests changed.

## Deviations from Plan

None. The cube golden omits morph data because `PatchMesh` has none (plan wording said "morph via payload"; cube patches have no payload encoder).

## Known Stubs

None.

## Self-Check: PASSED
