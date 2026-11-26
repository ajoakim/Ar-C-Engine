# ADR-0012: Terrain LOD Selection - Tight Bounds, Culling and Budgets

- Status: accepted
- Date: 2026-11-26
- Requirement/Phase: PLAN-03, PLAN-07, PERF-02 / Phase 7.1

## Context

Phase 7 selection used relief-inflated bounding spheres and a soft leaf cap with deterministic `tau *= 1.5` escalation (ADR-0010). Below about 4 km altitude the cap could not be met even at the escalation limit: wanted leaves jumped from about 660 to 164000 in one frame, selection visited about 2.0 million nodes per frame, and frames took 0.7 s (release) to 10 s (debug). Detail never streamed in on descent. Diagnosis: `.planning/phases/07.1-hex-grid-planet-topology-descent-performance/07.1-DIAGNOSIS.md`.

## Decision

- **Extruded-footprint distance.** The distance used by the screen-space-error test is measured to the patch footprint extruded by its height range, not to an inflated sphere.
- **Measured ranges.** A patch's height range comes from the nearest resident ancestor's measured `height_min/max` widened by `residual(L)` of that ancestor's level `L`, which bounds the height change between any point of a level-`L` patch and the ancestor's sampled heights (derived from the generator's amplitude and wavelengths in `LodModel`). Until a mesh is resident the conservative global `max_relief` applies.
- **Horizon culling.** A node is skipped when it lies beyond the horizon, tested with exact-op arithmetic (angle-sum test using sqrt-only cosines and sines, ADR-0008 rules; no libm trig).
- **Frustum culling with tight bounds.** The app passes the camera frustum planes (`PlanetaryCamera::frustum_planes`, unit normals, sqrt only; aspect tracked from resize events and never narrower than 16:9) in the `View`. Nodes fully outside are not refined. Draw submission also culls effective-cut leaves; the cell under the eye is exempt. Culled leaves stay in the drawn set so the partition and 2:1 balance invariants keep their meaning.
- **Best-first traversal with a node-visit budget** of 8192 (`LodParams::max_nodes_visited`). Refinement proceeds by largest screen-space error first, so exhausting the budget loses the least visible detail.
- **Leaf budget 900** (`LodParams::max_leaves`) enforced by deepest-level trimming: when splitting would exceed the budget the deepest splits are undone together with the splits that forced them. Because trimming removes whole sibling groups from the deepest level upward, a 2:1 balanced partition stays balanced.
- **Draw cap.** `MAX_DRAWS_PER_FRAME = 1024` with `DRAW_MARGIN = 24`; `TerrainSystem::new` rejects `max_leaves` above `MAX_DRAWS_PER_FRAME - DRAW_MARGIN`. Overflow is an explicit error ("draw budget exceeded"), never a silent drop.
- **Tau escalation removed.** `tau` is a constant (2 px); budgets, not a moving threshold, bound the work.
- **Default max_level 16**, tied to the generator's finest wavelength (about 32 m): finer levels add no information. PatchId `MAX_LEVEL` stays 24. Hysteresis (`h = 0.2`) is unchanged.

## Alternatives Considered

- Keep inflated spheres plus tau escalation: rejected, it is the cause of the G1 stall and non-convergence.
- Nanite-style cluster DAG: deferred, far beyond the P0 scope.
- Dropping `max_level` instead of tightening bounds: that was the Phase 7 workaround (reduced level 13 in tests); the real defaults must work.

## Consequences

- Per-frame selection cost is bounded by `max_nodes_visited` and `max_leaves` regardless of altitude.
- Leaves not yet resident fall back to coarser resident ancestors (effective cut), so streaming still never blocks rendering.
- Release wall-clock numbers are machine dependent; tests assert counted work only (ADR-0008).
- Goldens and benchmarks are Windows-only until hosted CI runs (GitHub billing).
- Debug builds exceed the 16.7 ms budget in the lower bands (CPU mesh generation); flights should use `--release`.

## Evidence

Machine: Windows 11, Intel Arc B580 (Vulkan). Commands: `cargo test -p planetary-app --test descent --locked -- --nocapture` and `planetary_flight --bench-descent --frames 600` (headless, CPU only, GPU time excluded). Raw CSVs under `target/diag/` (not committed).

### Descent suite at TerrainConfig::default() (max_level 16, 32 cells per edge)

| Measurement | Value |
| --- | --- |
| Run | 720 frames, 8 s descent from orbit to 3 m, then 4 s hover |
| Max per frame | nodes_visited 646 (budget 8192), leaves 687 (budget 900), drawn 573, draw_items 260, culled_draws 313, held meshes 1638, pending 96, started 8, uploads 4 |
| Convergence | drawn == wanted at frame 610, 130 frames after the descent ends (bound 240) |
| Detail at look-at point | drawn leaf 10 m ahead of the camera has level 16 |
| Ascent (surface to orbit, 720 frames) | at orbit max wanted level 3 (bound 6), drawn == wanted (57 leaves), partition and 2:1 balance every frame |
| Crack-freedom | drawn partition with max jump <= 1 on every frame after warm-up |
| Detail budget 60 | 2732 evictions, buffer_count == 2 * held every frame |

### Headless bench, release (`--bench-descent --frames 600`, RESULT: PASS, exit 0)

| band | avg ms | p95 ms | max ms | max nodes | max wanted | max drawn | max held | max pending |
|---|---|---|---|---|---|---|---|---|
| >1000 km | 0.08 | 0.20 | 0.41 | 190 | 171 | 4 | 4 | 0 |
| 100-1000 km | 0.20 | 0.34 | 0.46 | 362 | 330 | 18 | 22 | 0 |
| 10-100 km | 0.36 | 0.49 | 0.57 | 538 | 522 | 39 | 54 | 0 |
| 1-10 km | 0.64 | 0.76 | 0.81 | 774 | 831 | 81 | 112 | 0 |
| 100 m-1 km | 0.72 | 0.87 | 0.96 | 690 | 777 | 120 | 176 | 4 |
| <100 m | 0.67 | 0.78 | 0.99 | 574 | 663 | 201 | 360 | 8 |

### Headless bench, debug (exit 0, RESULT: FAIL against 16.7 ms: expected, the budget targets release)

| band | avg ms | p95 ms | max ms | max nodes | max wanted | max drawn | max held | max pending |
|---|---|---|---|---|---|---|---|---|
| >1000 km | 2.50 | 7.19 | 7.45 | 190 | 171 | 24 | 32 | 0 |
| 100-1000 km | 7.83 | 12.56 | 13.05 | 354 | 324 | 114 | 161 | 4 |
| 10-100 km | 15.80 | 19.43 | 23.21 | 494 | 471 | 228 | 417 | 36 |
| 1-10 km | 23.65 | 34.67 | 35.83 | 638 | 627 | 267 | 712 | 72 |
| 100 m-1 km | 33.80 | 36.10 | 36.54 | 646 | 687 | 243 | 1016 | 60 |
| <100 m | 35.61 | 62.59 | 79.55 | 554 | 636 | 519 | 1573 | 48 |

Baseline (Phase 7 code): release 312-722 ms average in the three lowest bands, 356 of 600 frames over 100 ms; debug 4-10 s per frame, did not finish in 900 s. Now zero frames over 100 ms in both builds.

## Amendment (07.1-19): streaming throughput

Measured at 30 fps with free-running workers (`planetary_flight --bench-stream`): the fixed caps (4 uploads, 8 starts, 16 requests per frame) supplied about 80 patches/s, detail took 198-224 frames to appear at the camera and 60% of frames were more than one level coarse. Decisions:

- **Adaptive per-frame caps.** `StreamBudget` scales the base caps between a quarter and the hard maxima `BudgetLimits` (32 uploads, 64 starts, 128 requests) from the measured prepare plus gpu_frame cost of the previous frame against `target_ms` (8). The terrain reads no clock: the app feeds the number (`TerrainSystem::set_frame_cost`); an absent, negative or non-finite value gives the base caps. Off unless `TerrainConfig::adaptive` (the app turns it on with `AppSettings::adaptive_streaming`).
- **Mechanism.** The stream manager config and the upload byte cap are fixed at construction, so adaptive mode builds them at the hard maxima and throttles per frame: the number of requests issued (queue at most two frames of starts, in flight at most three frames of uploads) and an uploader wrapper that refuses uploads beyond the cap (refused uploads stay CPU-ready and retry).
- **Order.** Requests are issued roots first (coverage), then by screen-space error over distance; an ancestor inherits the best score of the leaves it serves. Ancestors-first is unchanged.
- **Fallback chain pinned by use.** The wanted closure (pieces and ancestors) is touched in the residency LRU every frame, otherwise the ancestors of a drawn leaf are the first eviction victims and the camera falls back to the base levels.
