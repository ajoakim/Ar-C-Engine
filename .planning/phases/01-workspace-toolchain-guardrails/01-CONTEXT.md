# Phase 1: Workspace, Toolchain & Guardrails - Context

**Gathered:** 2026-11-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Developers can build a workspace whose architectural boundaries are enforced automatically from day one (WS-01..WS-04): pinned toolchain (Rust >= 1.95, edition 2024), coarse crates with documented dependency direction, CI dependency-direction checks (core never depends on extensions; wgpu/winit only in designated crates), ADR template and conventions ADR.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Guidance from research (.planning/research/SUMMARY.md, STACK.md, ARCHITECTURE.md): ~8-12 coarse crates, cargo-deny for dependency-direction/isolation, CI on Windows + Linux, decide separate `engine-render-backend` crate vs folded into `engine-render` here. Local rustc is 1.69; toolchain install via rustup is required to verify builds (flag to user before installing system toolchains).

</decisions>

<code_context>
## Existing Code Insights

Greenfield — no code exists yet. Only HLD.md and .planning/ artifacts.

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None
</deferred>
