# ADR-0003: Dependency-Direction Enforcement and CI

- Status: accepted
- Date: 2026-11-25
- Requirement/Phase: WS-03 / Phase 1

## Context

Documented dependency direction drifts unless it is checked mechanically. We need failures to be loud, local and also run in CI.

## Decision

Two layers of enforcement:

1. `cargo xtask check-deps`: an in-repo checker using the `cargo_metadata` crate. It checks the declared dependencies of every workspace package against the layer table, rules R1-R6 (see `docs/architecture/dependency-direction.md`). Negative fixture tests under `xtask/fixtures/` must fail the checker.
2. `cargo-deny` (`deny.toml`): transitive wgpu/winit bans via `wrappers`, a license allowlist and RustSec advisories.

CI runs on GitHub Actions: an `ubuntu-latest` and `windows-latest` matrix runs fmt, clippy `-D warnings`, tests, `cargo xtask check-deps`, and a negative step asserting the checker fails on a fixture. A separate ubuntu job runs `cargo deny check`.

## Alternatives Considered

- cargo-deny only: workspace-internal direction is weakly testable.
- Grep scripts over Cargo.toml: miss renames and dependency kinds.
- cargo-hakari: not a direction checker.

## Consequences

- The layer table is a second source of truth that must be updated when crates are added; the `unlisted-crate` rule guards this.
- cargo-deny wgpu/winit rules are dormant until Phase 3 adds those crates.

## Evidence

- `.planning/phases/01-workspace-toolchain-guardrails/01-RESEARCH.md` (enforcement pattern, cargo-deny configuration, CI).
