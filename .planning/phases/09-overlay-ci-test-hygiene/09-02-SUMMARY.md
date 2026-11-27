---
phase: 09-overlay-ci-test-hygiene
plan: 02
subsystem: planetary-terrain / planetary-app overlay
tags: [debt, overlay, budget]
requirements: [DEBT-05]
key-files:
  modified:
    - extensions/planetary/planetary-terrain/src/system.rs
    - extensions/planetary/planetary-terrain/src/lib.rs
    - extensions/planetary/planetary-terrain/tests/terrain_stream.rs
    - extensions/planetary/planetary-app/src/lib.rs
    - extensions/planetary/planetary-app/src/overlay_snapshot.rs
decisions:
  - "Overlay held-mesh cap is base_patch_budget + detail_budget_patches, single-sourced via TerrainSystem::held_mesh_cap"
metrics:
  completed: 2026-11-27
---

# Phase 9 Plan 02: Held-mesh overlay cap fix Summary

The overlay line "detail patches 2026 / 1500" compared held meshes of both classes (base + detail) against the detail-only budget; it now shows "held meshes (base+detail)" against the combined cap.

## Investigation outcome: A (budget enforced, overlay mixed quantities)

Code read in system.rs:
- `:562` base class budget = `6*((4^(base_levels+1))-1)/3 + 16` = 526 for base_levels 3; `:570-576` DETAIL_CLASS budget = `detail_budget_patches` (1500). Class assigned by level at `:950`.
- `:1262` `report.held_meshes = self.held.len()` counts all held meshes of both classes.
- overlay used `held_meshes` as numerator and `detail_budget_patches` as cap. 526 + 1500 = 2026, exactly the observed value, so the budget is enforced and the display was wrong. The existing hover test (held <= base + detail) already asserted enforcement.

## Changes
- `base_patch_budget(u8)` extracted (exported from planetary-terrain) and used by the constructor; `TerrainSystem::held_mesh_cap()` = saturating base + detail.
- Tests: `held_mesh_cap_is_base_plus_detail` (526+60); hover test bound now derived from `held_mesh_cap()` (assertion not weakened).
- Overlay: `overlay_snapshot::build` param renamed `held_mesh_cap`, label "held meshes (base+detail)"; lib.rs dispatcher `held_mesh_cap` replaces `detail_budget_patches`; new test `held_meshes_at_cap_is_not_over_cap` pins label, cap and used<=cap. Telemetry CSV columns untouched.

## Commits
- 4d343ed fix(09-02): expose held_mesh_cap (base+detail) as single-source terrain bound
- see `git log` for the overlay commit (fix(09-02): overlay shows held meshes against the combined base+detail cap)

## Verification
- `cargo test -p planetary-terrain --locked --test terrain_stream held_mesh`: 2 passed
- `cargo test -p planetary-app --locked --features debug-overlay --lib overlay_snapshot`: 3 passed
- `--test overlay_toggle`: 2 passed; clippy `-D warnings` with debug-overlay: clean

## Deviations
Added `base_patch_budget` re-export in planetary-terrain `lib.rs` (needed by the test; not listed in plan files). Otherwise none.

## Notes for orchestrator
- STATE.md / ROADMAP.md intentionally NOT edited: the orchestrator must tick 09-02 there.
- DEBT-05 not marked complete (plan 09-04 does that).

## Self-Check: PASSED
