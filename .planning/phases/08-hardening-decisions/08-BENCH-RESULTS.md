# Phase 8 Bench Results (plan 08-04)

Real measurements from release (`bench` profile) runs of the criterion benches. Nothing here is estimated or carried over. Benches never gate CI.

## Machine

- CPU: AMD Ryzen 9 5950X, 16 cores / 32 threads, 128 GB RAM, Windows 11 Pro 10.0.26300
- Toolchain: rustc 1.99.0 (b940084d7 2026-09-28), criterion 0.8.2
- Command: `cargo bench -p <crate> --bench <name> --locked -- --noplot --quick`
- Method: each bench run 3 times in separate processes. Cells show the median of the three runs' criterion point estimates, with the (min-max) across the three runs. `--quick` uses criterion's fast sampling mode, so treat single-digit-percent differences as noise. The machine was not otherwise idle-controlled (desktop, other processes running).

## Spatial index: HashGridIndex vs LinearIndex (ADR-0006 evidence)

Bench: `crates/engine-spatial/benches/spatial.rs`. Entity density is constant across N (world half-extent 500 m * cbrt(N/1000), entity half-extents 0.1-5 m, default `HashGridConfig`: 64 m cells). Each query-case time is for a batch of **16 queries** (region: AABB half-extent 20-80 m; radius: 20-100 m; frustum: 1.0 aspect, 0.5-200 m depth), so per-query time is the cell value divided by 16. The update case moves 10% of the entities by a fixed offset and back (two passes, 2 x 0.1 N updates). N = 100 and 300 were added to the plan's 1e3/1e4/1e5 to locate the crossover.

| case | N | HashGrid | Linear | Linear / HashGrid |
|------|---|----------|--------|-------------------|
| frustum | 100 | 6.89 us (6.89 us-6.96 us) | 5.44 us (5.38 us-5.51 us) | 0.79x |
| frustum | 300 | 43.90 us (43.70 us-44.30 us) | 15.66 us (15.53 us-15.70 us) | 0.36x |
| frustum | 1000 | 53.64 us (53.56 us-53.96 us) | 92.05 us (90.98 us-92.06 us) | 1.72x |
| frustum | 10000 | 62.34 us (61.40 us-63.75 us) | 1.04 ms (1.04 ms-1.05 ms) | 16.74x |
| frustum | 100000 | 87.09 us (86.68 us-88.28 us) | 9.29 ms (9.28 ms-9.37 ms) | 106.67x |
| radius | 100 | 10.28 us (10.16 us-10.28 us) | 5.86 us (5.79 us-5.86 us) | 0.57x |
| radius | 300 | 10.92 us (10.71 us-11.04 us) | 17.12 us (17.07 us-17.19 us) | 1.57x |
| radius | 1000 | 11.71 us (11.63 us-11.71 us) | 57.47 us (57.20 us-58.25 us) | 4.91x |
| radius | 10000 | 12.05 us (11.98 us-12.10 us) | 701.58 us (699.94 us-732.77 us) | 58.24x |
| radius | 100000 | 15.00 us (14.80 us-15.51 us) | 7.10 ms (7.06 ms-7.39 ms) | 473.10x |
| region | 100 | 5.52 us (5.49 us-5.56 us) | 5.80 us (5.79 us-5.82 us) | 1.05x |
| region | 300 | 4.58 us (4.55 us-4.65 us) | 17.12 us (17.06 us-17.29 us) | 3.74x |
| region | 1000 | 4.93 us (4.92 us-4.98 us) | 70.19 us (69.60 us-70.27 us) | 14.24x |
| region | 10000 | 5.70 us (5.59 us-5.73 us) | 1.01 ms (1.01 ms-1.01 ms) | 177.19x |
| region | 100000 | 7.51 us (7.47 us-7.64 us) | 9.83 ms (9.76 ms-9.96 ms) | 1308.75x |
| update_10pct_roundtrip | 100 | 3.69 us (3.68 us-3.71 us) | 154 ns (152 ns-157 ns) | 0.04x |
| update_10pct_roundtrip | 300 | 12.18 us (12.06 us-12.48 us) | 581 ns (580 ns-586 ns) | 0.05x |
| update_10pct_roundtrip | 1000 | 44.64 us (44.41 us-44.77 us) | 2.37 us (2.35 us-2.44 us) | 0.05x |
| update_10pct_roundtrip | 10000 | 636.65 us (635.89 us-647.22 us) | 41.29 us (41.18 us-42.14 us) | 0.06x |
| update_10pct_roundtrip | 100000 | 7.66 ms (7.55 ms-8.10 ms) | 906.23 us (904.65 us-946.14 us) | 0.12x |

Conclusion: for queries the hash grid is flat in N (region 5-8 us, radius 11-15 us, frustum 54-87 us per 16 queries from N=1e3 to 1e5) while the linear scan grows linearly, reaching 9-10 ms at N=1e5 (about 100x-1300x slower). Crossover: region and radius queries break even at roughly N = 100-300 entities (hash grid is already ahead at N=300, linear is ahead or equal at N=100); frustum queries break even between N=300 and N=1000 (linear 3x faster at 300, hash grid 1.7x faster at 1000). Updates go the other way: the hash grid is 8x-24x slower than the linear scan at every N (24x at N=100, 8x at N=1e5) (it re-buckets cells and maintains a `BTreeMap`; linear update is an in-place write), e.g. 7.7 ms vs 0.91 ms for 10k moved entities at N=1e5. Consequence for ADR-0006: hash-grid-first is justified for any scene beyond a few hundred queried entities; update cost is the hash grid's weak point and the first thing to look at if dynamic-entity counts grow, and no rstar/kd-tree comparison was run (out of plan scope).

## Selection

Bench: `extensions/planetary/planetary-core/benches/select.rs`. `select_cells` with `LodParams::default()`; hex sector side 16 (`AppSettings` default), cube 32 cells per edge. `orbit` = eye at 2 R; `low_1km` = 1 km above the surface (hex: above root-cell 0 centre; cube: on +X). `cold` = empty previous split set, `warm` = previous split set from the same view. Results include building the returned `CellSelection`. Cells are median of 3 runs, then (min - max).

| case | median | range |
|------|--------|-------|
| select_cells/hex/orbit/cold | 98.44 us | 97.41 us - 98.62 us |
| select_cells/hex/orbit/warm | 99.16 us | 98.92 us - 102.92 us |
| select_cells/hex/low_1km/cold | 1.96 ms | 1.89 ms - 2.05 ms |
| select_cells/hex/low_1km/warm | 1.94 ms | 1.93 ms - 1.97 ms |
| select_cells/cube-sphere/orbit/cold | 27.97 us | 27.53 us - 27.98 us |
| select_cells/cube-sphere/orbit/warm | 27.81 us | 27.63 us - 27.92 us |
| select_cells/cube-sphere/low_1km/cold | 2.62 ms | 2.60 ms - 2.62 ms |
| select_cells/cube-sphere/low_1km/warm | 2.62 ms | 2.61 ms - 2.63 ms |

Observation: orbit selection is cheap (hex 98 us, cube 28 us). At 1 km the cost rises about 20x (hex) to 90x (cube) to 1.9-2.6 ms, because of far deeper traversal. Warm and cold are within noise of each other, so hysteresis seeding does not change the traversal cost in a static view. The cube is faster than hex in orbit but slower at low altitude here (cube default grid is 32 per edge vs hex 16, so the two are not like-for-like).

## Mesh build

Bench: `extensions/planetary/planetary-core/benches/mesh.rs`. Production grids: hex sector side 16 (`build_hex_cell`, level-8 cell, no debug colouring, never cancelled), cube 32 cells per edge (`build_patch`). Terrain built once outside the timed region.

| case | median | range |
|------|--------|-------|
| mesh_build/hex_cell/hexagon_l8_n16 | 2.84 ms | 2.82 ms - 2.85 ms |
| mesh_build/hex_cell/pentagon_l8_n16 | 2.00 ms | 1.99 ms - 2.06 ms |
| mesh_build/cube_patch/level4_n32 | 1.56 ms | 1.54 ms - 1.57 ms |
| mesh_build/cube_patch/level10_n32 | 1.60 ms | 1.56 ms - 1.62 ms |

Observation: one mesh build is 1.6-2.8 ms single-threaded on this machine (hexagon 2.84 ms, pentagon 2.00 ms, cube patch 1.56-1.60 ms; patch level has no measurable effect, as expected since the grid size is fixed).
