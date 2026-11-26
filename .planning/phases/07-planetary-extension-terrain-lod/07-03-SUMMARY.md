---
phase: 07-planetary-extension-terrain-lod
plan: 03
subsystem: planetary-core
tags: [cube-sphere, equal-angle, determinism, procedural, goldens]
requires:
  - phase: 07-02
    provides: Face/PatchId/neighbor topology
provides:
  - exact-op equal-angle mapping, lattice directions, locate
  - planet-space terrain layers (base/detail/mask) and pure height_at_dir
  - banned-ops and usize scans
affects: [07-04, 07-05, 07-08, 07-09]
tech-stack:
  added: []
  patterns: [exact-op math only in border-critical paths, BORDER-CRITICAL marker regions, concat!-built scanner tokens]
key-files:
  created:
    - extensions/planetary/planetary-core/src/mapping.rs
    - extensions/planetary/planetary-core/src/terrain.rs
    - extensions/planetary/planetary-core/tests/mapping.rs
    - extensions/planetary/planetary-core/tests/generator.rs
    - extensions/planetary/planetary-core/tests/banned_ops.rs
  modified:
    - extensions/planetary/planetary-core/src/lib.rs
key-decisions:
  - "Three Generator impls written out explicitly (not via macro) to match the plan's grep acceptance check."
  - "Task 1 committed a placeholder terrain.rs (marker pair only, not in lib.rs) so the usize scan could run before Task 2."
requirements-completed: [PLAN-05]
duration: ~25 min
completed: 2026-11-25
---

# Phase 7 Plan 03: Exact-op mapping and planet-space terrain Summary

Equal-angle cube-sphere mapping using only `+ - * /` and `sqrt` (Taylor `T(s)`, exact at s = +-1) over a global integer lattice, plus three independently seeded planet-space generator layers composed by a pure `height_at_dir`. Shared patch borders and cube corners are bit-identical.

## Tasks

| Task | Commit | Result |
|------|--------|--------|
| 1. mapping + banned-ops/usize scans | 4e1a0c7 | 11 mapping tests, 2 scan tests pass |
| 2. terrain layers + goldens | 76f47bb | 8 generator tests pass, goldens pinned |

## Verification

- `cargo test -p planetary-core --locked`: all pass (2 + 6 + 2 + 8 + 11 + 9 tests across binaries; 0 failures).
- `cargo clippy -p planetary-core --all-targets --locked -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo xtask check-deps`: OK (18 workspace packages). `cargo xtask check-canary`: OK (conventional-3d reaches 289 packages, none planetary).

## Golden constants

Pinned in `tests/generator.rs`: `GOLDEN_HEIGHTS = 0xf067_ef3f_b83f_bd54`, `GOLDEN_DIRS = 0x3697_d55a_c59d_0aa2` (world seed 0x0007_5EED_2026_0001, default params, n = 8, four patches). Identical on 1 and 4 threads.

**Cross-OS caveat:** these constants were computed on Windows x86_64 only. Linux/macOS confirmation is pending hosted CI (blocked on GitHub billing). If another OS disagrees, fix the drifting operation; do not loosen the goldens.

## Scanner design

`tests/banned_ops.rs` builds every banned token with `concat!` split inside the word, so the file never contains `HashMap`, `HashSet`, `Instant`, `SystemTime` or `thread::sleep` literally (grep count 0; 29 `concat!` uses). `no_usize_in_border_critical_code` checks all of `mapping.rs`, and the `// BORDER-CRITICAL BEGIN/END` regions of `terrain.rs` (and `mesh.rs` once it exists; 07-05 must add markers there).

## Deviations from Plan

None in behavior. Minor: `impl Generator for` expanded explicitly rather than via macro (first draft used a macro; acceptance grep requires 3 literal impls). Rule 3 style change only.

## Issues Encountered

None. Git status at start: planetary-core files from 07-02 were all committed; only `.planning/config.json` and `HLD.md` untracked (left alone).

## Known Stubs

None.

## Self-Check: PASSED

Files exist (mapping.rs, terrain.rs, three test files); commits 4e1a0c7 and 76f47bb exist.
