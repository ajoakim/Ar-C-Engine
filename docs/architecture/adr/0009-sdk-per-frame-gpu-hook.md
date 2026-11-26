# ADR-0009: SDK Per-Frame GPU Hook and Mesh Destruction

- Status: accepted
- Date: 2026-11-25
- Requirement/Phase: APP-02, PLAN-07 / Phase 7

## Context

ADR-0007 (decision 7) limited `Resources` to `App::setup`. Streamed content (for example terrain patches that appear and disappear while the camera moves) must upload and free GPU meshes after setup, every frame. The SDK had no way to do that, and no way to free a mesh at all.

## Decision

1. `App::gpu_frame(&mut self, &mut Resources<'_>, &mut SceneState) -> Result<(), SdkError>` is a new trait method with a default no-op body. Existing apps compile and behave unchanged.
2. `Runtime::frame` calls it once per frame after `App::update` and the engine tick and before extraction. It also runs when the surface is zero-sized. It is not called on a frame where `update` returned `Exit`. An error aborts the frame and is returned from `Runtime::frame`.
3. `Resources::destroy_mesh(MeshHandle)` destroys the vertex buffer and, if present, the index buffer. Both destroys are attempted and the first error is returned. A double destroy is an `Err` from the backend, never a panic.
4. `MAX_DRAWS_PER_FRAME` is re-exported so apps can bound draw lists.
5. The SDK contains no planetary concept: the hook is generic and works for any runtime-produced resource.

## Alternatives Considered

- Passing `Resources` into `update`: a breaking signature change for every app. Rejected.
- A shared index-buffer API: a possible later optimisation, deferred.
- A streaming-specific SDK type: rejected, the core must stay generic (Core Value).

## Consequences

- Apps own mesh lifetimes and must reconcile eviction against GPU residency themselves.
- Double destroy is an error rather than a panic, so reconciliation bugs are reportable.
- Every frame pays one virtual call for apps that do not override the hook (negligible).

## Evidence

`crates/engine-sdk/tests/gpu_frame.rs`: `default_hook_is_noop`, `hook_called_once_per_continue_frame_not_on_exit`, `create_then_destroy_indexed_mesh`, `create_then_destroy_non_indexed_mesh`, `double_destroy_is_error_not_panic`, `gpu_frame_error_propagates`, `hook_runs_with_zero_size_surface`.
