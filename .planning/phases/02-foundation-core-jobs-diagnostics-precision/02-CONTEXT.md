# Phase 2: Foundation (Core, Jobs, Diagnostics, Precision) - Context

**Gathered:** 2026-11-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Applications can drive an engine lifecycle with modules, jobs and diagnostics, on a precision-safe position model (CORE-01, CORE-02, CORE-04, CORE-05, WORLD-02, WORLD-04). Crates: engine-math, engine-core, engine-jobs, engine-diagnostics (workspace from Phase 1; conventions in ADR-0001).

</domain>

<decisions>
## Implementation Decisions

### WorldPos / Precision
- Opaque `WorldPos` wrapping f64 `DVec3` plus a `Frame` id; internals swappable later (i64 cell + offset remains a possible ADR spike).
- Coordinate spaces are explicit `Frame` handles with registered parent transforms; conversions go through frames, never implicit mixing.
- Origin rebasing is an explicit `rebase(new_origin)` that shifts registered positions; trigger policy is left to app/module.

### Lifecycle & Modules
- `Module` trait with init/load/update/suspend/shutdown hooks; registry orders modules by declared dependencies.
- Typed `EngineConfig` loaded from RON (serde) at Initialize, with per-module sections.
- `Engine` state machine Initialize → Load → Running → Suspend → Shutdown; illegal transitions return errors (no panics).

### Jobs
- Custom priority queue on std threads + crossbeam channels with priority classes and atomic cancel tokens.
- Cooperative cancellation: `CancelToken` checked by jobs; queued jobs dropped on cancel.
- Single-worker test mode for deterministic priority-order tests.

### Diagnostics
- `tracing` with a thin `engine-diagnostics` wrapper that installs the subscriber.
- Per-frame `FrameStats` (frame time + named span timings) exposed via API for later overlays.

### Claude's Discretion
Exact type/method names, error types (thiserror in libraries), module file layout inside crates, test structure.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- Phase 1 workspace: 13 stub crates with tiered path deps; `cargo xtask check-deps` enforces layering (layer table must stay in sync when crates/deps change).

### Established Patterns
- Rust 1.99.0 pinned, edition 2024, workspace lints (clippy -D warnings), ADR-0001 conventions (SI meters, RH Y-up, f64 world / camera-relative f32).

### Integration Points
- engine-math/engine-diagnostics/engine-core/engine-jobs stubs get real code; engine-sdk will re-export in Phase 3.

</code_context>

<specifics>
## Specific Ideas

Test a position at 6.4e6 m and rebasing round-trips preserve relative offsets.

</specifics>

<deferred>
## Deferred Ideas

Automatic rebasing in the engine loop; i64-cell precision representation (ADR spike later).

</deferred>
