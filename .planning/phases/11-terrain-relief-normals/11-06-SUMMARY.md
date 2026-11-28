---
phase: 11-terrain-relief-normals
plan: 06
subsystem: planetary pre-flight evidence and ADR-0020 draft
tags: [pre-flight, adr, d-05, flight-gate]
requires: [11-05]
status: PRE-FLIGHT COMPLETE, AWAITING TASK 3 HUMAN FLIGHT (plan not complete)
requirements-completed: []
---

# Phase 11 Plan 06: Pre-flight evidence and ADR-0020 draft (awaiting human flight)

Tasks 1 and 2 are done. Task 3 (human flight) has not happened. No verdict line exists, the default is still v2, no golden was recorded, no requirement was ticked.

## Pre-flight suite (HEAD 4e41986, working tree has no tracked changes)

- `cargo fmt --all -- --check`: clean.
- `cargo clippy -p planetary-core -p planetary-terrain -p planetary-app --all-targets --locked -- -D warnings`: clean.
- `cargo test -p planetary-core --locked`, `-p planetary-terrain --locked`, `-p planetary-app --locked`: all green (52 "test result: ok" lines, 0 failures).
- `grep demo_v2 extensions/planetary/planetary-terrain/src/system.rs`: present (default still v2).

## Cost bounds (one session, no other load, `rm -rf target/criterion/mesh_build` first)

Commands: `cargo bench -p planetary-core --bench mesh --locked`; `bash scripts/mesh-ratio-check.sh 3 2 1.5`; `cargo run -q --release -p planetary-app --locked --example planetary_flight -- --bench-descent --frames 600 --generator 3 --budget-ms 8 --strict`.

Mesh build, criterion median-range midpoint (ms):

| id | gen1 | gen2 | gen3 | gen3/gen2 |
|----|------|------|------|-----------|
| hexagon l8 n16 | 3.023 | 4.743 | 5.777 | 1.264 |
| pentagon l8 n16 | 2.146 | 3.580 | 4.227 | 1.180 |
| cube level4 n32 | 1.630 | 2.012 | 2.507 | 1.243 |
| cube level10 n32 | 1.660 | 2.104 | 2.538 | 1.185 |

```
ratio gen3/gen2 hex_cell?hexagon_l8_n16 = 1.264 (limit 1.5)
ratio gen3/gen2 hex_cell?pentagon_l8_n16 = 1.180 (limit 1.5)
ratio gen3/gen2 cube_patch?level4_n32 = 1.243 (limit 1.5)
ratio gen3/gen2 cube_patch?level10_n32 = 1.185 (limit 1.5)
MESH RATIO PASS
```

Gated hex v3 descent (exit 0), p95 ms per band (>1000 km, 100-1000 km, 10-100 km, 1-10 km, 100 m-1 km, <100 m): 0.60, 1.38, 3.24, 4.06, 3.83, 3.92. Worst band p95 4.06 ms (limit 8). Overall perf line p95 3.85 ms. `RESULT: PASS`.

Recorded, not gated (p95 per band; max nodes visited in worst bands 10-100 km / 1-10 km):

| run | p95 per band | max nodes |
|-----|--------------|-----------|
| hex v2 | 0.56, 0.98, 2.13, 3.99, 4.12, 4.11 | 942 / 1362 |
| hex v3 | 0.60, 1.38, 3.24, 4.06, 3.83, 3.92 | 1439 / 1439 |
| cube v2 | 0.38, 0.63, 1.18, 2.96, 1.95, 1.71 | 622 / 994 |
| cube v3 | 0.32, 0.55, 1.48, 3.74, 3.15, 2.67 | 934 / 1046 |

All four runs `RESULT: PASS`. Both D-05 bounds hold; no constants changed in this plan.

## Flown terrain.rs blob

`git hash-object extensions/planetary/planetary-core/src/terrain.rs` = `07439dc86cd9273e3d4627b8e3b6625195936ee0` (working tree equals HEAD 4e41986 blob; no tracked changes). This is the blob the user is asked to fly.

## ADR-0020

Drafted at `docs/architecture/adr/0020-generator-v3-relief-and-height-derived-normals.md`, status proposed, indexed in the ADR README.

## Pending

Task 3 human flight, then Tasks 4 to 6 (verdict line, bookkeeping) only after the user replies. Plan 07 and plan 10 must not proceed until a `Flight verdict: APPROVED ... | terrain.rs blob <hash>` line exists here.
