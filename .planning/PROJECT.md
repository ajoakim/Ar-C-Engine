# General-Purpose 3D Engine

## What This Is

An open-source, general-purpose 3D engine written in Rust, designed to support applications ranging from conventional 3D games and simulations to planetary-scale visualization, scientific applications, and other specialized spatial environments. The initial development effort uses planetary-scale rendering and exploration as its primary reference implementation — an architectural stress test — while a conventional 3D scene serves as a secondary reference use case validating that the engine core remains useful without planetary systems.

## Core Value

The engine core must remain general-purpose and independent of any specialized capability (planetary rendering, physics, scripting, etc.) — specialized implementations are validated through demanding reference applications but never become assumptions embedded in the core.

## Current State

Shipped v1.0 (2026-11-27): about 55,300 lines of Rust across a tiered Cargo workspace (core, platform, render, world/spatial/assets, jobs/streaming/procedural, SDK, debug-ui, planetary extension), two SDK-only reference apps (conventional-3d, planetary-app) and ADRs 0001-0019. All 45 v1 requirements are complete; milestone accepted with tech debt (see `.planning/v1.0-MILESTONE-AUDIT.md`). The planetary reference runs on an H3-inspired hex topology with the cube-sphere kept as oracle.

## Current Milestone: v1.1 Visual Fidelity & Flight

**Goal:** Make the planetary reference look and fly better (seamless LOD detail transitions, real terrain relief with lit normals, 6DOF free flight) while closing the v1.0 tech debt, without adding planetary assumptions to the engine core.

**Target features:**
- LOD detail transition: no blur-then-sharpen pop at level changes (cross-fade or morph of colour/detail between levels)
- Terrain relief and normals: generator v3 with hills and mountains, height-based normals / bump shading
- Free-flight 6DOF camera (roll, strafe, vertical) in the planetary example/app
- Tech-debt sweep: IN-01..03, 03-VALIDATION, Nyquist phases 1-5, macOS goldens (or documented skip), overlay count formatting and detail-patch cap display

**Key context:** All planetary-specific work stays in the planetary extension; the engine core stays general-purpose (any generic render or camera capability must be expressed without planetary concepts). No ECS, physics, scripting or editor work.

## Requirements

### Validated

- ✓ Modular Rust workspace with enforced crate boundaries and dependency direction — v1.0
- ✓ Engine lifecycle, configuration, module registration and minimal public SDK — v1.0
- ✓ World/entity/transform/identity with multiple coordinate spaces and high-precision `WorldPos` — v1.0
- ✓ Rendering framework with replaceable backend boundary, render graph and geometry-agnostic extraction — v1.0
- ✓ Pluggable spatial interfaces with camera-relative transforms and origin rebasing — v1.0
- ✓ Asset/resource lifecycle with stable IDs and CPU/GPU residency states — v1.0
- ✓ Job scheduling and async streaming with priority, cancellation, coarse fallback and budgets — v1.0
- ✓ Deterministic, independently seeded procedural generation framework — v1.0
- ✓ Planetary extension (stable cell IDs, hex/cube topologies, planetary camera, orbit-to-surface) — v1.0
- ✓ Screen-space-error terrain LOD, crack-free, never blocking render — v1.0
- ✓ Two reference applications on the public SDK — v1.0
- ✓ Diagnostics, tests (unit, integration, perf, image-diff) and ADRs for open decisions — v1.0

### Active

Defined in `.planning/REQUIREMENTS.md` (v1.1): VIS (LOD detail transition), TERR (terrain relief and normals), CAM (6DOF free flight), DEBT (tech-debt sweep).

### Out of Scope (P0)

- Editor / visual development environment — deferred until runtime and SDK stabilize
- Physics engine integration — future phase, optional capability
- Gameplay scripting / scripting VM — future capability, not committed before evaluation
- Large-scale ECS commitment — ADR-0017 spike did not justify adoption; revisit on evidence
- Full geospatial data ingestion pipeline (NASA/USGS/Copernicus/etc.) — P2
- Persistent world editing / modification overlays — P2
- Full weather, day-night cycle, seasons, clouds — later phases
- Plugin marketplace / dynamic plugin loading / external plugin ABI — deferred until a real requirement emerges
- Ray tracing / advanced GPU-driven rendering — future rendering capabilities
- Advanced simulation, comprehensive animation system — future phases

## Context

This project starts from a single source document: `HLD.md` (v2.0, ~2,257 lines), the High-Level Design authored as an architecture proposal and foundation for implementation. The HLD's Section 22 ("First Coding-Agent Assignment") is the immediate implementation target: bootstrap a Rust workspace and deliver a working prototype (engine core, SDK foundation, rendering, spatial, planetary extension, terrain LOD, async generation/streaming, diagnostics, tests, docs) validated by two reference applications. Section 23 lists open technical decisions (rendering API, ECS, spatial backends, planet LOD/transition technique, etc.) that must be resolved through early P0 prototyping and benchmarking rather than assumed. Section 24's guiding statement: "Build a general-purpose engine that provides stable foundational capabilities and extensible interfaces. Validate those capabilities through demanding reference applications, beginning with planetary-scale rendering, while ensuring that specialized implementations remain independent of the engine core."

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
| Use GSD (get-shit-done) as the multi-agent planning/execution workflow | User confirmed GSD is a good fit for phased, multi-agent development of this engine | ✓ Good (v1.0) |
| Granularity: Standard (5-8 phases, 3-5 plans each) | Balanced phase size for a project of this scope | ✓ Good (v1.0) |
| Execution: Parallel plans where independent | Speeds up independent deliverables (e.g., core vs. rendering vs. spatial) | ✓ Good (v1.0) |
| Commit planning docs to git | Full traceability of planning artifacts | ✓ Good (v1.0) |
| Research/Plan-check/Verifier agents enabled | Extra quality gates before/after each phase | ✓ Good (v1.0) |
| Model profile: Balanced (Sonnet) | Good quality/cost ratio for most agents | ✓ Good (v1.0) |
| Planet topology: H3-inspired hex grid (half-cell, 4-children nested scheme) instead of cube-sphere; real H3 rejected (non-nesting children leave ~7% gaps; needs trig) | User's original inspiration was Uber H3 (hex gives way to its parts on descent, aggregates on ascent). Research (HEX-TOPOLOGY.md) found a nested half-cell scheme that fits ADR-0008 exact arithmetic; cube-sphere kept behind a `Topology` trait as oracle/fallback; sphere spikes with a go/no-go gate before the full build; Nanite ideas borrowed (tight bounds, error rule, culling) but no cluster DAG yet. Decided 2026-11-25 after the Phase 7 flight found detail not streaming in | ✓ Good (v1.0, ADR-0011) |
| Rendering API, ECS, spatial backends, planet LOD/transition technique | Decided via prototypes/benchmarks: wgpu (ADR-0005), hash-grid spatial (ADR-0006), no ECS in P0 (ADR-0017), geomorphing (ADR-0014) | ✓ Good (v1.0) |
| Hex planet topology: aperture-4 half-cell hierarchy on an icosahedron, gnomonic exact-op projection, `Topology` trait with cube-sphere as oracle (ADR-0011) | Hex cells give the visible LOD unit without H3 gaps or trigonometry in the deterministic path; proven by coverage and seam proofs and the full scripted descent at real defaults | ✓ Good (v1.0) |
| Atmosphere as limb-ring mesh, not shader pass (ADR-0019) | Cheapest minimal atmosphere meeting PLAN-08; user override accepted | ⚠️ Revisit if a true scattering pass is wanted |
| egui debug overlay confined to debug-ui and an optional SDK feature (ADR-0018) | Diagnostics without egui leaking into core | ✓ Good (v1.0) |

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
*Last updated: 2026-11-27 after starting milestone v1.1*