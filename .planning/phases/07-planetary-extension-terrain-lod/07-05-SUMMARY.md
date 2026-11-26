---
phase: 07-planetary-extension-terrain-lod
plan: 05
subsystem: planetary-core
tags: [mesh, skirts, seam, payload, adr-0010]
requires: [07-03, 07-04]
provides: [mesh builder, validated payload codec, CPU seam proofs]
affects: [07-07, 07-10]
key-files:
  created:
    - extensions/planetary/planetary-core/src/mesh.rs
    - extensions/planetary/planetary-core/src/payload.rs
    - extensions/planetary/planetary-core/tests/mesh.rs
    - extensions/planetary/planetary-core/tests/seam.rs
  modified:
    - extensions/planetary/planetary-core/src/lib.rs
    - docs/architecture/adr/0010-terrain-lod-skirts-and-shared-border-sampling.md
decisions:
  - "kappa kept at 0.25 (measured ratio would permit 1/32; not tuned to keep one default for downstream plans)"
metrics:
  tasks: 2
  completed: 2026-11-25
---

# Phase 7 Plan 05: Crack-free patch mesh, payload codec and seam proofs Summary

Patch mesh builder (per-patch f64 origin, f32 offsets, shared grid + 4 skirts topology, face-independent border normals), a validated fuzz-safe binary payload, and CPU seam proofs recorded in ADR-0010.

## Commits
- 72c5b0f feat(07-05): mesh builder, skirts, border normals, payload codec
- 63e52a0 test(07-05): seam proofs with negative control; ADR-0010 evidence

## Evidence (from `cargo test --test seam -- --nocapture`)
- Same-level border pairs: 73656 checked, all bit-identical in f64.
- Cube edges 12/12 and corners 8/8 bit-identical.
- L/L+1 junctions: 4272 junctions, 34176 odd vertices over 30 reference views; max gap / min skirt depth = 0.035.
- Negative control (depth 0): 34176/34176 violations, so the check can fail.
- f32 reconstruction: max error 0.31 of bound, bound < skirt depth.
- Measured deviation/cell: 0.0025 (L2), 0.0011 (L6), 0.0014 (L10), 0.00058 (L14), 2.7e-5 (L18), 1.7e-5 (L19); all <= eps(L).

## Deviations from Plan
None of substance. `build_patch` uses `patch_samples` heights (falls back to `sample` if lengths mismatch). The negative-control and mixed-level tests share one junction collector. `border_normal_f64` is additionally public in `mesh` (not re-exported at crate root).

## Notes
- GOLDEN_MESH (0x51f4dccc04dbd5bd) is Windows-computed; cross-platform CI confirmation pending.
- banned_ops.rs untouched; BORDER-CRITICAL markers in mesh.rs wrap skirt_depth, vertex_position_f64, border_positions_f64, border normal helpers.
- ADR-0010 keeps Status: proposed and the single capitalized "Pending: scripted descent evidence from 07-10." line.
- Gates: `cargo test -p planetary-core --locked` pass, clippy `-D warnings` clean, `cargo fmt -p planetary-core -- --check` clean, xtask docs_policy pass.

## Self-Check: PASSED
