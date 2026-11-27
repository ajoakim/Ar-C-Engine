---
phase: 08-hardening-decisions
plan: 04
subsystem: testing, performance
tags: [criterion, benches, spatial-index, adr-0006, select-cells, mesh-build, streaming]
requires:
  - phase: 08-02
    provides: criterion workspace pin (0.8.2, default features off, cargo_bench_support)
provides:
  - criterion benches for HashGridIndex vs LinearIndex, select_cells, build_hex_cell/build_patch, streaming frames
  - 08-BENCH-RESULTS.md with measured numbers and the ADR-0006 crossover evidence
affects: [08-05 ADR-0006 amendment, 08-11 non-blocking bench CI job]
key-files:
  created:
    - crates/engine-spatial/benches/spatial.rs
    - extensions/planetary/planetary-core/benches/select.rs
    - extensions/planetary/planetary-core/benches/mesh.rs
    - extensions/planetary/planetary-app/benches/stream.rs
    - .planning/phases/08-hardening-decisions/08-BENCH-RESULTS.md
  modified:
    - crates/engine-spatial/Cargo.toml
    - extensions/planetary/planetary-core/Cargo.toml
    - extensions/planetary/planetary-app/Cargo.toml
    - Cargo.lock
requirements: [QA-01, WS-05]
metrics:
  tasks: 3
  completed: 2026-11-26
---

# Phase 8 Plan 04: Criterion benches Summary

Four criterion bench files (spatial, select, mesh, stream) with release-mode measurements recorded in 08-BENCH-RESULTS.md. The headline result for ADR-0006: the hash grid answers queries in roughly constant time (flat from N=1e3 to 1e5) and beats the linear scan from about N=300 (region, radius) / N=1000 (frustum) upward, but its updates are 8-24x slower than the linear scan.

## Commits
- 7597f48 spatial bench, engine-spatial dev-dependency, Cargo.lock regenerated with a plain `cargo build`, spatial results
- e7ea403 select and mesh benches in planetary-core, results
- 2104e08 streaming frame bench in planetary-app, results

## Headline numbers
(AMD Ryzen 9 5950X, Windows 11, rustc 1.99.0, median of 3 `--quick` runs; full tables and ranges in 08-BENCH-RESULTS.md)

- Spatial, 16 queries per cell, N = 1e5: region 7.5 us vs 9.8 ms linear; radius 15.0 us vs 7.1 ms; frustum 87 us vs 9.3 ms. At N = 1e3: region 4.9 vs 70 us; radius 11.7 vs 57 us; frustum 54 vs 92 us.
- Crossover: region is equal at N=100 (1.05x) and hash grid 3.7x ahead at 300; radius linear is 1.8x faster at 100 and hash grid 1.6x ahead at 300; frustum linear is 2.8x faster at 300 and hash grid 1.7x ahead at 1000.
- Updates (10% of entities moved and back): hash grid 8x (N=1e5, 7.7 ms vs 0.91 ms) to 24x (N=100) slower than linear.
- select_cells: hex orbit 98 us, hex 1 km 1.9 ms; cube orbit 28 us, cube 1 km 2.6 ms. Cold and warm previous-split are the same within noise.
- Mesh build: hexagon 2.84 ms, pentagon 2.00 ms (sector side 16); cube patch 1.56-1.60 ms (32 per edge), single thread.
- Streaming, per frame over 30-frame windows: hex 9.8 ms (orbit start) / 13.1 ms (low altitude); cube 8.2 / 9.7 ms. Consistent with the ~14 ms hex figure in ADR-0015. Run-to-run spread under 2 percent.

## Deviations from Plan

### Auto-fixed / adjusted

**1. [Rule 3 - Blocking] `cargo bench ... --benches` still runs lib unittests**
- The plan's verify command `cargo bench -p <crate> --benches ... -- --noplot --quick` (and the `--workspace --benches` form) fails here with `Unrecognized option: 'noplot'` because the lib unittest target is also selected. Benches were run with `--bench <name>` per target instead; `cargo bench --workspace --bench spatial --bench select --bench mesh --bench stream --locked -- --noplot --quick` was verified to run all four. `cargo bench --workspace --benches --locked -- --noplot --quick` therefore does NOT work in this repo; plan 11 (bench CI job) must use `--bench <name>` (or set `bench = false` on lib/bin targets).

**2. [Scope] Extra sizes in the spatial bench**
- N = 100 and 300 were added to the plan's 1e3/1e4/1e5 so the crossover is measurable (at 1e3 the hash grid already wins every query type).

**3. Streaming bench shape**
- The plan suggested a scripted descent run end to end. Each sample instead builds a fresh app, warms it untimed to frame 0 or 400 of the 480-frame script, and times 30 frames, so the per-frame cost is reported for two representative altitude bands. `PlanetaryApp::new` already uses a deterministic `SimClock`; `with_clock` was not needed.

## Checks
- Spatial/select/mesh/stream each ran 3 times to completion (`cargo bench -p <crate> --bench <name> --locked -- --noplot --quick`).
- `cargo clippy --workspace --all-targets --locked -- -D warnings` clean; `cargo fmt --check` clean.
- `cargo run -p xtask -- check-deps` OK (18 packages); `check-canary` OK (310 packages, none planetary: the count rose from 289 because the canary graph includes dev dependencies and criterion is now a dev-dependency; `cargo tree --workspace -e normal,build -i criterion` prints nothing, so criterion is in no normal or build graph).
- `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok (duplicate-crate warnings only).
- docs_policy 13/13; `cargo test --workspace --locked` exit 0 (no failures, no flaky re-runs needed).
- Cargo.lock regenerated by plain `cargo build` (188 lines for criterion, then one line each for the planetary-core and planetary-app dev-dependency edges); `--locked` bench and clippy runs pass.

## Not run
- Nothing pushed. rstar/kd-tree backends not benchmarked (out of plan scope). Hosted Linux/macOS bench runs not done (plan 11). No timing gates exist.
- `state.advance-plan` and `requirements.mark-complete` were not run; STATE.md hand-edited. QA-01 and WS-05 left unmarked because later plans share them.

## Self-Check: PASSED
