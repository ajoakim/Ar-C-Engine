# ADR-0005: Rendering API - wgpu Behind an Engine-Owned Backend Trait

- Status: accepted
- Date: 2026-11-25
- Requirement/Phase: REND-01, WS-05 (rendering API) / Phase 3

## Context

Render backends must be replaceable (HLD). wgpu breaks its API roughly every three months, so its types must not spread through the engine. Headless tests must run without a GPU. ADR-0002 and ADR-0003 already confine wgpu to `engine-render-wgpu` and winit to `engine-platform`.

## Decision

- Use wgpu 30.0.2 with WGSL shaders, default features and an auto-selected native backend.
- `engine-render` defines a `RenderBackend` trait (create_buffer, destroy_buffer, create_pipeline, resize, size, submit(&FramePacket)) over engine-owned `BufferId`, `PipelineId`, `TextureId` and `MeshHandle`. Built-in shaders and vertex layouts are named by engine enums (`ShaderId`, `VertexLayout`).
- No wgpu type appears in any `engine-render` signature.
- `NullBackend` in `engine-render` records frames and validates handles through a shared `ResourceTable`, so both backends use the same validation.
- Window handles cross from `engine-platform` to `engine-render-wgpu` only as `Arc<dyn RenderTarget>` (raw-window-handle 0.6 traits). `engine-render-wgpu` therefore does not depend on `engine-platform`.
- Surface creation is a constructor on the concrete `WgpuBackend`, not part of the trait.
- Depth is reverse-Z Depth32Float, cleared to 0.0, compare Greater, infinite far plane, per ADR-0001.
- The backend is owned by the SDK app loop on the main thread and is never registered as an engine `Module`.
- cargo-deny `multiple-versions` stays at `warn` because the wgpu and winit stacks duplicate crates.

## Alternatives Considered

- ash (raw Vulkan): unsafe-heavy and Vulkan only.
- vulkano: Vulkan only.
- Bevy renderer or rend3: ties the engine to Bevy's ECS/app model, or has low activity.
- wgpu types directly in `engine-render`: couples every crate to wgpu's release cadence.
- `engine-render-wgpu` depending on `engine-platform` for the window: allowed by the tiers but couples the backend to windowing.

## Consequences

- wgpu upgrades touch one crate.
- The trait surface is intentionally small and grows per phase.
- Validation is not duplicated, because both backends use `ResourceTable`.
- Linux Wayland may need the display handle at instance creation (deferred to Phase 8).
- Software-adapter CI is deferred to Phase 8.

## Evidence

- A scratch crate outside the repo (toolchain 1.99.0) compiled wgpu 30.0.2, winit 0.30.13, raw-window-handle 0.6.2 and pollster, passed an `Arc<dyn RenderTarget>` to `create_surface`, and `request_adapter` succeeded (Intel Arc B580, Vulkan).
- `cargo deny check licenses bans` on that 251-package tree had zero license failures.
- Follow-up evidence: the Phase 3 null-backend tests and the manual triangle smoke run (`cargo run -p engine-sdk --example triangle`).
