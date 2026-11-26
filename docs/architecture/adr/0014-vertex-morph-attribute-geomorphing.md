# ADR-0014: Vertex Morph Attribute for Geomorphing (Cell Payload v4)

- Status: accepted
- Date: 2026-11-26
- Requirement/Phase: HEX-02, HEX-03, PERF-02 / Phase 7.1

## Context

When the effective cut changes, a leaf is replaced by its children (or the reverse) in one frame. The finer mesh has more vertices on the same surface, and its new vertices lie off the coarse piecewise-linear surface by the geometric error of the coarse level, so the replacement is visible as a pop. Geomorphing removes the pop by drawing a cell at a blend between its own vertices and the surface of its parent-level mesh, with the blend driven by the cell's distance. The data path (worker to GPU) must carry the second surface, and the decision must stay generic: the render layer knows a per-vertex second position and uv and a per-draw factor, and nothing about terrain.

## Decision

1. Each vertex carries a second position (`morph_position`) and a second uv (`morph_uv`). The per-draw factor `m` in [0, 1] is a uniform; the vertex shader blends `mix(pos, morph_position, m)` and `mix(uv, morph_uv, m)`. At `m = 0` the draw is bit-identical to the plain layout (plan 21 identity tests).
2. Morph target definition: the surface of the parent-level mesh at the same direction. The target of a vertex with unit direction `d` is the intersection of the ray `d` with the flat parent triangle that contains `d`, `P = d / (w_a + w_b + w_c)` in barycentric weights of the triangle; `morph_uv` uses the same weights on the parent vertex uvs. All arithmetic is exact-op f64 (`+ - * /` and `sqrt`).
3. Cube topology: the parent lattice is an integer sub-lattice of the child lattice. Even-index vertices copy the parent vertex exactly; odd-index vertices are projected onto the parent triangle.
4. Hex topology: the primary parent (lowest raw id) is tried first; a vertex outside it (the half-cell sides) uses the second parent; a vertex outside both keeps its own position (identity). Pentagon cells use the same rule.
5. Borders and skirts: border and skirt vertices morph too, and the target is position-only (no height from a differently sampled neighbour), so same-level neighbours compute identical border targets. The measured maximum `|target - position|` on the border ring is below the skirt depth (see Evidence), so mismatched factors between neighbours stay covered by the skirts of ADR-0010. No fallback (border pinned to `m = 0`) was needed.
6. Colour attribute: `uv.x` (height) and `uv.y` (mask row) are both already used, so a second uv, `morph_uv`, is carried. The vertex grows from 32 to 52 bytes (position 3, normal 3, uv 2, `morph_position` 3, `morph_uv` 2 floats): +62% bytes per cell. The shading normal is NOT morphed (there is no second normal); this is a documented limitation, the lighting error is small because the parent and child surfaces differ by less than the geometric error.
7. Payload v4: `CELL_PAYLOAD_VERSION = 4`, `CELL_VERTEX_BYTES = 52`, exact length `CELL_HEADER_LEN + vertex_count * 52`, all 13 floats finite. The decoder rejects v3 (`BadVersion`). The v3 encoder is kept, doc-hidden and used only by tests, so the v3 byte goldens remain as `*_v3` tests. One constant (`CELL_VERTEX_BYTES`) feeds `cell_payload_estimate` and so every residency and byte budget.
8. Exact-op boundary: the morph factor is computed on the CPU in f64 with `+ - * / sqrt` only and passed as an f32 per-draw uniform. Morph fields and the factor are never part of a terrain hash or a border-critical path (ADR-0008).

## Alternatives Considered

- Octave cut (evaluate the height function with fewer octaves for the target): rejected. The parent mesh is piecewise linear between its samples, so a height function with fewer octaves is not the parent mesh; at `m = 1` the child would not equal the parent surface and the handover to the parent would pop by about the geometric error, which is exactly what must be removed.
- CPU re-mesh per frame: rejected; it costs a mesh build per visible cell per frame and breaks the "render never waits on generation" rule.
- Vertex texture fetch of the parent heights: rejected; it needs a height texture per level, a second sampler path, and does not match the piecewise-linear parent surface.
- Cross-fade or dither between levels: rejected; it draws both levels (up to 2x draws and overdraw) and dither is visible as noise.
- Packing morph as a single height delta along the normal: rejected; the child and parent differ laterally on the hex half-cell sides and at skirts, a radial delta cannot represent that.

## Consequences

- +62% bytes per cell (same cell counts), so residency byte budgets are hit earlier at equal settings; measured in the plan 23 summary.
- Worker mesh time grows by the target construction: hex about 4.7 triangle tests per vertex, cube about +25% terrain samples (plan 22 counts).
- v3 payloads are rejected; any persisted v3 cache must be regenerated (there is none in tree).
- The shading normal is not morphed.
- The factor-driving logic (distance bands and the border ring bound) lands in plan 24 and amends this ADR with its evidence.

## Evidence

- Plan 22 oracle: cube, 36 cells and 3796 vertices, worst position error 4.0e-9 m, 1124 bit-exact parent copies; hex, 85 cells and 16752 vertices, worst error 3.7e-9 m, identity fallbacks 0 for centre and edge cells, 4932 vertices in the second parent.
- Plan 22 seams: same-level border targets differ by 0 (cube 2040 pairs, hex 5040 pairs); cross-level fine targets lie on the coarse border within 9.3e-10 m; max border displacement over skirt depth is 0.066 (cube), 0.069 (hex outer) and 0.036 (hex half diagonals); negative controls were detected.
- Plan 21 identity tests (Intel Arc B580, Vulkan): morph pipeline at `m = 0` and fog density 0 render the same image hash `583ae580ef4842fd` as before the change.
- Plan 23: v4 round trip is exact, v3 is rejected, the v3 hex golden `0xa826ad7efa5959b6` is unchanged and the v4 golden is `0x17a07e461b54c31b`; terrain meshes upload as the morph layout and draw at `m = 0` with all existing descent, streaming and app tests unchanged; bench numbers are in the plan 23 summary.

## Amendment (plan 24): driving

The factor is a pure function of the view and the cell, computed in `TerrainSystem::gpu_frame` from f64 distances (`+ - * /` and `sqrt` inside `cell_distance`) and converted once to f32 (ADR-0008). Selection is untouched.

- Threshold: `refine_distance(model, proj_scale, tau, level) = geometric_error(level) * proj_scale / tau` is the single source of the selector's per-level split distance (ADR-0012); a test ties it to `screen_space_error` and the hysteresis test. A cell of level `L` is refined below `d_L = refine_distance(L)`.
- Formula: for a drawn cell of level `L >= 1` at distance `d` (distance to the extruded footprint, clamped to `min_distance`), `m = morph_factor(d, d_start, d_end)` with `d_start = refine_distance(L - 1)` (the distance where the parent was split) and `d_end = morph_end_scale * refine_distance(L)` (default 1.25), clamped linear: 1 at `d >= d_start`, 0 at `d <= d_end`. `1.0 <= morph_end_scale < 1.9` keeps `d_end(L) < d_start(L)` because consecutive distance thresholds differ by a factor of at least 2. Level 0 is always 0.
- First-frame guard: a cell that was not shown last frame and has a parent in the previous drawn set (it appeared by refinement) is shown at exactly 1.0; its vertices then lie on the parent mesh surface (plan 22), so the displacement at the handover is `m_parent_shown * |own - morph|` of the parent, and zero once the parent has finished morphing.
- Slew guard: the shown value is `max(m_distance, previous - 1 / morph_frames)` (default 8), counted in frames, so a fast camera move cannot snap a factor from 1 to 0.
- Coarsening: the selector merges children when the parent's distance exceeds `d_L / (1 - hysteresis) = 1.25 * d_L`; the children's `d_start` is `d_L`, so they are already at 1 when the parent replaces them, and the parent appears at its distance-based value (about 0 at that distance), not at 1.
- Telemetry: `morph_min`, `morph_mean`, `morph_first_frame_defects`, `handover_gap_max`, `multi_level_handovers` (CSV columns and `TerrainReport`).

Measured (release tests `morph_handover`, hex N=16 and cube N=32, defaults; the exact numbers are in the plan 24 summary): the first-frame guard holds on every refine event (0 defects); with a 0.5 % per frame approach every refine event after the initial stream-in has `m_parent_shown == 0` exactly; the scripted and fast descents satisfy `m_parent * delta_parent <= m_parent * (geometric_error + residual)` of the grandparent level (the target of the parent's own vertices) with the measured delta at most 6 % of the bound; every coarsen event outside budget/trim/eviction frames replaces children shown at 1.

Known limits: multi-level jumps (a drawn ancestor replaced by descendants more than one level deeper, caused by budget trims, fast flight and streaming catch-up) are counted, not hidden, because the single-parent target cannot be exact there; during the initial stream-in the cut climbs one level per few frames and a freshly refined parent is still slewing when it is replaced (handover gap up to 0.875); the border ring relies on the skirts as in decision 5; the shading normal is not morphed. The visual judgement is Pending (human).
