# ADR-0010: Terrain LOD - Skirts, Shared-Border Sampling and 2:1 Balance

- Status: proposed
- Date: 2026-11-25
- Requirement/Phase: PLAN-03, PLAN-04, PLAN-05, WS-05 / Phase 7

## Context

Cube-sphere terrain is drawn as a quadtree of fixed-size patches at mixed levels. Where patches of different levels meet, T-junctions open cracks. The user locked the crack-free technique in the phase context (CONTEXT.md); this ADR records it as a checkable decision and stays proposed until the seam and descent evidence lands.

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
- The effective cut costs an extra pass per frame over the leaf set.

## Evidence

| Measurement | Value | Source test |
| --- | --- | --- |

Pending: filled by plans 07-05 (seam proofs, measured eps) and 07-10 (scripted descent).
