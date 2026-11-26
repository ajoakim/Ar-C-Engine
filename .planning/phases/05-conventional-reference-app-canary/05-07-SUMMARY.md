---
phase: 05-conventional-reference-app-canary
plan: 07
subsystem: conventional-3d / glTF import
tags: [gltf, image, security, untrusted-input, REND-05]
requires: [05-06, 05-01]
provides:
  - "import_gltf_path/_slice(_with) -> ImportedModel with ImportLimits"
  - "decode_image bounded PNG/JPEG decode"
  - "procedural cube/plane/checker fallback"
  - "assets/textured_cube.glb fixture (2276 bytes)"
affects: [05-09]
tech-stack:
  added: []
  patterns: ["URI confinement before gltf touches URIs", "own bounded image decode (no gltf::import)", "iterative DFS with visited set"]
key-files:
  created:
    - extensions/conventional-3d/src/error.rs
    - extensions/conventional-3d/src/import.rs
    - extensions/conventional-3d/src/procedural.rs
    - extensions/conventional-3d/tests/support/mod.rs
    - extensions/conventional-3d/tests/gltf_import.rs
    - extensions/conventional-3d/assets/textured_cube.glb
  modified:
    - extensions/conventional-3d/src/lib.rs
    - .gitattributes
decisions:
  - "Children are pushed then the tail reversed (gltf Nodes iterator is not DoubleEnded) to keep document pre-order"
metrics:
  tasks: 3
  completed: 2026-11-25
---

# Phase 5 Plan 07: glTF import Summary

Hardened glTF 2.0 importer in conventional-3d producing engine-shaped CPU data (meshes, base-color materials, RGBA8 textures, pre-order node hierarchy), with a procedural fallback and a 2276-byte checked-in textured cube.

## Commits
- 51fa9e3 feat: error type, data model, procedural fallback, URI policy, bounded image decode
- 1946fa7 feat: document import (buffers, meshes, materials, iterative traversal)
- accdd01 test: GLB builders, fixture, 14 integration tests (+ .gitattributes binary rules)

## Hardening
- `check_uri` rejects `%` (avoids gltf 1.4.1 urlencoding unwrap panic), `..`/`.`/empty segments, leading `/`, `:`, `\`, `?`, `#`; slice import rejects all external URIs.
- File-size cap (including external buffers/images) before reading; pre-decode dimension check, `image::Limits` (width/height/alloc), PNG/JPEG and 8-bit color types only.
- Per-mesh vertex/index caps checked before collecting; index range, multiple-of-3, attribute length, finiteness checks.
- Iterative DFS, visited set (cycle), depth cap 64, node cap 10,000.
- No unwrap/expect outside tests; no gltf::import/import_images.

## Deviations from Plan
None of substance. Clippy-driven tweaks (`is_multiple_of`, `as_chunks`) and the reversed-tail child push were needed to compile/lint.

## Verification
- `cargo test -p conventional-3d --locked`: 7 unit + 14 integration (1 ignored regenerator) pass
- clippy `-D warnings`, `cargo fmt --all --check`: clean
- `cargo xtask check-deps`: OK; `check-canary`: OK (287 packages, none planetary); `cargo deny check`: advisories/bans/licenses/sources ok

## Known Stubs
None.

## Self-Check: PASSED
