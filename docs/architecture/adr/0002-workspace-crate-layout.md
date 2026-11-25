# ADR-0002: Workspace Crate Layout, Render Split and License

- Status: accepted
- Date: 2026-11-25
- Requirement/Phase: WS-02 / Phase 1

## Context

The engine core must never depend on extensions, and third-party GPU/windowing crates must be isolated behind single crates. The crate boundaries must be coarse enough to avoid empty fake boundaries yet fine enough to enforce the dependency direction. An earlier note (ARCHITECTURE.md) suggested folding the render backend into `engine-render` until a second backend exists.

## Decision

- 13 coarse crates (11 under `crates/`, 2 under `extensions/`) as listed in `docs/architecture/dependency-direction.md`, plus the `xtask` tooling crate. Streaming, procedural, additional planetary crates and examples are deferred to the phases that first use them.
- Render split: `engine-render` (backend-agnostic `RenderScene`, extraction, render graph, backend trait; no GPU API types) and `engine-render-wgpu` (sole wgpu dependent). A dedicated crate makes the wgpu allowlist exactly one crate and keeps extraction tests GPU-free. This consciously overrides ARCHITECTURE.md's "fold until a second backend".
- License: dual `MIT OR Apache-2.0` (Rust ecosystem norm), declared once in `[workspace.package]`.
- Toolchain: `rust-toolchain.toml` pins `1.99.0`, MSRV `rust-version = "1.95"`, edition 2024, resolver 3.

## Alternatives Considered

- Name `engine-render-backend`: rejected, `engine-render-wgpu` is self-documenting in ban lists.
- Fold the backend into `engine-render`: rejected, the wgpu allowlist would cover the whole render crate and extraction tests would need GPU types.
- Single-license MIT: rejected, dual licensing follows the Rust ecosystem norm and adds the Apache patent grant.

## Consequences

- One extra tiny crate.
- Extension crates must re-check the allowlist when egui/winit integration lands (Phase 8).
- Adding crates requires updating the layer table (see ADR-0003).

## Evidence

- `.planning/phases/01-workspace-toolchain-guardrails/01-RESEARCH.md` (crate layout decision and tiers).
