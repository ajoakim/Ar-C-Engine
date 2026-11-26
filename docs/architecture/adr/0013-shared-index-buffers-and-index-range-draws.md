# ADR-0013: Shared Index Buffers and Index-Range Draws

- Status: accepted
- Date: 2026-11-26
- Requirement/Phase: HEX-02, HEX-03 / Phase 7.1

## Context

A drawn piece of geometry is one vertex buffer plus one of several static index lists. Giving every mesh its own index buffer wastes memory when many meshes use identical index lists, and forces duplicated meshes when only part of an index list should be drawn: a partial draw needs a second mesh, which costs up to about 1.9x the draw calls. The render layer could only draw `0..count` of a per-mesh index buffer.

## Decision

1. `MeshIndices` becomes `{ buffer, first, count, shared }`. `MeshHandle::with_indices` is unchanged in behaviour (`first = 0`, `shared = false`).
2. `IndexBufferHandle { buffer, count, max_index }` describes a shared static u32 index buffer. `MeshHandle::with_index_range(&ib, first, count)` attaches a range and sets `shared = true`. It fails when `count` is zero or not a multiple of 3, when `first + count` (computed in u64) exceeds `ib.count`, or when `ib.max_index >= vertex_count`.
3. `ResourceTable` records the maximum index of every index buffer on upload (data length must be a multiple of 4). `validate_packet` re-checks every draw before submit: `(first + count) * 4 <= buffer size` in u64 arithmetic, and the recorded `max_index < vertex_count`. A forged handle therefore cannot read out of bounds.
4. The wgpu backend draws `draw_indexed(first..first + count, 0, 0..1)`; `base_vertex` is fixed at 0. `MAX_DRAWS_PER_FRAME` is unchanged.
5. SDK `Resources` gains `create_index_buffer`, `create_lit_vertices` (vertex buffer only) and `destroy_index_buffer`. `destroy_mesh` never destroys an index buffer whose `shared` flag is set; the app owns shared buffers. A draw that references a destroyed shared buffer is rejected with `InvalidHandle`.
6. The change is generic: the core contains no planetary concept.

## Alternatives Considered

- Two meshes per partial piece: up to about 1.9x draws and duplicate vertex data. Rejected.
- Multi-draw-indirect: GPU-driven rendering is out of P0 scope. Rejected.
- `base_vertex` packing of several vertex sets into one buffer: not needed yet; can be added later without breaking this API.

## Consequences

- Shared buffers must outlive every mesh that draws from them; the app controls their lifetime.
- Existing `with_indices`, `create_lit_mesh` and `destroy_mesh` behaviour is unchanged.
- Validation is done at handle creation and again at submit.

## Evidence

- engine-render `null_backend` tests: `shared_index_buffer_range_draws`, `range_out_of_bounds_rejected`, `max_index_beyond_vertex_count_rejected` pass.
- engine-sdk `index_range` tests (5): two ranges of one buffer yield two draws, destroy_mesh keeps the shared buffer, destroy_index_buffer and double-destroy errors, destroyed shared buffer rejects the frame, range validation, create_lit_mesh still owns its buffer. All pass.
- Planetary-absent canary and dependency checks pass.
- Goldens/hosted CI pending: Windows-only until GitHub billing is resolved.
