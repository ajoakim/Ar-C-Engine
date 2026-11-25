# Project Research Summary

**Project:** Ar-c Engine (general-purpose Rust 3D engine with planetary and conventional reference apps)
**Researched:** 2026-11-25 | **Confidence:** MEDIUM

## Executive Summary

A small, layered Rust engine core whose Core Value is independence from any specialized capability. Two reference apps prove it: a planetary-scale world (cube-sphere quadtree terrain, streaming, procedural generation) and a conventional 3D scene.

Established approach:
- extract-then-render frame packet, independent of geometry source
- f64 authoritative positions with camera-relative f32 rendering
- handle + residency model that never blocks the frame
- extensions implement core traits; dependency direction enforced in CI

Recommended: wgpu 30 behind a thin engine-owned render boundary, winit 0.30, glam (f32 render / f64 world). Small custom scene graph with generational IDs, no ECS in P0. Custom priority + cancellable job scheduler (threads + channels), not tokio. Planetary code only in extension crates. Start with ~8-12 coarse crates (not the HLD's 15). Build the conventional app early as a canary for core leakage.

Main risks: late precision handling; planetary leakage into core; premature abstraction / wgpu leaking through the backend boundary; cracks at cube-face seams; blocking/unbounded streaming; non-deterministic generation.

**Hard blocker:** local rustc is 1.69; the stack needs Rust >= 1.95.

## Key Findings

**Stack:** Rust >= 1.95, edition 2024, workspace pinned via `rust-toolchain.toml`; wgpu 30.0.2 (WGSL, only render-backend crate depends on it); winit 0.30.13; glam 0.34 + opaque `WorldPos`; reverse-Z with Depth32Float; slotmap; custom job/stream scheduler on std threads + crossbeam (rayon only inside jobs); serde + RON (config), postcard (cache), gltf, image; in-house seeded integer-hash noise in f64; tracing/profiling/optional tracy; egui in its own crate; proptest, insta, criterion, nextest, cargo-deny.

**Features:**
- Table stakes: workspace with enforced boundaries, lifecycle/module registry, SDK facade, window/input, replaceable render backend, render graph + extraction, scene/transforms/stable identity, camera, mesh/material/lit shading/directional light, glTF, resource residency, job system with priority and cancellation, diagnostics, tests, ADRs.
- Differentiators: large-world precision in core, streaming with budgets and coarse fallback, deterministic procedural framework, planetary topology with stable patch IDs, crack-free screen-space-error LOD, planetary camera, budget/LOD observability, conventional app runs without planetary crates. Basic atmosphere is in scope but first to cut.
- Defer: shadows, geomorphing, import pipeline (P1); geospatial ingestion, weather/day-night, physics, scripting, animation, editor, ray tracing, plugin ABI.

**Architecture:** Layers: apps, SDK facade, extensions, engine services, foundation. Central seam is the geometry-agnostic, camera-relative f32 `RenderScene` frame packet. Streaming is generic (key, priority, budget class, cancel token); LOD policy stays in the planetary extension. Generation is a pure function of stable ID + layer seed, sampled in planet space. Components: engine-math, engine-core, engine-jobs, engine-platform, engine-render (null backend), engine-world, engine-spatial, engine-assets, engine-streaming, engine-procedural, extensions/conventional-3d, extensions/planetary-*, engine-sdk.

**Top pitfalls and prevention:**
1. Late precision: build `WorldPos` and camera-relative extraction first; test at 6.4e6 m.
2. Planetary leakage: CI dependency-direction check, generic core keys, conventional app built without planetary crates.
3. Premature abstraction / backend leakage: trait needs two implementations or a test double; engine-owned handles; null backend.
4. Cracks / cross-face adjacency: adjacency table with reciprocity property tests, 2:1 balance, crack strategy by spike + ADR (skirts first), hysteresis, seam test cameras.
5. Blocking/unbounded streaming: generation-tagged state machine, cancellation, per-frame upload caps, pinned parent fallback, hard budgets, scripted descent frame-time test.
6. Non-determinism: versioned fixed hasher (not `DefaultHasher`), position-pure noise, golden-hash tests.

## Implications for Roadmap

Suggested phases (8); 5 and 6 can run in parallel:

1. **Workspace, Toolchain, Guardrails** — Rust 1.95+ pin, coarse crates, CI (Windows + Linux), dependency-direction check, ADR template, conventions ADR (meters, axes, handedness, depth).
2. **Foundation (math, core, jobs, diagnostics)** — `WorldPos`, frames, rebasing, lifecycle + Module registry, priority/cancellable scheduler, tracing, criterion scaffolding.
3. **Platform, Render Backend, Vertical Slice** — winit, wgpu behind engine-owned handles, null backend, render graph skeleton, RenderScene extraction, triangle through the SDK.
4. **World, Spatial, Assets** — scene graph with generational `EntityId`, one simple spatial backend, `Handle<T>`, residency state machine.
5. **Conventional Reference App (canary)** — mesh, material, light, fly camera, glTF; basic-scene example using only the SDK; CI job with planetary crates absent.
6. **Streaming and Procedural Framework** — stream scheduler with budgets and pinned fallback, generator contract, versioned seeding, golden-hash tests.
7. **Planetary Core and Terrain LOD** — cube-sphere, packed `PatchId`, adjacency tests, quadtree, SSE LOD with hysteresis and 2:1 balance, crack strategy, parent fallback, layered planet-space generation, terrain pass, planetary camera, descent frame-time test.
8. **Hardening** — minimal atmosphere, LOD/budget overlays, image-diff regression on a software adapter, benchmark budgets, SDK polish, docs, ECS and spatial-backend ADR spikes.

**Ordering rationale:** precision before world/render (retrofit = rewrite); conventional app before planet work to catch leakage; generic streaming precedes planet; benchmarks/budgets before LOD; scope held to Active requirements (HLD sections 13-17 are non-goals).

**Research flags (need `/gsd-research-phase`):** Phase 3 (wgpu 30 API, software-adapter CI, DX12 vs Vulkan); Phase 6 (scheduler/upload throttling, determinism level); Phase 7 (LOD crack spike, reverse-Z infinite far plane, f64 vs i64-cell precision); Phase 8 (egui/wgpu alignment, ECS ADR, spatial benchmark). Standard patterns: Phases 1, 2, 4, 5.

## Confidence

| Area | Confidence | Notes |
|---|---|---|
| Stack | MEDIUM | Versions from crates.io are HIGH; API details from training knowledge |
| Features | MEDIUM | Taxonomy from HLD; competitor claims LOW-MEDIUM |
| Architecture | MEDIUM | Patterns established; LOD technique unproven until prototyped |
| Pitfalls | MEDIUM | Domain practice established; ecosystem claims unverified |

**Overall: MEDIUM**

### Gaps to Address
- Local rustc 1.69 must be upgraded to >= 1.95 (blocks Phase 1).
- egui 0.37 / wgpu 30 alignment unverified; fall back to minimal in-house overlay if conflicting.
- wgpu 30 API details need docs lookup in Phase 3.
- bincode/rand stream stability and `noise` crate status unverified (mitigated by postcard, custom noise, golden tests).
- Precision representation (f64 vs i64 cell + offset): keep `WorldPos` opaque, prototype.
- Float determinism level (same-machine vs cross-platform): decide by ADR in Phase 6.
- Crate layout conflict: STACK.md has separate `engine-render-backend`; ARCHITECTURE.md folds it into `engine-render`. Prefer separate crate if cargo-deny can enforce wgpu isolation; decide in Phase 1.
- Software-adapter GPU CI unverified on all platforms.
- Competitor comparisons unverified; should not drive scope.

## Sources
crates.io API queried 2026-11-25 (HIGH); HLD.md and PROJECT.md (HIGH for intent); established engine/terrain practice and Rust ecosystem knowledge (MEDIUM); competitor, rend3, bincode, rand status (LOW).
