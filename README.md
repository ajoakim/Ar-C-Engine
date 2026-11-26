# Ar-c_engine

A general-purpose 3D engine in Rust with a strictly layered core and pluggable extensions. The core stays independent of any specialised capability; two reference apps exercise it:

- **Conventional 3D** (`extensions/conventional-3d`): lit meshes, materials, glTF, fly camera. Builds and runs with every planetary crate removed (enforced in CI).
- **Planetary** (`extensions/planetary`): a streamed, procedurally generated planet you can fly down to from orbit, with level-of-detail terrain on a hex-grid topology inspired by Uber's H3.

> **Status: early development.** Phases 1-6 are complete. Phase 7.1 (hex-grid planet, descent performance) is in progress. APIs will change. Hosted CI for Linux, macOS and Windows is configured, but local verification has been Windows-first. See [.planning/ROADMAP.md](.planning/ROADMAP.md).

## Highlights

- Precision: f64 world positions, camera-relative f32 rendering, origin rebasing.
- Rendering behind an engine-owned backend boundary (wgpu 30 first, a null backend for headless tests), reverse-Z depth.
- Asynchronous streaming with priorities, cancellation, budgets and coarse-parent fallback; rendering never blocks on generation.
- Deterministic procedural generation (exact-arithmetic, golden-hash tested; see ADR-0008).
- Planet terrain: screen-space-error LOD with horizon and frustum culling, crack-free borders (skirts plus shared-border sampling), generic `Topology` trait with hex-grid and cube-sphere implementations.
- Telemetry: frame time, altitude, selection and streaming stats, CSV log, and a headless descent benchmark.

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
```

The full list of flags and the CSV columns is in [extensions/planetary/planetary-app/README.md](extensions/planetary/planetary-app/README.md).

## Architecture

- [Dependency direction](docs/architecture/dependency-direction.md)
- [Architecture Decision Records](docs/architecture/adr/README.md), including hex topology (ADR-0011), terrain LOD selection (ADR-0012) and index-range draws (ADR-0013)

The workspace is split into coarse crates (`crates/engine-*`) and extensions. The engine core never depends on an extension, and `wgpu` and `winit` are confined to their own crates, both checked by `cargo xtask check-deps` and `cargo deny`.

## Project planning

Requirements, roadmap and phase plans live in [.planning/](.planning/).

## License

Dual-licensed under MIT OR Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
