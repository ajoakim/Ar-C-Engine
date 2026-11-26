# ADR-0011: Hex-Grid Planet Topology (Aperture-4 Half-Cell Hierarchy)

- Status: proposed
- Date: 2026-11-26
- Requirement/Phase: HEX-01, HEX-02, HEX-03 / Phase 7.1

## Context

The cube-sphere quadtree is the only planet topology. Phase 7.1 wants hex cells as the visible LOD unit: a hex gives way to a centre hex plus six edge hexes (each half covered by this parent) on descent, and aggregates on ascent. Real H3 (aperture 7) does not nest (about 7.3 percent parent area gaps) and needs trigonometry, which ADR-0008 forbids in the deterministic path. The design was proven only in the plane, so spikes S1-S4 tested it on the sphere before the full build. The user answered GO at the go/no-go checkpoint (plan 07.1-07). This ADR stays proposed until plan 07.1-17 accepts it.

## Decision

- Icosahedral aperture-4, centre-aligned hex hierarchy: `10*4^L + 2` cells per level, of which exactly 12 are pentagons (the icosahedron vertices). Layout is 10 diamonds plus 2 poles; diamond origins sit at obtuse corners.
- Edge children have two parents (half-cell edge children); the primary parent is the lower id. Pentagons are never halved.
- `CellId` is a u64: base 63..60, level 59..55, Morton(i,j) 54..7. Capacity is 24 levels.
- Projection is gnomonic and exact-op: `normalize(a*VA + b*VB + c*VC)`, terms summed in global vertex-index order, zero-weight terms skipped, only `+ - * / sqrt`. The canonical unrotated `(0, +-1, +-phi)` icosahedron is kept (a rotation would not be exact-op).
- `locate` is the nearest centre in the gnomonic face-lattice metric. Cell selection, bounds and culling code must use this lattice definition consistently, not the spherical Voronoi.
- Balance rule (revised 2026-11-26, user decision): a cell may split only if all its parents are split (parents-only closure). The earlier full rule also required the neighbours of the parents to be split; that half is dropped. Coarse half pieces may therefore face finer cells; the skirts cover that junction (see Balance rule evidence).
- Pentagons need separate per-edge neighbour enumeration (5 neighbours); the diamond-plane wrap is valid only for non-vertex cells.
- Skirts and shared-border sampling are kept (ADR-0010). A coarse hex edge equals the fine edge-child diagonal (N+1 versus 2N+1 samples).
- A `Topology` trait is added; the cube-sphere stays as test oracle and fallback until hex passes the full descent at real parameters.
- Defaults: max_level 16, N = 16 cells per sector side, ID capacity 24.
- FALLBACK (not chosen): nested triangles over the same lattice, hexes as IDs and overlay.

## Alternatives Considered

- Real H3 (aperture 7): children do not nest (about 7.3 percent gaps) and the projection needs trigonometry.
- ISEA3H: needs a rotated icosahedron and trigonometry, so it breaks the exact-op rule.
- Nanite-style cluster DAG: offline build step and GPU-driven pipeline, out of scope for this phase.
- Design C (triangles over the same lattice): lower risk but the LOD unit is a triangle; kept as the documented fallback.

## Consequences

- About 11 further plans (07.1-08..18) build IDs, adjacency, projection, the Topology trait, selection and balance, meshes with seam and coverage proofs, index-range draw and the topology-generic TerrainSystem.
- Caveat: the S4 skirt model is loose (`eps(L) = 0.25 * R * a_L` passes with a 27x to 1000x margin). It was validated only to coarse level 4 on the real hierarchy and with an interior construction to level 19. It does not validate the 1.5 margin against the engine's real LodModel error measure.
- Caveat: S3 and the balance finding are empirical on random sets, not a proof.
- Caveat: gnomonic Voronoi differs from spherical Voronoi (1.18 percent of 10,000 samples, worst cosine gap 3.6e-5), so code must stay on the lattice definition.
- Caveat: the S2 negative control is weak (addition of two terms is commutative), so skip-zero-weight is cheap insurance, not a discriminating test.
- Forced-split cost of the full closure is 4.58 per random pick versus 0.97 for parents-only (re-measured in 07.1-16b: 4.47 versus 0.93).
- Caveat (balance): max level jump 1 under the parents-only closure is empirical, on 40 random sets plus 12 pentagon distance-rule sets and 30 real selection cuts, together with the seam proof. It is not a general proof. If a jump of 2 is ever observed the rule must be revisited, not the assertion weakened.

## Evidence

Spike harness `extensions/planetary/planetary-core/tests/hex_spike/` (17 tests, 0 ignored, re-run 2026-11-26: 17 passed). Details in 07.1-04-SUMMARY.md and 07.1-06-SUMMARY.md.

| Spike | Result | Numbers |
|-------|--------|---------|
| S1 lattice and IDs | PASS | cells 12, 42, 162, 642, 2562, 10242, 40962 at L0-6; exactly 12 pentagons; reciprocal neighbours; 54,624 ids round trip |
| S2 exact-op projection | PASS | 0 edge and 0 vertex mismatches L0-6; hex-only area ratio 1.565 / 1.878 / 1.935 at L3/5/6; locate matches brute force 0 of 10,000 |
| S3 coverage and balance | PASS | 2,150,400 atoms, 0 uncovered, 0 multi-covered; max level jump 1; pentagons never halved; no-closure control violates 40/40 sets |
| S4 skirt gap | PASS | max gap ratio 0.0369 (L0) down to 0.0050 (L4); deep L3-19 max 0.0068; skirt-depth-0 control detected 5,040 of 5,040 |
| Wave A descent (cube-sphere, real defaults) | PASS | converged 130 frames after descent, level 16 at the look-at point; release p95 0.20 to 0.87 ms per band (07.1-05-SUMMARY.md) |

## Balance rule: parents-only (decision record, plan 07.1-16b)

Decision (user, 2026-11-26, explicit): switch the hex balance closure from parents plus neighbours-of-parents to parents-only. Reason: under the full closure the 900-piece budget stopped the descent at level 12 at a generic point (about 80 pieces per level); parents-only needs about 30 per level.

Evidence after the change (`pieces::required_splits`/`close`; cube-sphere untouched, 131-digest golden 0xee2da36546c6e081 still passes):

| Check | Result |
|-------|--------|
| Coverage, 40 random sets plus 12 pentagon sets | 2,519,040 atoms, 0 uncovered, 0 multi-covered, max jump 1, 0 level-diff-2 adjacencies (42,368 level-diff-1) |
| Negative controls | no closure: violated in 40/40 sets; dropped half and duplicated parent detected 40/40; synthetic L+2 control sees jump 2 |
| Real selection cuts, 30 views at 1 km | 1,591,824 atoms, 0 violations, max jump 1, 13,344 level-diff-1 pairs; dropped half control: 1,152 violations in 12 views |
| Seam proof (N=8, 60 views) | 6,617 junctions, 89 with a coarse half piece facing finer cells, 0 skirt violations, max gap/skirt 0.0369 (whole) and 0.0165 (half), 0 even-vertex mismatches |
| Real defaults (16 / 8192 / 900), 3 m | level 16 reached at a generic point, a pentagon and an icosahedron-edge midpoint; 1,347 to 1,396 nodes, 900 pieces |
| Superseded full closure, same sets | also 0 violations and max jump 1, but 4.47 forced splits per pick versus 0.93 and 16,017 versus 7,850 pieces |

Caveat: jump 1 is empirical on random sets and real cuts plus the seam proof; it is not proven in general.
