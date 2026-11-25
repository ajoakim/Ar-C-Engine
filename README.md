# Ar-c_engine

A planetary-scale 3D engine with a strictly layered core and pluggable extensions (conventional 3D, planetary).

## Building

Requires [rustup](https://rustup.rs). The repo pins Rust 1.99.0 via `rust-toolchain.toml`; rustup installs and selects it automatically (or run `rustup toolchain install`).

```sh
cargo build --workspace
cargo test --workspace
cargo xtask check-deps
cargo deny check
```

## Architecture

- [Dependency direction](docs/architecture/dependency-direction.md)
- [Architecture Decision Records](docs/architecture/adr/README.md)

## License

Dual-licensed under MIT OR Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
