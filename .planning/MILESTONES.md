# Milestones

## v1.0 Engine Prototype (Shipped: 2026-11-27)

**Delivered:** A modular Rust 3D engine with an enforced core/extension boundary, validated by a conventional reference app and a planetary-scale reference app that flies from orbit to surface on a streamed, deterministic hex-grid planet.

**Phases completed:** 9 phases (1-8 plus inserted 7.1), 90 plans, 45/45 requirements

**Stats:**

- 368 commits, 573 files changed, 93,929 insertions (git range 4c2db39 `docs: initialize project` to 1e6375e)
- About 55,300 lines of Rust across 245 `.rs` files
- Timeline: 3 days (2026-11-25 to 2026-11-27)

**Key accomplishments:**

1. Workspace and guardrails first: pinned Rust 1.99 toolchain (MSRV 1.95, edition 2024), tiered crates, and automated boundary enforcement (`xtask check-deps`, `check-canary`, cargo-deny, hosted CI on Linux, Windows and macOS) with negative fixtures.
2. Engine foundation and render slice: lifecycle, modules, priority job scheduler with cancellation, diagnostics, opaque `WorldPos` with origin rebasing, an engine-owned render boundary (wgpu and null backends), camera-relative extraction proven at 6.4e6 m, and an SDK facade.
3. World, spatial and assets plus a canary: generational entities, pluggable spatial index (hash grid default), typed asset handles with a residency state machine, and a conventional lit/glTF app built on the SDK alone with all planetary crates absent.
4. Generic streaming and deterministic procedural generation: prioritised, cancellable, budgeted streaming with coarse-ancestor fallback, and exact-arithmetic seeded generator layers with golden hashes identical across thread counts and OSes.
5. Planetary extension and descent performance: crack-free streamed terrain LOD from orbit to surface; Phase 7.1 added an H3-inspired aperture-4 hex topology behind a `Topology` trait, measured descent telemetry, geomorphing and a fixed crash/stutter at real parameters.
6. Hardening: egui debug overlay (F3), limb-ring atmosphere (ADR-0019), software-adapter image-diff and criterion benches in CI, architecture/assumptions/limitations docs, and ADRs 0017-0019 closing the open technical decisions.

**Known gaps / tech debt accepted at close (see v1.0-MILESTONE-AUDIT.md, status tech_debt):**

- Open review items IN-01 (hard-coded CI toolchain), IN-02 (overlay ignores format/scale changes), IN-03 (image-diff tolerance)
- 03-VALIDATION.md draft; Nyquist validation partial for phases 01-05
- No macOS image goldens; bench CI job non-blocking
- Caveats C1, C3, C4 on the 07.1 flight/benchmark evidence
- Post-Phase-8 visual feedback todo still pending (LOD detail pop, flat terrain, flight DOF)

Known deferred items at close: 1 (the visual feedback todo, kept in `.planning/todos/pending/`).

**Archives:** [v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md), [v1.0-REQUIREMENTS.md](milestones/v1.0-REQUIREMENTS.md), [v1.0-MILESTONE-AUDIT.md](v1.0-MILESTONE-AUDIT.md)

---
