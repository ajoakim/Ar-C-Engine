# Ar-c_engine

A general-purpose 3D engine in Rust with a strictly layered core and pluggable extensions. The core stays independent of any specialised capability; two reference apps exercise it:

- **Conventional 3D** (`extensions/conventional-3d`): lit meshes, materials, glTF, fly camera. Builds and runs with every planetary crate removed (enforced in CI).
- **Planetary** (`extensions/planetary`): a streamed, procedurally generated planet you can fly down to from orbit, with level-of-detail terrain on a hex-grid topology inspired by Uber's H3.

> **Status: early development.** Phases 1-7 and 7.1 (hex-grid planet, descent performance) are complete, with caveats recorded in the phase validation. Phase 8 (hardening: F3 debug overlay, atmosphere limb ring, CI image-diff tests and benchmarks, docs and decision ADRs) is complete, with caveats recorded in `.planning/phases/08-hardening-decisions/08-VALIDATION.md`; post-flight visual feedback is tracked as a follow-up todo. The v1.0 milestone audit found all 45 requirements satisfied, with remaining tech debt (review warnings, validation paperwork) listed in `.planning/v1.0-MILESTONE-AUDIT.md`. APIs will change. Hosted CI for Linux, macOS and Windows is configured, but local verification has been Windows-first. See [.planning/ROADMAP.md](.planning/ROADMAP.md).

## Highlights

- Precision: f64 world positions, camera-relative f32 rendering, origin rebasing.
- Rendering behind an engine-owned backend boundary (wgpu 30 first, a null backend for headless tests), reverse-Z depth.
- Asynchronous streaming with priorities, cancellation, budgets and coarse-parent fallback; rendering never blocks on generation.
- Deterministic procedural generation (exact-arithmetic, golden-hash tested; see ADR-0008).
- Planet terrain: screen-space-error LOD with horizon and frustum culling, crack-free borders (skirts plus shared-border sampling), generic `Topology` trait with hex-grid and cube-sphere implementations.
- Geomorphing: a per-vertex morph target and per-piece factor make level-of-detail handovers smooth (ADR-0014).
- Scale and relief: mid-scale ridge terrain (generator v2), slope-based rock colouring, relief lighting and distance haze (ADR-0015).
- Present-mode control and a live window-title readout of altitude, speed, level and frame time (ADR-0016).
- Debug overlay: press F3 (build with `--features debug-overlay`) for the LOD histogram, selected/drawn counts, streaming queue, memory and upload budgets and frame/prepare times (ADR-0018).
- Minimal atmosphere: a sun-tinted sky plus an opaque limb-ring mesh around the planet, on by default; `--no-atmosphere` turns it off (ADR-0019; a ring mesh rather than a shader pass).
- Regression and performance tests: image-diff goldens rendered on software adapters in CI, and criterion benches (non-blocking CI job).
- Telemetry: frame time, altitude, selection and streaming stats, per-scope prepare timings, morph and handover statistics, CSV log, and headless descent and streaming benchmarks.

## Building

Requires [rustup](https://rustup.rs). The repo pins Rust 1.99.0 via `rust-toolchain.toml`; rustup installs and selects it automatically.

```sh
cargo build --workspace
cargo test --workspace
cargo xtask check-deps      # layer / dependency-direction rules
cargo xtask check-canary    # conventional app must not reach planetary crates
cargo deny check            # licenses, bans, wrappers
```

Release builds are strongly recommended for anything you run: debug builds are several times slower. Allow a few GB of free disk for `target/`.

## Running

```sh
# Conventional scene
cargo run --release -p conventional-3d --example basic_scene

# Planet: free flight (WASD/arrows, E/Space up, Q/Ctrl down, Shift boost, right mouse to look)
cargo run --release -p planetary-app --example planetary_flight

# Scripted orbit-to-surface descent, with a per-frame CSV log
cargo run --release -p planetary-app --example planetary_flight -- --descent --perf-log flight.csv

# Headless descent benchmark with per-altitude-band frame-time statistics
cargo run --release -p planetary-app --example planetary_flight -- --bench-descent

# Debug overlay (F3) and atmosphere flag
cargo run --release -p planetary-app --example planetary_flight --features debug-overlay
cargo run --release -p planetary-app --example planetary_flight -- --no-atmosphere

# Image-diff regression tests (software adapter) and benchmarks
cargo test -p engine-render-wgpu --locked --test image_diff -- --nocapture
cargo bench --workspace --bench spatial --bench select --bench mesh --bench stream --bench ecs --locked -- --noplot --quick

# Choose the topology, present mode and haze (Immediate is a diagnostic mode and can tear)
cargo run --release -p planetary-app --example planetary_flight -- --descent --fast --topology cube --present-mode immediate --no-haze
```

The full list of flags and the CSV columns is in [extensions/planetary/planetary-app/README.md](extensions/planetary/planetary-app/README.md).

## Documentation

- [Architecture overview](docs/architecture-overview.md): layers, crates, extension model, decision index
- [SDK usage guide](docs/sdk-usage.md): building and running the examples, the `App` API, present mode, the F3 overlay, tests and benchmarks
- [Assumptions](docs/assumptions.md) and [limitations](docs/limitations.md)
- [Architecture Decision Records](docs/architecture/adr/README.md)

## Architecture

- [Dependency direction](docs/architecture/dependency-direction.md)
- [Architecture Decision Records](docs/architecture/adr/README.md), including hex topology (ADR-0011), terrain LOD selection (ADR-0012), index-range draws (ADR-0013), vertex-morph geomorphing (ADR-0014), scale, contrast and haze (ADR-0015) and present mode and window readout (ADR-0016)

The workspace is split into coarse crates (`crates/engine-*`) and extensions. The engine core never depends on an extension, and `wgpu` and `winit` are confined to their own crates, both checked by `cargo xtask check-deps` and `cargo deny`.

## Project planning

Requirements, roadmap and phase plans live in [.planning/](.planning/).

## License

Dual-licensed under MIT OR Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
