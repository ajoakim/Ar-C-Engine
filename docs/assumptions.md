# Assumptions

Assumptions the engine and its reference applications rely on. Each names the ADR, file or document it comes from, so it can be re-checked when it changes. Known gaps and defects are in [limitations.md](limitations.md); the structure is in [architecture-overview.md](architecture-overview.md).

## Assumptions

### Toolchain and platform

- **Rust.** The repository pins Rust 1.99.0 (`rust-toolchain.toml`, with `rustfmt` and `clippy`). The workspace uses edition 2024, and licences are `MIT OR Apache-2.0` (`Cargo.toml`, ADR-0002). Rust is the only implementation language for the engine core, SDK and extensions.
- **wgpu 30.0.2 and winit 0.30.13.** Pinned in the root `[workspace.dependencies]`. `wgpu` is confined to `engine-render-wgpu` and `winit` to `engine-platform` (ADR-0005, `dependency-direction.md`). wgpu changes its API often, so upgrades are expected to touch one crate (ADR-0005 consequences).
- **Target platforms.** Windows, Linux and macOS are built and tested by the hosted CI matrix (ADR-0003, `.planning/phases/08-hardening-decisions/08-CI-EVIDENCE.md`). Local development has been Windows-first, on one machine with an Intel Arc B580 (ADR-0016 measurements) and an AMD Ryzen 9 5950X (benchmark results).
- **A GPU is needed for windowed runs.** Headless runs use the null backend; image tests use a software adapter (see below).

### Units, precision and frames

- **Conventions.** SI units, right-handed Y-up with -Z forward, reversed-Z `Depth32Float` with an infinite far plane (ADR-0001).
- **Precision.** World positions are f64 (`WorldPos`, tagged with a coordinate frame); render transforms are camera-relative f32, subtracted in f64 first (ADR-0001, ADR-0004). At a planet radius of about 6.4e6 m an f32 has a 0.5 m step, which is why the world position is not f32.
- **Rebasing is explicit.** Positions must be registered in a `PositionStore` to be rebased, and the application decides when to rebase. Tests use 1e-9 m (round trip) and 1e-8 m (rebasing) tolerances, and repeated rebases accumulate bounded drift (ADR-0004).
- **Single-planet scale.** The planetary extension assumes one planet per `TerrainConfig`. The default radius is 6,371,000 m (`TerrainParams::default` in `planetary-core/src/terrain.rs`) with generator version 2 (the ridge layer of ADR-0015), a sea level of -200 m and a documented maximum relief of about 6 km used for node bounds (ADR-0010, ADR-0012). Multi-body or non-spherical bodies are not supported or tested.
- **Interactive flight starts from an altitude above the nominal radius** (`AppSettings.start_altitude`); scripted descents are defined relative to the configured radius (`DescentScript::orbit_to_surface`).

### Determinism contract

- **Procedural output is a pure function of the seed, layer and coordinates.** Hashed paths use only `+ - * /`, `floor`, `sqrt` and exact integer-to-float conversion; transcendental functions, `DefaultHasher` and unordered collections are banned in ordered paths, and a test greps the source for banned identifiers (ADR-0008, item 8).
- **Bit-exact for the framework.** Golden hashes are taken over `f64::to_bits` bytes after canonicalising `-0.0`. Results must not depend on thread count; the goldens are checked at 1 and 4 threads (ADR-0008, `planetary-core/tests/generator.rs`).
- **Cross-platform equality is evidenced for the CPU goldens only.** Hosted run 45947523739 passed the height, hex direction, hex mesh and cube-select goldens on Linux, macOS and Windows (`08-CI-EVIDENCE.md`). The older "unverified until hosted CI" note in ADR-0008 is closed by that run for those goldens. GPU image goldens are separate (see below).
- **Cache formats are versioned.** Cell payload v4 carries the morph attribute; v3 payloads are rejected and no persisted cache exists in the tree (ADR-0014).

### Streaming and rendering

- **Rendering never blocks on generation.** Work is asynchronous with priorities, cancellation, budgets and a coarse-ancestor fallback; the blocking `settle()` is a test and shutdown helper only (ADR-0008, `--headless` awaits workers each frame for determinism).
- **The windowed frame budget is 16.7 ms.** The headless benches assert p95 per altitude band against that budget (`--budget-ms`, default 16.7). They measure CPU only; GPU time is not included (planetary-app README).
- **Hex is the default topology and cube-sphere is the fallback.** The hex lattice definition (not the spherical Voronoi) is the source of truth for cells, and the maximum level jump of 1 between neighbours is an empirical property that code must preserve (ADR-0011).
- **Present mode defaults to Fifo** with a frame latency of 2; the default changes only if Mailbox is supported and measurably better (ADR-0016).
- **The distance haze is a stand-in.** It and the sky colour are an application policy and a first-guess set of values to be judged by eye (ADR-0015).

### Testing

- **Image goldens come from software adapters.** WARP on Windows and lavapipe on Linux; there is no software Metal adapter for macOS. Tolerances are 2/255 per channel and a 0.5 percent bad-pixel budget, chosen as headroom, not from observed noise (ADR-0005 amendment, plan 08-03 summary).
- **The Windows goldens were generated on local Windows 11 WARP.** Hosted runners may differ (ADR-0005 amendment).
- **Benchmark numbers are single-machine and informational.** No timing gates exist; `--quick` sampling on a desktop that was not idle (ADR-0017, `08-BENCH-RESULTS.md`).

### Scope exclusions

The engine is a general-purpose core, and specialised capabilities are kept out of it (`CLAUDE.md`, ADR-0002, ADR-0007):

- No ECS (ADR-0017), physics engine, scripting, editor, plugin marketplace, geospatial data ingestion, weather or day-night cycle, or ray tracing in P0.
- No planet-specific concept in the core crates; planetary code lives only under `extensions/planetary`.
- The spatial index is the uniform hash grid with a linear-scan oracle; other backends (rstar, kd-tree) were not benchmarked (ADR-0006).
- The debug overlay is read-only (ADR-0018).
