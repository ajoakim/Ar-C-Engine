# Hexagonal Planet Topology - Research (with Nanite assessment)

**Researched:** 2026-11-25
**Domain:** Hierarchical hexagonal planet topology for streamed, crack-free terrain LOD (planetary extension)
**Confidence:** MEDIUM overall. The planar geometry of the recommended scheme was checked numerically. Sphere-level claims need the spikes listed below.
**Scope note:** The user has decided to move the planet to a hex grid. This document finds the best hex design and says where the real risks are. It does not argue for staying on the cube-sphere. A cost comparison with fixing the quadtree is included because the brief asked for one.

---

## 1. Executive recommendation

**Build design B\*: an icosahedral, aperture-4, centre-aligned hex/pentagon hierarchy, with "half-cell" edge children. Project it with a trig-free gnomonic projection on the icosahedron faces. Keep skirts and shared-border sampling (ADR-0010) for crack-freedom, and take the selection, bounds and streaming lessons from Nanite. Do not use real H3 (aperture 7) as render geometry.**

What this means:

- **Cells.** Level `L` cells are the hexagons around the vertices of an icosahedron whose faces are subdivided 2^L times. At each level there are `10*4^L + 2` cells, 12 of them pentagons, which sit on the 12 icosahedron vertices.
- **Children.** Each parent hex gives way to its constituent parts: one centre child, which is a full hex, plus six edge children. Each edge child is split exactly in half by the parent's border. The neighbouring parent owns the other half and completes it when that parent splits too.
  - Checked numerically in the plane: this tiles the parent exactly (0 coverage violations in 90,000 random tests). It is the only straight-edged hex hierarchy found that nests without holes.
  - By contrast, about 7.3% of an aperture-7 (H3) parent's area is covered by its *neighbours'* children. Those regions become holes whenever a parent splits and its neighbour does not.
- **Projection.** A cell or sample point is `normalize(i*A + j*B + k*C)` over integer barycentric lattice coordinates on an icosahedron face. That uses only `+ - * / sqrt`, which ADR-0008 allows, and needs no `tan` Taylor trick. The measured distortion is a max/min hex area ratio of about 1.96 at level 7, which is about the same as H3's 1.99.
- **Patches and seams.** Each cell's patch is 6 triangular sectors (5 for a pentagon), each a triangular grid of side `N`. Drawing half an edge child means drawing 3 sectors. Shared borders sit on one global integer lattice, so the ADR-0010 seam proof carries over: even vertices are bit-identical, odd-vertex gaps are no larger than the skirt depth, and a negative control is included.
- **Balance.** The 2:1 balance analogue is: "a cell may split only if all its parents are split, and the neighbours of those parents are split". It bounds adjacent drawn pieces to at most one level apart (simulated: max jump 1 with the rule, 2 without it).
- **Selection fixes.** These are topology-independent and matter more than the topology switch for gap G1:
  - Distance is measured to the cell footprint, extruded radially between per-cell `[hmin, hmax]`, instead of a sphere inflated by global relief.
  - Height ranges come from the nearest resident ancestor's measured min/max, plus an analytic residual-octave bound.
  - Exact-op horizon culling and frustum culling.
  - A node-visit budget.

### Decisions the USER must confirm

1. **Hierarchy choice.** Use aperture-4 centre-aligned hexes on an icosahedron (B\*), not real H3 aperture 7. Exact H3 cell IDs and compatibility are not a requirement. An H3 export or index adapter can be added later as a geospatial index (HLD §7.2 already separates "H3 or similar geospatial indexing" from topology).
2. **Half-cell children.** A hex gives way to 1 full child plus 6 half children, and the neighbour completes each half when it splits. This makes the hierarchy a DAG: edge children have two parents. Streaming uses a deterministic *primary* parent, the lower id.
3. **Projection.** Use the gnomonic projection: no trig, area ratio about 2, the same as H3. An equal-area projection (ISEA/Snyder) needs trig and a superseding ADR to ADR-0008.
4. **Crack-free technique.** Keep skirts plus shared-border sampling: the ADR-0010 technique, applied to hex sectors and half-cell diagonals. Defer Nanite-style locked borders (watertight without skirts) as a possible later upgrade.
5. **`Topology` trait.** Add it now, with a small surface (section 4.3), and keep the cube-sphere as the second implementation and the oracle until the hex path passes its descent at real parameters. Delete or keep the cube-sphere in Phase 8. Alternative: replace the cube-sphere outright with no trait.
6. **One generic core change.** Add shared static index buffers plus index-range draws to `engine-render`/`engine-sdk`. Half-cell drawing needs it, and it also halves buffer counts on the quadtree. The fallback is two half-meshes per edge child, which costs up to about 1.9x more draws.
7. **Defaults.** Sector side `N = 16`. The `MAX_LEVEL` id capacity is 24. The **default max level is tied to the generator's finest feature**: about level 16 for today's TerrainParams, whose finest noise wavelength is about 32 m. Reaching about 1 m resolution needs level 18 *and* more detail octaves (see 3.4).
8. **Nanite.** Borrow its cut rule, monotone/tight bounds, always-resident root levels and hierarchical culling. Do not build a cluster DAG, visibility buffer or GPU-driven pipeline now: compute and GPU-driven rendering are out of scope for P0.
9. **Sequencing.** Do the telemetry and crash diagnosis (gap G2/G3) and the selection fixes (G1) first, in parallel with the hex spikes. They carry over unchanged.

---

## 2. Comparison of candidates

| | A: Real H3 (aperture 7) | B\*: Aperture-4 centred hex + half-cell children (RECOMMENDED) | B': ISEA3H (aperture 3) | C: Hex as index layer over a nested icosahedral triangle quadtree | D: Nanite-style cluster DAG as the terrain topology | E: Hybrid (B\* + Nanite-borrowed selection/bounds/streaming) |
|---|---|---|---|---|---|---|
| Nesting | Approximate. About 7.3% of a parent is covered by neighbours' children (measured, planar) | Exact as a DAG: parent = centre + 6 half children (measured, planar) | Not congruent. Grid rotates 30° per level | Triangles nest exactly. Hex cells are only labels | Exact per group (locked group borders) | As B\* |
| Hole-free mixed cuts | No. Needs overdraw, stencil or flange hacks (unproven) | Yes, with the "split only if all parents split" rule | Not with straight halves (rotation) | Yes (the quadtree argument) | Yes, by construction | Yes |
| What the user sees during descent | Hexes (rotating per level) | Hexes, and a hex gives way to its parts | Hexes | Triangles give way to 4 triangles. The hex is only a debug overlay | Irregular clusters | Hexes |
| Projection math | Gnomonic on icosahedron faces, with trig (h3o depends on `libm`) | Gnomonic barycentric, `+ - * / sqrt` only | ISEA (Snyder) needs trig | Gnomonic, exact-op | n/a (mesh-based) | Exact-op |
| ID | 64-bit: mode, res (4 bits), base cell (7 bits, 122 cells), 15 three-bit digits [ASSUMED layout] | 64-bit: base (4 bits: 10 diamonds + 2 poles), level (5 bits), Morton(i,j) (≤48 bits) | Sahr Q2DI or CPI codes | Like the current PatchId: 20 faces, Morton | Cluster/page indices (no geographic id) | As B\* |
| Pentagons | 12 per resolution; complicated neighbour and rotation rules | 12 per level, always centre children of the 12 root pentagons; 5 sectors; never drawn as halves | 12 | Hidden (triangle geometry) | n/a | As B\* |
| Area ratio (max/min hex) | about 1.99 at res 14-15 [CITED h3geo] | about 1.87-1.96 at levels 5-7 [VERIFIED: script] | Equal-area | Triangle ratio about 1.98 [VERIFIED] | n/a | As B\* |
| Crate or dependencies | h3o 0.11.1, BSD-3-Clause (allowed); deps include libm, float_eq, ordered-float, either, h3o-bit | In-house (~1.5-2.5k LOC) | DGGRID is C++ | In-house | meshoptimizer bindings or METIS-like partitioning; nothing fits ADR-0008 | In-house |
| ADR-0008 fit | Poor: trig in cell geometry, needs a superseding ADR | Good | Poor | Good | Poor: float QEM simplifier and graph partitioning | Good |
| Complexity / risk | High, with an unproven seam technique | Medium-high; seam proof analogous to ADR-0010 | High | Medium (closest to current code) | Very high; offline build, needs GPU-driven rendering | Medium-high |
| Verdict | Reject as geometry; possible later index adapter | **Build** | Reject | Fallback if the B\* spikes fail | Reject as topology; borrow ideas | **= the recommendation** (B\* geometry + Nanite lessons) |

---

## 3. Detailed findings

### 3.1 Q1: Candidate designs

#### Verified facts about H3

| Fact | Value | Source / confidence |
|---|---|---|
| Aperture | 7. Each cell has 7 children; unit length scales by √7 and area by 1/7 | h3geo overview [CITED, HIGH] |
| Containment | "Geographic containment is approximate"; logical containment in the index is exact; the grid alternates orientation between levels | h3geo highlights/indexing [CITED, HIGH] |
| Projection | Grid on the faces of an icosahedron, then "inverse face-centered polyhedral gnomonic projection" | h3geo overview [CITED, HIGH] |
| Base cells | 122 at res 0 (110 hex + 12 pentagons) | h3geo overview [CITED, HIGH] |
| Pentagons | Exactly 12 at each resolution, centred on the icosahedron vertices | h3geo restable [CITED, HIGH] |
| Cell counts | `2 + 120*7^r` (res 0: 122, res 15: 569,707,381,193,162) | h3geo restable [CITED, HIGH] |
| Edge length | res 15 about 0.584 m, res 14 about 1.55 m, res 10 about 75.9 m | h3geo restable [CITED, HIGH] |
| Hex area ratio within a res | about 1.212 (res 0) up to about 1.993 (res 14/15) | h3geo restable [CITED, HIGH] |
| Index | 64 bits; modes 1 cell, 2 directed edge, 4 vertex | h3geo h3Indexing [CITED, HIGH]; field widths (4 res, 7 base cell, 15×3 digits) [ASSUMED] |
| Class II/III alternation, rotation of about 19.1° per resolution | Training knowledge | [ASSUMED, MEDIUM] |
| h3o crate | 0.11.1, BSD-3-Clause, updated 2026-11-02, edition 2024; non-optional deps `either`, `float_eq`, `h3o-bit`, `libm`, `ordered-float` | crates.io API + docs.rs Cargo.toml [VERIFIED] |

**Hexes cannot nest exactly.** "A triangular grid would be congruent whereas a hexagonal grid would not". The NWP-SAF report and DGGRID material agree, and OpenEAGGR offers a congruent ISEA4T but a non-congruent ISEA3H [CITED, MEDIUM-HIGH]. A hexagon cannot be tiled by smaller hexagons. Every hex hierarchy therefore has to choose one of:
- non-nested hexes plus overlap handling (A, B'),
- fractal or staircase "aggregate" cells (Gosper/GBT style: exact, but the boundaries are jagged at every scale, and mixed-level boundaries produce *horizontal* slivers that skirts cannot hide),
- a nested non-hex geometry with a hex labelling (C),
- or the B\* construction below, which uses straight-edged hex cells but makes the *drawn pieces* (whole cells and half cells) tile exactly.

#### Design A: real H3 (aperture 7)

- **ID and hierarchy:** use h3o directly (`parent`, `children`, `grid_disk`). Children are rotated against the parent (Class II/III), so the parent's border passes *through* child hexes at irregular angles.
- **Prototype result:** about 7.3% of a parent's area is not covered by its own 7 children [VERIFIED: planar Eisenstein-lattice script, 200k samples, aperture-7 sublattice `(3+ω)Z[ω]`]. With a mixed cut (P split, neighbour Q not), those slivers are holes.
  - The possible fixes are all unproven for terrain: draw a lowered "flange" of Q under P's children; draw P under its children with a depth bias; or use stencil "finest wins", which is wrong at silhouettes because stencil is screen-space.
- **Trig:** h3o computes geometry with `libm`. libm is pure Rust and probably platform-consistent [ASSUMED], but ADR-0008 bans `sin/cos/atan2` regardless, so this needs a superseding ADR plus quantisation before hashing.
- **Verdict:** reject as render geometry. Keep as a *possible later index adapter*: map our cells to and from H3 ids for geospatial data.

#### Design B\*: aperture-4 centre-aligned hex, half-cell children (recommended)

**Lattice.** Take the icosahedron (12 vertices, 20 faces) and subdivide every face with frequency `2^L` (Class I geodesic, dyadic).
- The level-`L` cells are the lattice vertices: `10*4^L + 2` of them, matching H3-like counts at aperture 4 (level 0 has 12 pentagons, level 1 has 42 cells, level 7 has 163,842) [VERIFIED: script counted 642, 10,242 and 163,842 vertices at L = 3, 5, 7].
- The cell's region is its hex (or pentagon) Voronoi region *in the face lattice*. Its corners are the centroids of the incident lattice triangles.
- Cell centres nest: centres at level `L` are a subset of the centres at `L+1`, because the lattice is dyadic.

**Children.**
- The centre child sits at `(2i, 2j)`.
- The 6 edge children sit at the 6 lattice-edge midpoints `(2i±1, 2j)`, `(2i, 2j±1)` and the two diagonal ones.
- An edge child has **two parents**: the two ends of the lattice edge it bisects.

Verified in the plane, over 200,000 random points and levels 0-5:
1. The nearest level-`L+1` cell of any point is always a child of that point's nearest level-`L` cell. So the parent is contained in the union of its children.
2. The border between the two parents of an edge child runs exactly along that child's corner-centre-corner diagonal, so each half is exactly 3 of its 6 sectors. 0 mismatches.
3. Rule: *a cell may be split only if all of its parents are split*, and a drawn cell draws only the sectors owned by split parents. Under that rule, every point of the plane is drawn exactly once: 0 of 90,000 tests had coverage other than 1, over 30 random closed split sets.

**IDs and packing.** Use the DGGRID Q2DI diamond layout: 10 diamonds (pairs of faces sharing an edge) plus 2 pole cells. That gives exactly `10*4^L + 2` cells. Each diamond owns lattice vertices `0 <= i, j < 2^L`.

`CellId(u64)` = base (bits 63..60: 0 = north pole, 1..10 = diamond, 11 = south pole) | level (59..55) | Morton(i, j) (54..7, up to 48 bits for L ≤ 24) | zero low bits. That is 57 used bits, validated like the current `PatchId` (stray bits, range).
- The pentagons are the 10 diamond origins `(0, 0)` plus the 2 poles. A unit test should assert exactly 12 cells with 5 neighbours at each level. The DGGRID layout claim is [ASSUMED, MEDIUM], so check it in Spike S1.

**Derivation, all integer math:**
- `children`: as above.
- `parents`: if both coordinates are even, one parent `(i/2, j/2)`. Otherwise two parents, the lattice-edge endpoints. The coordinate `2^L` wraps into a neighbouring diamond through a static table.
- `primary_parent`: the lower raw id, used by `StreamKey::parent()`.
- `neighbors`: the 6 lattice directions, with cross-diamond wrap and rotation tables (10 diamonds: 5 northern, 5 southern). This is the same kind of work as the current cube-face adjacency table, with the same reciprocity tests.

**Pentagons.** These are only the 12 icosahedron vertices.
- They are always centre children of pentagons, so a pentagon is never an edge child and is never drawn as a half.
- The mesh is 5 sectors.
- The neighbour count is 5.
- Measured pentagon area is about half the mean hex area at deep levels. The cell area ratio including pentagons is about 2.37 at L = 7; for hexes only it is about 1.96.

**Projection (exact-op).** Each sample point is an integer barycentric coordinate `(face, a, b, c)` with `a + b + c = D`, where `D = 3*N*2^L` (the factor 3 makes the centroids, which are hex corners, lattice points).
- Direction: `p = a*V_A + b*V_B + c*V_C`, summed in *global vertex-index order*, then `p / sqrt(p·p)`.
- Points on an icosahedron edge have one zero weight. Both faces then compute the same sum of the same operands in the same order, so they are bit-identical. This is the same invariant as `lattice_dir` today.
- The vertex constants use φ = (1+√5)/2, from literals or `sqrt`.
- `locate(dir)`: pick the face by the largest dot product with the 20 face normals; get gnomonic barycentrics by dividing by the dot product; round to the nearest hex with cube-coordinate rounding (`floor`). No `atan`.
- **Distortion** [VERIFIED: script]:

  | Level | Hex area max/min | Lattice edge max/min |
  |---|---|---|
  | 3 | 1.52 | 1.38 |
  | 5 | 1.87 | 1.45 |
  | 7 | 1.96 | 1.47 |

  For comparison, H3 (also gnomonic) is 1.99. An exact-op polynomial warp could reduce this later; it is not required.

**Complexity.** Medium-high, but the code has the same shape as today's: integer lattice, Morton ids, adjacency tables, a canonical edge sum. Rough estimate [ASSUMED]: 1.5-2.5k LOC plus tests.

#### Design B': ISEA3H / aperture 3

- The grid rotates 30° between successive resolutions [CITED: Sahr 2008 via search, MEDIUM].
- Child borders are therefore not along parent borders, so the half-cell trick fails and holes appear as in A.
- Equal-area ISEA needs trig.
- Reject.

#### Design C: hex index over a nested triangle quadtree

- Render patches are the 20 icosahedron faces, each split into 4 recursively (exact nesting, 3 edge neighbours, 2:1 balance). This is close to the current code.
- Hex cells (the duals of the vertices) are used for ids, aggregation and an overlay.
- It is fully proven technology, and it uses the same lattice and projection as B\*. The B\* spikes therefore produce C almost for free.
- **Downside:** the LOD unit the user sees is a triangle, not a hex.
- **Fallback if S3/S4 fail.**

### 3.2 Q2: Rendering model

**Patch mesh.**
- A hex patch has 6 sectors `(centre, corner_k, corner_{k+1})`; a pentagon has 5.
- Each sector is a triangular grid of side `N` on the level's sample lattice (spacing `R_hex/N`).
- Vertices per sector: `(N+1)(N+2)/2`. Duplicating sector-boundary vertices is fine because they are bit-identical. For `N = 16` that is 153 vertices per sector, 918 per hex, and 1,536 triangles.
- The sample lattice of level `L+1` contains the lattice of level `L`. Aperture 4 does not rotate, and every hex edge and diagonal is a lattice line. So on any border, the coarse samples are exactly the *even* fine samples. This is the same structure as the quadtree's dyadic T-junction.
  - This is a construction argument [HIGH in the plane]. It needs `N` even so that the edge midpoint is a sample.

**Index variants (static, shared by all hexes).**

| Variant | Contents |
|---|---|
| Full hex | 6 sectors + 6 outer-edge skirts |
| Full pentagon | 5 sectors + 5 outer-edge skirts |
| Half hex, one per side of each of the 3 diagonals | 3 sectors + 3 outer-edge skirts + 1 diagonal skirt |

That is 6 half variants and 8 index lists in total, shared across all patches.
- This needs a generic **index-range or shared-index-buffer draw** in `engine-render`/`engine-sdk`. Today `MeshHandle` owns one vertex and one index buffer and there is no range draw (checked: `crates/engine-render/src/scene.rs`, `backend.rs`). STACK.md already anticipates "Terrain patches should share a few static index buffers". wgpu `draw_indexed` supports index ranges and `base_vertex` [ASSUMED, standard wgpu API].
- The fallback is to upload two half-meshes per edge child.

**Mixed levels and balance.**
- Rule: if X is split, all of X's parents are split, and all hex neighbours of those parents are split.
- Simulated in the plane over 12 random sets: the maximum level jump across drawn-piece borders was 1 with the rule and 2 without it [VERIFIED: script]. This is the hex form of 2:1 balance. It is a closure that only inserts splits, so the existing `balance()` pattern ports directly.

**Pentagons.** No special crack case. A pentagon is a 5-sector fan, and its borders are lattice lines like any other. The special handling is limited to the id/neighbour tables, the mesh variant, and the fact that it never splits into halves.

**Seam proof analogue.** ADR-0010's technique carries over because:
- every vertex is `surface_point(lattice_dir(face, a, b, c, D))` from one continuous height function;
- same-level borders are bit-identical;
- L/L+1 borders have equal even vertices and odd vertices bounded by `eps(L)`.

Tests:
1. Same-level shared-border pairs are bit-identical, including the 30 icosahedron edges and the 12 vertices.
2. Mixed L/L+1 junctions across 30 reference views:
   - even fine vertices equal the coarse ones bit-exactly;
   - max odd-vertex gap / skirt depth ≤ 1;
   - **half-diagonal borders included**.
3. Negative control: skirt depth 0, so every odd vertex is reported.
4. **New: a coverage proof.** Rasterise the sphere's lattice triangles of level `max+1` and assert that each one is covered by exactly one drawn piece for random closed and balanced split sets.
5. The f32 origin reconstruction test, unchanged.

**Bounded gap.** At an L/L+1 border, the coarse edge between two consecutive even samples is a chord of the surface. The fine odd sample is the true surface at the midpoint. The vertical gap is therefore at most `max deviation of the level-L mesh from the true surface`. ADR-0010 measured this as ≤ `eps(L)` with `kappa = 0.25`. A skirt of depth `1.5*eps(L)` covers it. Same argument, same margin; the gap must be re-measured per level on the hex mesh.

**Nanite-style locked borders, the alternative to skirts.** With B\*, parent borders become the *interior diagonals* of edge children one level down. This is Nanite's "group boundaries change between levels" property, and it is why the hex hierarchy, unlike the quadtree, could in principle be made watertight *without* skirts (coarse borders built at fine resolution). It needs a case analysis of which border types meet; section 3.2 of the simulation showed that hex edges and half diagonals both occur at L/L+1 borders. Treat it as a later experiment (Spike S6), not the baseline.

### 3.3 Q3: LOD selection and streaming

**Why G1 happened** (from the code). `LodModel::node_bound` returns a sphere of radius `corner extent + max_relief + sagitta`, with `max_relief = base + detail amplitude = 3,800 m` (ADR-0010 quotes "about 6 km"). The SSE distance is `|eye - c| - r`, clamped to `min_distance = 1 m`. So every node within about 3.8 km *horizontally* of the camera has distance 1 m and splits to `max_level`. At level 19 (0.6 m cells), that is tens of thousands of leaves. Then:
- `tau` escalation reruns the full selection up to 8 times, which costs CPU and does not fix the leaf count (ADR-0010 itself says so);
- `max_leaves = 700` can be exceeded;
- `effective_cut` adds another pass.

This is independent of topology and would hit the hex grid the same way.

**Selection design, topology-agnostic, in exact-op math:**

1. **Distance to an extruded footprint, not an inflated sphere.**
   - Footprint per cell: unit centre `c`, angular radius stored as `(cos δ, sin δ)` per cell, computed from the max corner dot product. Height range `[hmin, hmax]`.
   - Eye: radius `r_e`, unit direction `e`.
   - Horizontal distance: `dh = max(0, chord(e, c) - R*2*sin(δ/2))` [ASSUMED approximation; conservative variants are easy].
   - Vertical distance: `dv = max(0, r_e - (R + hmax), (R + hmin) - r_e)`.
   - `d = sqrt(dh² + dv²)`.
   - The relief only extends the bound *vertically*, so far-away cells are no longer pulled in.
2. **Tight per-cell height ranges** (Nanite lesson: bounds come from actual geometry and are monotone).
   - The payload stores the exact min/max of the sample heights, computed at generation.
   - A non-resident cell inherits the union of its parents' resident ranges (edge children lie inside the union of their two parents, verified) plus `residual(L)`.
   - `residual(L)` is the amplitude of the noise octaves finer than the parent's sample spacing, plus a Lipschitz term for the rest. fbm uses amplitude ×0.5 and frequency ×2 per octave (checked in `engine-procedural/src/noise.rs`), so the residual is a geometric tail `2*A*0.5^K/norm` per layer [formula VERIFIED from code; tightness to be measured].
   - With no resident ancestor (only levels 0 to `base_levels`, which are always resident), use the global relief.
   - Monotonicity: a child's range must lie inside its parent's range plus residual, and its error must be no larger than the parent's. Assert both in tests, as Nanite does with `ParentLODError = max(...)`.
3. **Horizon culling, exact-op.**
   - Occluder sphere radius `r_o = R + global hmin` (conservative).
   - Cesium's point test: `cv = C/r_o`, `vh² = |cv|² - 1`, `vt = t - cv`, `vt·vc = -(vt·cv)`. The point is occluded iff `vt·vc > vh²` and `(vt·vc)²/|vt|² > vh²` [CITED: Cesium horizon culling blog, MEDIUM-HIGH].
   - Apply it to a per-cell occludee point (the highest corner, lifted to `R + hmax`), or use the angle-sum form with cosines and sines from `sqrt` only. No `acos`.
4. **Frustum culling.** Six f64 planes against the cell's bounding sphere, built from the *tight* range.
5. **Node-visit budget.** Traverse best-first by SSE (a `BinaryHeap` keyed by priority, then id, for determinism), with a hard cap of 8,192 visits by default [ASSUMED default]. When the cap is hit, the remaining nodes stay at their current level and are reported in telemetry. Remove the full-rerun `tau` escalation: it was the stall multiplier. Replace it with a single pass that raises `tau` for the remainder once the leaf or draw budget is reached.
6. **Cut rule.** Keep SSE with hysteresis: split when `rho > tau`, stay split while `rho > tau(1-h)`. Rewritten Nanite-style, a cell is drawn iff its own error is ≤ threshold and some owning parent's error is > threshold. Error is per cell and parents' errors are ≥ children's, so the decision is per-cell and parallelisable later.
7. **Effective cut.** Unchanged semantics: split only when all 7 children are resident (centre + 6 edge children; the edge children may already be resident through the other parent), then coarsen to a balanced fixpoint.
   - Pinned coarse-parent fallback via `StreamKey::parent()` = primary parent. Rendering does not depend on which parent is primary, because the drawn pieces come from the split set.
   - Per-frame caps (`max_starts`, `max_completions`, uploads) are unchanged.
   - `MAX_DRAWS_PER_FRAME = 1024`: today an overflow silently drops draws, which shows up as holes (`system.rs`, `draw_overflow`). Make the draw budget an input to selection (`max_leaves` ≤ 1024 minus a margin) instead of dropping draws after the fact.

**Sizes and counts** (`R = 6,371 km`, `N = 16`, `kappa = 0.25`, `tau = 2 px`, 1080p, 60° vertical field of view; split distance `d_L = kappa * s * proj / tau`) [VERIFIED: script]:

| L | Cells | Mean centre spacing | Hex circumradius | Sample spacing | Split distance d_L |
|---|---|---|---|---|---|
| 0 | 12 | 7,006 km | 4,045 km | 253 km | 29,556 km |
| 5 | 10,242 | 240 km | 138 km | 8.65 km | 1,012 km |
| 10 | 10,485,762 | 7.49 km | 4.33 km | 270 m | 31.6 km |
| 12 | 167,772,162 | 1.87 km | 1.08 km | 67.6 m | 7.9 km |
| 14 | 2,684,354,562 | 468 m | 270 m | 16.9 m | 1.98 km |
| 15 | 10,737,418,242 | 234 m | 135 m | 8.45 m | 988 m |
| 16 | 42,949,672,962 | 117 m | 67.6 m | 4.23 m | 494 m |
| 17 | 171,798,691,842 | 58.6 m | 33.8 m | 2.11 m | 247 m |
| 18 | 687,194,767,362 | 29.3 m | 16.9 m | 1.06 m | 124 m |
| 19 | 2,748,779,069,442 | 14.6 m | 8.5 m | 0.53 m | 62 m |
| 20 | 10,995,116,277,762 | 7.3 m | 4.2 m | 0.26 m | 31 m |

- **About 1 m mean sample spacing is reached at level 18 with `N = 16`**, or level 17 with `N = 32`. Gnomonic distortion makes the worst cells about 1.2-1.5x coarser in edge length.
- Expected drawn pieces per level [ASSUMED estimate, to be measured by telemetry]: `d_L / r_hex ≈ 7.3`, so about 64 cells in the split disc, of which about 48 are drawn per active level without culling. Frustum culling cuts that to about 12-20.
  - Near the surface (2 m altitude, horizon about 5 km), levels 12-18 are active: about 120-350 draws.
  - The peak is at about 100 km altitude (horizon about 1,130 km, levels about 5-10): about 300.
  - Both fit within 1,024 draws, even counting half pieces.
  - Nodes visited ≈ 7 × drawn, about 1-3k, which is well within an 8k budget.
- **Honest finding about "1 m".** Today's generator has `detail_frequency = 24` and 14 octaves, so its finest value-noise wavelength is about `R/(24*2^13) ≈ 32 m`. Below a sample spacing of about 4-8 m (levels 15-16), extra levels only interpolate smooth noise. **Recommended default `max_level = 16`**. Expose it as a function of the generator: choose the max level where the sample spacing is about the finest wavelength / 8. For real 1 m detail, raise `detail_octaves` to about 19 (ADR-0008's `freq*2^oct < 2^40` limit allows it) and use `max_level = 18`.
- Grid resolution: `N = 16` per sector side (918 vertices, 1,536 triangles plus skirts), which is comparable to today's `N = 32` quad patch (1,089 vertices).

### 3.4 Q4: Fit with the architecture

**Survives unchanged or nearly unchanged:**
- `engine-streaming` (StreamKey, StreamManager, pins, resolve, caps)
- `engine-procedural` and the goldens policy
- `engine-math`
- SDK `gpu_frame` / `destroy_mesh`
- `planetary-terrain`: camera, uploader, palette and the request/reconcile flow (made generic over the cell id)
- `planetary-app`: descent script and settings
- `planetary-core`: `terrain.rs` (heights depend only on direction), payload encoding (plus min/max height fields)
- the banned-ops test
- the planetary-absent CI canary (everything stays under `extensions/planetary`)

**Replaced or added in `planetary-core`:**
- `patch_id` → `cell_id`
- `face`/`adjacency` → icosahedron, diamond and neighbour tables
- `mapping` → barycentric exact-op mapping and locate
- `lod/model` → footprint/range bounds and culling
- `lod/select` → budgeted best-first selection
- `lod/balance` → hex closure
- `mesh` → sector meshes, variants, skirts on edges and diagonals
- seam tests → hex seam plus coverage proofs
- `effective` → piece-based cut

**Generic core change (one):** index-range or shared-index-buffer draws in `engine-render` and `engine-sdk`, with an ADR amendment. It is topology-agnostic, so the core stays planet-agnostic.

**`Topology` trait: recommend YES, minimal, in `planetary-core::topology`.** It has two real implementations (cube-sphere quadtree and hex), HLD §12.3 says "the cube-sphere is the first implementation, not a mandatory topology", and the cube-sphere is the oracle while hex is being validated.

```rust
pub trait Topology: Send + Sync + 'static {
    type Cell: Copy + Ord + core::fmt::Debug + Send + Sync + 'static;
    fn roots(&self) -> &[Self::Cell];
    fn level(&self, c: Self::Cell) -> u8;
    fn raw(&self, c: Self::Cell) -> u64;                  // stream key / asset id
    fn children(&self, c: Self::Cell, out: &mut Vec<Self::Cell>);   // 4 or 7
    fn parents(&self, c: Self::Cell, out: &mut Vec<Self::Cell>);    // 1 or 2 (0 for roots)
    fn primary_parent(&self, c: Self::Cell) -> Option<Self::Cell>;
    fn neighbors(&self, c: Self::Cell, out: &mut Vec<Self::Cell>);  // 4, 5 or 6
    fn footprint(&self, c: Self::Cell) -> Footprint;      // centre dir, cos/sin angular radius
    fn locate(&self, dir: DVec3, level: u8) -> Self::Cell;
    /// Drawn pieces for a balanced split set: (cell, piece mask) where mask selects full / half variant.
    fn pieces(&self, split: &BTreeSet<Self::Cell>, out: &mut Vec<(Self::Cell, PieceMask)>);
    fn build_mesh(&self, c: Self::Cell, terrain: &PlanetTerrain, n: u32) -> PatchMesh;
}
```

Selection, balance closure, effective cut, requests and the TerrainSystem become generic over `T: Topology`. Making them generic costs about one plan. If the user does not want to keep the cube-sphere, skip the trait (YAGNI per PROJECT.md) and replace it in place.

### 3.5 Q5: Plan shape, effort and risk

**Spikes first** (cheap, each ≤ 1 plan; scratch or test-only code):

| # | Unknown | Spike | Pass criterion |
|---|---|---|---|
| S1 | Diamond (Q2DI) id layout, wrap tables, exactly 12 pentagons | Rust test-only module: enumerate L ≤ 6, check counts `10*4^L+2`, neighbour reciprocity, pentagon count, parent/child round-trips | All invariants hold |
| S2 | Exact-op barycentric mapping is bit-identical on the 30 edges and 12 vertices | Compute every edge and vertex sample from both faces | 0 mismatches, levels 0-6 |
| S3 | Half-cell partition and balance **on the sphere** (only the plane is verified so far) | Rasterise level-(max+1) lattice triangles; random closed and balanced split sets; count coverage | Every triangle covered exactly once; max jump 1 |
| S4 | Gap/skirt bound on hex edges and half diagonals | Port `mixed_level_gap_bounded_by_skirts` with N = 8 | max gap / skirt ≤ 1, plus negative control |
| S5 | Selection convergence at real parameters | Headless: new bounds plus culling on the **existing quadtree** at max_level 16/19, descent to 2 m | wanted == drawn within 240 frames; nodes visited < 8k |
| S6 (optional) | Skirtless locked borders | Enumerate border-type pairs in the S3 rasteriser | Decide later |

**Proposed Phase 7.1 (hex topology), about 10-12 plans:**
1. 7.1-01 Telemetry and crash diagnosis (section 3.6); can run in parallel.
2. 7.1-02 Selection fixes on the current quadtree (S5): footprint bounds, measured ranges, culling, budget, draw-budget-aware leaves, removal of the escalation loop. This closes G1 *now* and is reused unchanged.
3. 7.1-03 Spikes S1-S4 (test-only), then a go/no-go with the user. On no-go, fall back to C (triangles over the same lattice).
4. 7.1-04 `cell_id` and the icosahedron/diamond tables (ids, parents, children, neighbours, tests). Plus ADR-0011 (hex topology) as proposed.
5. 7.1-05 Exact-op mapping and locate, goldens.
6. 7.1-06 `Topology` trait, with the quadtree adapted behind it.
7. 7.1-07 Hex selection, closure/balance and pieces, with coverage proof tests.
8. 7.1-08 Sector meshes, variants and skirts; seam proofs including the negative control.
9. 7.1-09 SDK index-range draw (generic) and its uploader integration.
10. 7.1-10 TerrainSystem generic, payload min/max, effective cut on pieces.
11. 7.1-11 App plus descent **at real default parameters** (max_level 16, no reduced level), release and debug bench, ADR-0011 accepted with evidence, ADR-0010 amended.
12. (7.1-12 reserve for fixes.)

**What the human must verify:**
- the windowed flight: hexes visibly give way to their parts (debug palette per cell and level), no cracks, detail streams in during descent;
- the release-mode frame-time table from the bench;
- the go/no-go after the spikes.

**Effort compared with fixing the quadtree** [ASSUMED estimates]:
- Quadtree fix (G1-G3): about 3-4 plans. Steps 01, 02 and 11 above apply to either path.
- Hex path: about 10-12 plans in total. The topology-specific cost is about 7-8 plans on top of the shared fixes.
- **The hex switch does not by itself fix G1/G2.** The selection fixes do, and they are needed either way. That is why they come first.

**Requirement, roadmap and doc edits needed:**
- **PLAN-02:** "cube-sphere with stable packed patch IDs and a tested face-adjacency table" → "an icosahedral hexagonal cell hierarchy (aperture 4, 12 pentagons) with stable packed cell IDs and a tested neighbour table (cube-sphere retained as reference topology behind a Topology trait)".
- **PLAN-03:** "quadtree with 2:1 balance" → "hierarchical cells with level-difference-1 balance and screen-space-error LOD selection with hysteresis, tight bounds, horizon/frustum culling".
- **PLAN-04:** "across cube-face edges and corners" → "across icosahedron edges, vertices and pentagons, including half-cell borders".
- **ROADMAP:** add Phase 7.1 (above). Phase 7 SC-6 evidence must be at real parameters.
- **PROJECT.md:** add a key-decision row (hex topology, aperture 4, gnomonic).
- **HLD:** §12.3 keeps "not mandatory"; add a note that hex B\* is the second topology. §7.2: optional H3 index adapter later.
- **ADR-0010:** amend, since the technique is unchanged but generalised: hex sectors, diagonal skirts, piece coverage proof.
- **New ADR-0011:** hex topology, ids, projection, half-cell rule, `Topology` trait.
- **ADR-0005/0009 amendment, or new ADR:** index-range draws.

### 3.6 Q6: Descent crash/stutter diagnosis and telemetry (topology-independent)

**Diagnosis steps:**
1. Reproduce in `--release` and in debug, with `RUST_BACKTRACE=1` and stderr redirected to a file. Check the Windows Event Viewer (Application Error / WER) for the faulting module and exception code (0xC0000005 or stack overflow 0xC00000FD versus an out-of-memory abort).
2. **Disk at 100% (6.2 GB free, confirmed).** The pagefile cannot grow, so a large allocation can abort with "memory allocation of N bytes failed". Record peak working set. Free disk before concluding anything.
3. Instrument the counts that can grow without bound per frame: leaves (selection is unbounded when `tau` steps run out), `held` meshes and buffers, pending requests, `draw_overflow` (more than 1,024 draws are silently dropped, which looks like holes), `upload_rejects`.
4. Check wgpu errors: device-lost or validation messages, buffer count, and total bytes against `MAX_BUFFER_BYTES`.
5. Time the phases per frame: select (including escalation reruns), balance, effective cut, reconcile, uploads. The 280 s headless repro points at selection cost × 8 reruns × tens of thousands of nodes in debug [ASSUMED until measured].

**Telemetry design:**
- **Module and clock.** A `telemetry` module in `planetary-app` (or an SDK-level generic `FrameClock` consumer). It reads time through a `Clock` trait: a `WallClock` implementation lives only in the telemetry module, and tests use `SimClock`.
  - The planetary `banned_ops` test bans `Instant`/`SystemTime` in `planetary-core`. Keep wall-clock out of core and exempt only the telemetry module file.
- **Per-frame record:**
  - frame, simulated time, wall `dt_ms`;
  - altitude above the surface, distance to the planet centre;
  - nodes visited, culled (horizon/frustum), budget hits;
  - wanted, drawn, held, pending, started, completed, uploaded, evicted, `draw_overflow`, `tau_eff`;
  - per-phase CPU ms.
- **Outputs:**
  - a console summary once per second (avg / p95 / max frame time, FPS, altitude);
  - `--perf-log <csv>`;
  - `--bench-descent`: headless NullBackend, real default parameters, a fixed scripted descent, and a report of frame-time statistics per altitude band (`>1000 km, 100-1000, 10-100, 1-10 km, 100 m-1 km, <100 m`), run in debug and in `--release`;
  - tests assert only counted values (ADR-0008 rule 5); wall-clock numbers are reported, never asserted.

---

## 4. Nanite: what it is and what to borrow

### 4.1 How Nanite works

| Mechanism | Detail | Confidence |
|---|---|---|
| Clusters | About 128 triangles each (`FCluster::ClusterSize = 128`, which matches the 7-bit triangle id in the visibility buffer); built by METIS-style graph partitioning that minimises shared edges | MEDIUM (secondary source traceyang.com walkthrough of UE source; consistent with the Karis slides via search) |
| DAG build | Group 8-32 adjacent clusters → merge → simplify to about half the triangles with **group-boundary edges locked** → re-split into new clusters → repeat to the root. The grouping differs per level, so a boundary locked at one level becomes interior and simplifiable at the next. Many-to-many, so it is a DAG | MEDIUM-HIGH (Karis slides as quoted by search: "Pick group, Merge and Simplify, Split", "lock shared boundary edges"; secondary sources agree) |
| Error | QEM simplification error per group; monotonic up the DAG (`ParentLODError = max(...)`); bounds must contain the children's bounds or monotonicity breaks (zeux) | MEDIUM |
| Cut | A cluster is drawn iff its own (group's) projected error ≤ threshold and its parent group's error > threshold. Error and bounds are stored per group, so all clusters in a group decide identically and in parallel, without traversing from the root. Monotone error plus locked borders gives a crack-free cut | MEDIUM-HIGH (knnyism.com, three-nanite, traceyang) |
| Culling | GPU persistent-thread work-queue traversal; frustum plus two-pass HZB occlusion (last frame's HZB, then the rebuilt one) | MEDIUM |
| Streaming | Root pages (about 32 KB) always resident give coarse geometry from the first frame; streaming pages (about 128 KB) on demand; compressed on disk and decoded on the GPU | MEDIUM (secondary source; page sizes not checked in Epic docs) |
| Raster | Small triangles go to a compute software rasteriser; large ones to hardware or mesh shaders; 64-bit visibility buffer (depth + cluster/triangle id, `InterlockedMax`); material shading deferred | MEDIUM |
| Landscape | "Nanite can be enabled for landscape terrain"; DX12 SM6 required; static or runtime displacement via tessellation | CITED, HIGH (UE docs) |

The full Karis PDF exceeded the fetch size limit, so its exact slide text was not read directly. Points marked MEDIUM come from reimplementation write-ups that cite it.

### 4.2 What transfers to this engine

| Nanite idea | Transfer? | How |
|---|---|---|
| (a) Cluster DAG with locked borders as the topology | **No, for now.** | It needs an offline or neighbour-aware build (graph partitioning plus a float QEM simplifier, which conflicts with ADR-0008 for hashed or border-critical output). Our planet is generated lazily per cell. With about 128-triangle clusters and CPU draws, a 1080p view (~10^4 clusters) blows far past `MAX_DRAWS_PER_FRAME = 1024` without multi-draw-indirect or GPU culling. A regular lattice already gives the hierarchy for free. |
| (a') Does it remove the hex non-nesting/pentagon problem? | Partly, in principle. | Nanite's answer to non-nesting is "replace whole groups whose outer border is identical". B\* *is* that answer for hexes: parent ↔ {centre + 6 halves} with an identical outer border. Its parent borders become child interiors at the next level, which is Nanite's alternating-boundary property. Skirtless locked borders are a later option (S6). A cluster DAG *inside* each hex cell is unnecessary while each cell is a regular grid. |
| (b) Cut rule, monotone error, tight geometry-derived bounds | **Yes.** | Per-cell rule with error that is monotone over levels. Bounds come from the payload's measured min/max, inherited downward with a residual. This directly fixes the "global relief inflation" failure (G1): Nanite never inflates by a global bound. Assert monotonicity in tests. |
| (c) Root pages plus streaming pages | **Already equivalent.** | `base_levels` always resident = root pages. Pinned ancestor fallback plus `resolve()` = coarse-geometry-first. Keep StreamKey/StreamManager. Each cell is one "page" (~20-60 KB payload at N = 16 [ASSUMED]). |
| (d) CPU-only subset today | Yes. | Hierarchical frustum and horizon culling on the CPU over the cell tree; per-cell error cut; draw-budget-aware selection; shared static index buffers. **Later, with compute:** GPU cut evaluation, HZB occlusion, multi-draw-indirect, sub-sector clusters (each sector = 2-4 clusters of about 128 triangles). Visibility buffer and software raster: out of scope (HLD P0). |
| (e) Effort and risk | Borrowing (b)-(d): about 0 extra plans, since it is folded into 7.1-02. A full Nanite-style terrain is research-grade and needs GPU-driven rendering, which is out of scope for P0. | |

**Extra decision for the user:** confirm "borrow the Nanite selection, bounds, streaming and culling ideas; no cluster DAG or visibility buffer in Phase 7.1; reconsider when GPU-driven rendering is in scope" (decision 8 in section 1).

---

## 5. Risks

1. **The sphere breaks the planar proof (MEDIUM).** Gnomonic faces bend at icosahedron edges, and half-cell borders crossing an edge are polylines bent at the edge point. Combinatorially it should still hold, because cells are defined by the lattice and not by spherical Voronoi regions. S3 must confirm it.
2. **The balance closure forces too many splits (LOW-MEDIUM).** Each split forces up to 2 parents × 6 neighbours. Measure the forced-split ratio in telemetry.
3. **Draw count with half pieces (LOW-MEDIUM).** Mitigate with index-range draws and a draw-budget-aware selection.
4. **The user may expect visibly hexagonal terrain (MEDIUM, expectation).** On a crack-free surface the patch tiling is invisible. The hex structure shows only through LOD transitions and a debug palette or overlay. Show the debug overlay early.
5. **The topology switch is not the fix for G1/G2 (HIGH certainty).** Selection, bounds and telemetry are. Do them first.
6. **Cross-OS goldens are still unverified** (as for ADR-0008/0010).
7. **Disk (6.2 GB free).** The hex work adds no heavy dependencies. Avoid h3o in the build unless the index adapter is approved.

## 6. Environment availability

| Dependency | Needed by | Available | Notes |
|---|---|---|---|
| Rust 1.99.0 | all | ✓ (pinned) | |
| Python 3.11.9 | spike math (optional) | ✓ | used for this research; scratch deleted |
| Free disk | builds | 6.2 GB (C: 100%) | likely factor in G2; free space before benches |
| h3o 0.11.1 | Design A only | not in tree | BSD-3-Clause is allowed by deny.toml; not recommended |

## 7. Validation architecture (Nyquist)

- **Framework:** cargo test (existing). Quick run: `cargo test -p planetary-core`. Full: `cargo test --workspace` plus `cargo clippy -D warnings` plus `cargo deny check`.
- **Tests per requirement:**

| Requirement | Tests |
|---|---|
| PLAN-02 | `cell_id.rs` (round-trip, stray bits, counts `10*4^L+2`, 12 pentagons) and `adjacency.rs` (reciprocity, wrap tables) |
| PLAN-03 | `lod_select.rs` (monotone bounds, hysteresis, budget cap, culling cases) and `lod_balance.rs` (jump ≤ 1) |
| PLAN-04 | `seam.rs` (same-level bit identity, edges and vertices, mixed-level gap/skirt ≤ 1, negative control, **coverage proof**) |
| PLAN-05 | existing generator goldens |
| PLAN-07 | `descent.rs` at real default parameters, plus `--bench-descent` (report only) |

- **Wave 0 gaps:** spike tests S1-S4; a coverage rasteriser helper; a telemetry `Clock` trait with `SimClock`.

## 8. Sources

**Primary (HIGH)**
- h3geo.org Resolution table: https://h3geo.org/docs/core-library/restable (counts, areas, edge lengths, 12 pentagons, area ratio)
- h3geo.org Overview: https://h3geo.org/docs/core-library/overview (gnomonic on icosahedron faces, 122 base cells, aperture 7)
- h3geo.org Indexing highlights: https://h3geo.org/docs/highlights/indexing (approximate containment)
- h3geo.org H3 index: https://h3geo.org/docs/core-library/h3Indexing (64-bit, modes)
- crates.io h3o API (0.11.1, BSD-3-Clause, 2026-11-02); docs.rs h3o Cargo.toml (dependencies including libm)
- UE docs, Nanite Virtualized Geometry: https://dev.epicgames.com/documentation/en-us/unreal-engine/nanite-virtualized-geometry-in-unreal-engine
- Codebase: `extensions/planetary/**`, `crates/engine-render/src/{scene,backend,resources}.rs`, `crates/engine-procedural/src/noise.rs`, ADR-0008, ADR-0010, `07-VERIFICATION.md`

**Secondary (MEDIUM)**
- Karis, Stubbe, Wihlidal, "Nanite: A Deep Dive", SIGGRAPH 2021 Advances: https://advances.realtimerendering.com/s2021/Karis_Nanite_SIGGRAPH_Advances_2021_final.pdf (fetched only via search excerpts; too large to fetch directly)
- Nanite end-to-end walkthrough: https://traceyang.com/en/2026/06/11/nanite-end-to-end-pipeline-walkthrough-qa/
- Nanite at Home (cut rule): https://knnyism.com/posts/nanite/
- meshoptimizer discussion #750 (monotonicity, bounds containment): https://github.com/zeux/meshoptimizer/discussions/750
- Recreating Nanite, LOD generation: https://jglrxavpok.github.io/2024/01/19/recreating-nanite-lod-generation.html
- Cesium horizon culling: https://cesium.com/blog/2013/04/25/horizon-culling/
- NWP-SAF report (hex grids not congruent): https://nwp-saf.eumetsat.int/publications/vs_reports/nwpsaf-mo-vs-028.pdf
- DGGRID manual / dggridR (aperture 3/4/43 hex, ISEA3H/4H): https://cran.r-project.org/web/packages/dggridR/vignettes/dggridR.html
- Sahr, "Location coding on icosahedral aperture 3 hexagon DGGs" (30° rotation): https://www.researchgate.net/publication/223763428

**Verified in this session by scratch scripts (Python, deleted afterwards)**
- Aperture-4 planar hierarchy: parent ⊆ ∪children; half-cell sector alignment; exact coverage (0/90,000 violations); balance max jump 1 (2 without the rule)
- Aperture-7 non-coverage: 7.26% of the parent
- Gnomonic icosahedral distortion (hex area ratio 1.52 / 1.87 / 1.96 at L = 3 / 5 / 7; cell counts match `10*4^L+2`)
- Per-level size table

**Assumed (needs confirmation)**
- H3 bit widths and the Class II/III 19.1° rotation
- Q2DI diamond layout details
- wgpu index-range draw details
- per-level draw-count estimates
- payload size
- the 8k node budget default
- the cause of the 280 s repro

## Metadata

- Standard stack: HIGH (no new dependencies; h3o facts verified).
- Architecture (B\*): MEDIUM. Verified in the plane; sphere spikes pending.
- Nanite: MEDIUM. Secondary sources; primary PDF only via excerpts.
- Pitfalls/G1 diagnosis: HIGH (from code).
- Valid until: about 2026-12-25 (stable domain).
