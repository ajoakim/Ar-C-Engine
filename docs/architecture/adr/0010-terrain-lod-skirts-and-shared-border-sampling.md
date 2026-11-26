# ADR-0010: Terrain LOD - Skirts, Shared-Border Sampling and 2:1 Balance

- Status: accepted
- Date: 2026-11-25
- Requirement/Phase: PLAN-03, PLAN-04, PLAN-05, WS-05 / Phase 7

## Context

Cube-sphere terrain is drawn as a quadtree of fixed-size patches at mixed levels. Where patches of different levels meet, T-junctions open cracks. The user locked the crack-free technique in the phase context (CONTEXT.md); this ADR records it as a checkable decision and is accepted with the seam and descent evidence below.

## Decision

- Crack-free transitions use **skirts + shared-border sampling**.
- Shared-border sampling: border vertices are computed from one planet-space function over a global integer lattice (dyadic `s`, exact-op equal-angle `T(s)` with the `s == +-1` special case, world-xyz-order normalisation, level-independent height), so same-level borders are bit-identical in f64.
- Skirts: a downward skirt of depth `SKIRT_MARGIN * eps(L-1)` (initial margin 1.5) hides the T-junction gaps between levels.
- Restricted quadtree **2:1** balance (cross-face) bounds the neighbor level difference to 1.
- The drawn set is a second **effective cut**: split only where all four children are GPU-resident, then coarsen to a balanced fixpoint. Streaming fallback would otherwise break 2:1.
- SSE LOD: `rho = eps(L) * proj_scale / d`; split when `rho > tau`; stay split while `rho > tau*(1-h)`. Initial `tau = 2 px`, `h = 0.2`, `kappa = 0.25`. Leaf cap 700 with deterministic `tau *= 1.5` escalation. `N = 32` cells per patch, demo max level 19, PatchId `MAX_LEVEL = 24`.

## Alternatives Considered

- Geomorphing: deferred (CONTEXT deferred ideas).
- Stitching index variants: needs more index buffers; rejected for simplicity.
- libm `tan`: banned by ADR-0008 (determinism).

## Consequences

- Limitations: f32 per-patch origin quantisation (Pitfall 4), cross-level normals differ slightly, no frustum or horizon culling in Phase 7 (the leaf cap is the guard instead), cross-OS goldens pending hosted CI.
- Cross-OS goldens (GOLDEN_HEIGHTS, GOLDEN_DIRS, GOLDEN_MESH) are Windows-computed; Linux/macOS confirmation is pending hosted CI (same status as ADR-0008). The manual window run result is recorded in the 07-10 SUMMARY.
- Relief-inflated node bounds (`max_relief` about 6 km) make every patch within that distance of the camera split to `max_level`; with 700 leaves the usable `max_level` is bounded (13 for the descent test, where cells are about 75 m at the leaf level). `tau` escalation cannot reduce this count.
- The effective cut costs an extra pass per frame over the leaf set.

## Evidence

| Measurement | Value | Source test |
| --- | --- | --- |
| Same-level border vertex pairs (levels 0..=4, n = 8) | 73656 checked, all bit-identical in f64 | `same_level_borders_bit_identical` in `extensions/planetary/planetary-core/tests/seam.rs` |
| Cube edges / corners (level 2) | 12/12 edges and 8/8 corners bit-identical | `cube_edges_and_corners_explicit` in `extensions/planetary/planetary-core/tests/seam.rs` |
| Mixed-level L/L+1 junctions (30 reference views, max level 14, n = 8) | 4272 junctions, 34176 odd fine vertices; even vertices equal coarse bit-exactly; max gap / min skirt depth = 0.035 (<= 1) | `mixed_level_gap_bounded_by_skirts` in `extensions/planetary/planetary-core/tests/seam.rs` |
| Negative control (skirt depth forced to 0) | 34176 of 34176 odd vertices reported as violations | `negative_control_without_skirts_fails` in `extensions/planetary/planetary-core/tests/seam.rs` |
| f32 origin + offset reconstruction (levels 0, 5, 10, 15, 19) | max error = 0.31 of the bound 2 * extent * 2^-23, bound < skirt depth | `f32_reconstruction_within_tolerance` in `extensions/planetary/planetary-core/tests/seam.rs` |
| Measured max deviation / cell size (n = 32) | level 2: 0.00253, 6: 0.00115, 10: 0.00140, 14: 0.00058, 18: 0.000027, 19: 0.000017; all <= eps(L) | `measured_error_below_eps` in `extensions/planetary/planetary-core/tests/seam.rs` |
| Final kappa | 0.25, kept (measured ratio would allow 1/32; not tuned so downstream plans keep one default) | `measured_error_below_eps` in `extensions/planetary/planetary-core/tests/seam.rs` |

Golden hashes (mesh, generator) are Windows-computed; cross-platform CI confirmation is pending.

### Scripted descent (07-10)

Source: `extensions/planetary/planetary-app/tests/descent.rs`, simulated clock dt = 1/60, no wall-clock, N = 16 cells, workers 2, settle each frame.

| Measurement | Value |
| --- | --- |
| Run | 600 frames (10 simulated seconds: 8 s descent from orbit to 3 m above the surface, then hover); convergence run 720 frames |
| Max per frame | started 8, uploads 4, uploaded_bytes 76656, nodes_visited 13618, drawn 612, held meshes 1820, tau_eff 10.125 |
| Crack-freedom | drawn set is a partition and 2:1 balanced on every frame after warm-up (10 frames) |
| Convergence | drawn == wanted at frame 590, 110 frames after the descent ends (bound 240) |
| Determinism | two runs give identical per-frame started, completions_applied, uploads and drawn sets |
| Degradation | max_starts = 1 and max_completions = 1: at most 1 start per frame, still a balanced partition, never empty after warm-up |
| Leaks | buffer_count == 2 * held every frame; detail budget 60 forced 1473 evictions with the invariant intact |
| Production path | no settle: 600 frames complete, caps and max jump <= 1 hold, partition and balance checked once all 6 faces are drawn |

Final tunables: tau = 2 px (escalates up to 4 steps of x1.5 during the descent), h = 0.2, kappa = 0.25, skirt margin 1.5, max_leaves 700, N = 32 for the demo and 16 for the tests, test max level 13, demo default max level 19 (PatchId MAX_LEVEL 24), UPLOADS_PER_FRAME 4, REQUESTS_PER_FRAME 16, max_starts 8, max_completions 8, CONVERGE_WITHIN 240.
