---
phase: 11-terrain-relief-normals
plan: 02
subsystem: planetary-core terrain generator
tags: [generator-v3, hills, mountains, additive, guard]
requires: [11-01]
provides:
  - TerrainParams hills_*, mountains_*, range_gate_lo/hi fields and TerrainParams::demo_v3()
  - hills_at, mountains_at, smoothstep, compose_v3, HillsLayer, MountainsLayer (planetary_core::terrain::*)
  - LayerSeeds.hills / LayerSeeds.mountains
  - generator_version 3 in sample(), patch_samples() and the cube LayerStack
affects: [11-03, 11-04, 11-10]
key-files:
  created:
    - extensions/planetary/planetary-core/tests/generator_v3.rs
  modified:
    - extensions/planetary/planetary-core/src/terrain.rs
    - extensions/planetary/planetary-core/tests/generator.rs
requirements-completed: []
completed: 2026-11-27
---

# Phase 11 Plan 02: Generator v3 hills and mountains Summary

Generator version 3 adds a small-scale hills layer and a mask-gated large-scale mountains layer to planetary-core, purely additively; v1/v2 paths are untouched.

## Commits

- see git log: `feat(11-02): generator v3 hills and mountains layers (additive)`

## What was added

- Constants `HILLS_LAYER = "planet.hills"`, `MOUNTAINS_LAYER = "planet.mountains"`, `V3_LAYER_VERSION = 3`.
- `TerrainParams` fields (ignored below version 3): hills 600 m / f64 / 4 oct, mountains 3500 m / f40 / 5 oct, gate smoothstep over mask 0.50..0.70. `demo_v3()` = `demo_v2()` + version 3, ridge 1800 m, 3 octaves. There is no vertical-exaggeration parameter (D-02). All values are STARTING ESTIMATES, calibrated in plan 03.
- `max_relief()` for v3 = sum of the five amplitudes. `validate()` accepts 1..=3; for v3 checks finiteness, amplitudes >= 0 (zero allowed), `0 <= gate_lo < gate_hi <= 1`, and joins hills/mountains to the shared octave/frequency rules.
- Inside BORDER-CRITICAL: `hills_at`, `mountains_at`, `smoothstep` (polynomial, `clamp`), `compose_v3` = v2 sum order verbatim + `hills * (1 - 0.6 g) + mtn * g`, floored at sea level.
- `HillsLayer`/`MountainsLayer` generators pushed to the cube stack only for version >= 3; `sample` and `patch_samples` have a leading `>= 3` branch.
- Only pre-existing lines changed: the version range check and its message, and the `if p.generator_version >= 2` line (now `else if` after the new v3 branch).

## Tests (all pass)

- `tests/generator_v3.rs` (7): `v3_without_new_layers_equals_v2` (4 patches via stack + 2000 directions bit-equal), `v3_is_deterministic_and_seed_dependent`, `v3_stack_equals_pure_function`, `v3_heights_stay_within_max_relief`, `v3_adds_relief_over_v2`, `v3_threads_1_and_4_agree` (hash printed, not pinned), `v3_validation`.
- `tests/generator.rs`: only `validation_covers_generator_version_and_ridge_fields` edited (version 3 now Ok via `demo_v3()`, version 4 rejected in the bad list; 0 still rejected).
- Whole `cargo test -p planetary-core --locked` passes, including GOLDEN_HEIGHTS_V1/V2, GOLDEN_DIRS_V1, banned_ops, surface_v2 and the 11-01 v2 mesh golden, unmodified. fmt and workspace clippy -D warnings clean.
- No GOLDEN_HEIGHTS_V3 recorded (plan 03 calibrates, plan 07 freezes).

## Deviations from Plan

None in behaviour. Notes: `smoothstep` uses `f64::clamp` instead of the plan's comparison chain (clippy `manual_clamp` forbids the chain; banned_ops does not ban `clamp`, inputs are finite and `b > a` by validation). Tests import the new items via `planetary_core::terrain::` because lib.rs re-exports were not in the plan's file list.

## Known Stubs

None.

## Self-Check: PASSED
