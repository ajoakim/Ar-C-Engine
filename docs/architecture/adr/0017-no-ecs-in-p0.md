# ADR-0017: No ECS in P0 (engine_world::World Stays)

- Status: accepted
- Date: 2026-11-26
- Requirement/Phase: WS-05 (ECS decision) / Phase 8

## Context

PROJECT.md and the HLD forbid committing to an ECS in P0 "until proven by benchmark". The scene layer is `engine_world::World`, a small slotmap-handle world with a transform hierarchy, coordinate frames and dirty-tracked world transforms (ADR-0001, ADR-0004). WS-05 asks for the ECS question to be settled with a measurement instead of an opinion. The question is whether the engine would gain enough from an ECS at the entity counts the P0 reference applications can plausibly reach to justify the added model, scheduler and dependency.

## Decision

No ECS in P0. `engine_world::World` (generational `slotmap` handles, f64 transforms in explicit frames) stays the scene layer. No ECS crate appears in any engine crate manifest. `hecs` is pinned only in the root `[workspace.dependencies]` and used only by the tooling crate `spikes/ecs-bench`, which is outside `crates/` and `extensions/` and therefore outside the tier rules (`docs/architecture/dependency-direction.md`).

The measurements below show that hecs is faster in every case, in bulk sweeps by a wide margin, but that the engine World's absolute cost at 1e4 entities is below half a millisecond per case and at 1e5 entities a few milliseconds. That cost is acceptable for P0, and the hierarchy and frame semantics the engine needs do not exist in hecs and would have to be rebuilt on top of it.

## Alternatives Considered

- hecs 0.11.2 (benchmarked in `spikes/ecs-bench`): minimal and standalone with no scheduler. It is the lowest-lock-in ECS and the likely first candidate at the revisit trigger. Rejected for P0 because it has no hierarchy, no frame-aware positions and no dirty-tracked world transforms, which would have to be rebuilt, and because it adds a second entity model next to `EntityId`.
- bevy_ecs 0.20.1 (not benchmarked): it brings its own schedule, change-detection and app model and a fast release cadence, which conflicts with the engine-owned architecture (CLAUDE.md "Core Value" and "Scope discipline"). It was not measured because that integration cost, not raw speed, is the reason it is out; benchmarking it would not change the decision.
- Adopting an ECS now and moving `World` onto it: rejected, premature under the P0 scope rules (CLAUDE.md "No premature ECS") and unsupported by a measured need.

## Consequences

- Scene code keeps using `World`, `EntityId`, `Transform` and `WorldTransform`. Spatial indexing (ADR-0006) keeps its own `EntityId`-keyed backend.
- The engine pays for per-entity generational lookup and dirty marking: about 40 ns per entity for a read-modify-write sweep and about 53 ns per entity for a hierarchy update. A bulk transform sweep over `World` has no cache-friendly iteration path; if one is needed, add a bulk accessor to `World` before considering an ECS.
- `hecs` stays a tooling-only dependency. `cargo xtask check-deps` and `cargo deny` keep it out of the engine crates; adding it to an engine manifest is a policy change that needs a new ADR.
- Limitation: the spike measures transform storage and hierarchy only. Queries over several component types, system scheduling and parallel iteration were not measured.
- Revisit this decision (supersede with a new ADR) when any one of these is true: (a) a real reference workload needs more than about 4e4 to 5e4 dynamic entities updated per frame, since at the measured 40-53 ns per entity that is roughly 2 ms or 12 percent of a 16.7 ms frame; (b) a feature needs queries over several component types or system scheduling that `World` cannot express without growing into a poor ECS; (c) profiling a reference application shows `World` updates, rather than terrain preparation or rendering, in the top frame costs. In that case benchmark hecs and bevy_ecs again on the actual workload first.

## Measurements

Source: `spikes/ecs-bench/benches/ecs.rs`, results and method in `.planning/phases/08-hardening-decisions/08-BENCH-RESULTS.md` (section "ECS"). Machine: AMD Ryzen 9 5950X, 16 cores, 128 GB RAM, Windows 11 Pro; rustc 1.99.0; criterion 0.8.2, release (`bench` profile). Command: `cargo bench -p ecs-bench --bench ecs --locked -- --noplot --quick`, three runs, median of the point estimates shown. `--quick` sampling on a desktop that was not otherwise idle, so treat single-digit-percent differences as noise. Payload per entity is the engine `Transform` (48 bytes). Group ids are `slotmap_world/<case>/<N>` and `hecs/<case>/<N>`.

| case | N | slotmap_world (engine) | hecs | engine / hecs |
|------|---|------------------------|------|---------------|
| spawn | 1e4 | 1.24 ms | 747.30 us | 1.7x |
| spawn | 1e5 | 10.57 ms | 6.01 ms | 1.8x |
| update_all_locals | 1e4 | 391.35 us | 9.14 us | 42.8x |
| update_all_locals | 1e5 | 3.96 ms | 92.10 us | 43.0x |
| random_1pct | 1e4 | 4.18 us | 600 ns | 7.0x |
| random_1pct | 1e5 | 52.14 us | 7.92 us | 6.6x |
| despawn10_respawn | 1e4 | 119.43 us | 47.87 us | 2.5x |
| despawn10_respawn | 1e5 | 1.01 ms | 533.82 us | 1.9x |
| hierarchy_update | 1e4 | 468.73 us | 221.11 us | 2.1x |
| hierarchy_update | 1e5 | 5.31 ms | 2.19 ms | 2.4x |

The engine World implements hierarchy, frames and dirty tracking that hecs lacks, so the comparison is the cost of what the engine needs, not feature parity. The hecs `hierarchy_update` stores only the roots' world transforms and does not cache the children's, so it does less work than the engine's.

## Evidence

- Prototype and benchmark: `spikes/ecs-bench` (workspace tooling crate, `publish = false`), cases `spawn`, `update_all_locals`, `random_1pct`, `despawn10_respawn`, `hierarchy_update` at N = 1e4 and 1e5, numbers above.
- Dependency confinement: `rg -l hecs --glob Cargo.toml` lists only the root workspace pin and `spikes/ecs-bench/Cargo.toml`; `cargo xtask check-deps` passes with the new member classified as tooling; `cargo deny check` passes.
- Policy: `adr_0017_present` in `xtask/tests/docs_policy.rs` requires this ADR's measurement and evidence sections and the README row.
- bevy_ecs was not benchmarked, for the reason given under Alternatives Considered.
