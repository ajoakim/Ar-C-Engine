# General-Purpose 3D Engine

## What This Is

An open-source, general-purpose 3D engine written in Rust, designed to support applications ranging from conventional 3D games and simulations to planetary-scale visualization, scientific applications, and other specialized spatial environments. The initial development effort uses planetary-scale rendering and exploration as its primary reference implementation — an architectural stress test — while a conventional 3D scene serves as a secondary reference use case validating that the engine core remains useful without planetary systems.

## Core Value

The engine core must remain general-purpose and independent of any specialized capability (planetary rendering, physics, scripting, etc.) — specialized implementations are validated through demanding reference applications but never become assumptions embedded in the core.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Establish a modular Rust workspace with clear crate boundaries and dependency direction favoring a small, stable engine core (Section 20.2)
- [ ] Implement engine lifecycle (Initialize → Load → Running → Suspend → Shutdown), configuration, module registration, and a minimal public SDK
- [ ] Implement world/scene/entity/transform/identity architecture supporting multiple coordinate spaces and high-precision positioning
- [ ] Implement rendering framework with a replaceable backend boundary, render graph, and render-data extraction independent of geometry source
- [ ] Implement spatial interfaces (query/visibility/region-lookup) behind pluggable backends, with camera-relative transforms and origin rebasing
- [ ] Implement asset/resource lifecycle with stable identifiers, CPU/GPU residency states, and caching
- [ ] Implement job scheduling and asynchronous streaming with priority, cancellation, coarse fallback, and memory budgets
- [ ] Implement general-purpose procedural generation framework with deterministic, independently-seeded layers
- [ ] Implement planetary extension as a separate module: cube-sphere topology, stable patch IDs, quadtree hierarchy, planetary camera with orbit-to-surface navigation
- [ ] Implement screen-space-error terrain LOD with crack-free transitions, coarse fallback, and async generation/streaming that never blocks the render loop
- [ ] Deliver two working reference applications (conventional 3D + planetary) built against the public SDK
- [ ] Establish diagnostics, testing (unit/integration/reference/performance/regression), and ADRs for major technical decisions

### Out of Scope (P0)

- Editor / visual development environment — deferred to P2.5+ after runtime and SDK stabilize
- Physics engine integration — future phase, optional capability
- Gameplay scripting / scripting VM — future capability, not committed before evaluation
- Large-scale ECS commitment — architecture must not prematurely commit to one ECS implementation
- Full geospatial data ingestion pipeline (NASA/USGS/Copernicus/etc.) — P2
- Persistent world editing / modification overlays — P2
- Full weather, day-night cycle, seasons, clouds — later phases
- Plugin marketplace / dynamic plugin loading / external plugin ABI — deferred until a real requirement emerges
- Ray tracing / advanced GPU-driven rendering — future rendering capabilities, not required for initial prototype
- Advanced simulation, comprehensive animation system — future phases

## Context

This project starts from a single source document: `HLD.md` (v2.0, ~2,257 lines), the High-Level Design authored as an architecture proposal and foundation for implementation. The repository contains no code yet — this is a greenfield start. The HLD's Section 22 ("First Coding-Agent Assignment") is the immediate implementation target: bootstrap a Rust workspace and deliver a working prototype (engine core, SDK foundation, rendering, spatial, planetary extension, terrain LOD, async generation/streaming, diagnostics, tests, docs) validated by two reference applications. Section 23 lists open technical decisions (rendering API, ECS, spatial backends, planet LOD/transition technique, etc.) that must be resolved through early P0 prototyping and benchmarking rather than assumed. Section 24's guiding statement: "Build a general-purpose engine that provides stable foundational capabilities and extensible interfaces. Validate those capabilities through demanding reference applications, beginning with planetary-scale rendering, while ensuring that specialized implementations remain independent of the engine core."

## Constraints

- **Tech stack**: Rust is the primary and only implementation language for the engine core, SDK, and extensions.
- **Architecture**: Engine core must not depend on planetary topology, terrain generation, geographic datasets, a particular physics engine, or a particular editor (HLD §5.1).
- **Architecture**: Planetary-specific functionality must live only in the planetary extension, never in the general engine core (HLD §22.4).
- **Performance**: Rendering must never block synchronously on terrain generation or loading; generation/streaming must be asynchronous (HLD §22.4, §12.8).
- **Scope discipline**: No premature ECS, physics, scripting, editor, plugin marketplace, geospatial ingestion, weather/day-night, or ray tracing commitments in P0 (HLD §22.4).
- **Simplicity**: Prefer simple, measurable implementations; avoid abstractions with no current use or clear architectural justification (HLD §22.4).
- **Documentation**: Assumptions, limitations, and architectural decisions (ADRs) must be recorded as work proceeds (HLD §22.4).

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Use GSD (get-shit-done) as the multi-agent planning/execution workflow | User confirmed GSD is a good fit for phased, multi-agent development of this engine | — Pending |
| Granularity: Standard (5-8 phases, 3-5 plans each) | Balanced phase size for a project of this scope | — Pending |
| Execution: Parallel plans where independent | Speeds up independent deliverables (e.g., core vs. rendering vs. spatial) | — Pending |
| Commit planning docs to git | Full traceability of planning artifacts | — Pending |
| Research/Plan-check/Verifier agents enabled | Extra quality gates before/after each phase | — Pending |
| Model profile: Balanced (Sonnet) | Good quality/cost ratio for most agents | — Pending |
| Rendering API, ECS, spatial backends, planet LOD/transition technique | Deferred — must be decided via early P0 prototyping/benchmarking per HLD §23 | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-11-25 after initialization*