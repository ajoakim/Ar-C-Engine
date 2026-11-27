---
phase: 07-planetary-extension-terrain-lod
verified: 2026-11-25T00:00:00Z
status: gaps_closed
score: 6/6 roadmap truths verified automatically (SC-5 visual part and demo-parameter caveat need human)
gaps:
  - id: G1
    requirement: PLAN-03, PLAN-07
    truth: "A user can fly from orbit to the surface ... with streamed terrain; refinement keeps streaming in as the camera descends"
    status: closed
    closed_by: "Phase 07.1 (2026-11-26): 07.1-02, 07.1-03 (selection fixes, measured ranges), 07.1-05 (descent and ascent suite at real defaults, max_level 16, cube), 07.1-17 (same on hex, the default topology), 07.1-18 (human flight approved with caveats C1-C4, see Update below)"
    reason: "User flight report (2026-11-25): descending into the planet in the interactive demo, the terrain detail does not render in. At default LodParams (max_level 19, max_leaves 700) node bounds are inflated by ~6 km relief so the wanted set never converges; the automated descent test only passes at reduced max_level 13."
    fix_hint: "Tight/distance-aware per-node height bounds (per-level relief amplitude), horizon + frustum culling, selection cost budget, and a descent test at the real demo parameters (no reduced level)."
  - id: G2
    requirement: PLAN-07
    truth: "A scripted orbit-to-surface descent stays within the frame-time budget without stalls"
    status: closed
    closed_by: "Phase 07.1 (2026-11-26): 07.1-01 and 07.1-DIAGNOSIS.md (evidence-based diagnosis), 07.1-03 (crash-path fixes, explicit 1024-draw budget), 07.1-05 (descent at real defaults, debug and release numbers), 07.1-27 (hex prepare speed, descent p95 10.9 -> 4.1 ms); automated gate in 07.1-18 (863 tests passed)"
    reason: "User report: `planetary_flight -- --descent` (windowed, debug build) stuttered and then crashed. Orchestrator repro: the same scripted descent headless on NullBackend at default parameters did not finish 600 frames in 280 s (per-frame CPU cost far over budget). Crash cause not yet captured; machine had ~6 GB free disk (C: 100% full) which may also contribute. Root cause must be diagnosed, not assumed."
    fix_hint: "Capture the crash message; per-frame work budget/time-slicing for selection, effective cut and reconcile; check unbounded growth (held meshes, pending requests, buffers) and wgpu limits (MAX_DRAWS_PER_FRAME 1024, buffer counts); verify in --release as well as debug."
  - id: G3
    requirement: QA-02 (partial, pulled forward), PLAN-07
    truth: "Frame rate, distance and descent height are measurable"
    status: closed
    closed_by: "Phase 07.1 (2026-11-26): 07.1-01 (telemetry module, --perf-log CSV, --bench-descent at real defaults, per-band stats), extended in 07.1-20, 07.1-24 and 07.1-27 (flicker, morph and prepare columns)"
    reason: "User request: there is no way to measure frame rate versus distance/descent height. Only per-frame counted caps exist in tests."
    fix_hint: "Telemetry: frame time/FPS (avg, p95, max), altitude above surface, distance to surface, selection stats (nodes visited, wanted/drawn leaves, held, pending, started, uploaded, evicted); console summary per second and CSV log (--perf-log); a headless --bench-descent that reports frame-time statistics per altitude band at real default parameters; usable in debug and --release."

human_verification:
  - test: "cargo run -p planetary-app --example planetary_flight, then -- --descent"
    expected: "Fly orbit to surface with controls working; no visible cracks at LOD/face edges/corners; no popping at hysteresis boundaries; near-surface refinement acceptable at default max_level 19 / max_leaves 700"
    why_human: "Windowed visual flight; not run (no interactive window launched). Result not recorded in 07-10-SUMMARY."
  - test: "Linux/macOS CI run for goldens (GOLDEN_HEIGHTS/DIRS/MESH)"
    expected: "Goldens match off Windows"
    why_human: "Goldens are Windows-computed only; hosted CI had not run at 07.1-18 (GitHub billing). Now tracked by .planning/phases/08-hardening-decisions/08-CI-EVIDENCE.md (plan 08-01); see 08-01 for the outcome if that file is not yet present"
---

# Phase 7 Verification

**Goal:** fly from orbit to the surface of a procedurally generated planet with crack-free, streamed terrain.

## Evidence run
- `cargo test -p planetary-core -p planetary-terrain --locked`: all green (incl. seam tests, `negative_control_without_skirts_fails`, `max_level_jump_detects_negative_control`, 131s descent suite 7/7, 11-test and 6-test integration suites).
- `xtask check-deps`: OK (18 packages). `xtask check-canary`: OK (conventional-3d reaches 289 packages, none planetary).
- ADR-0009 and ADR-0010 both Status: accepted.

## Truths
| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Cube-sphere, packed PatchId, adjacency reciprocity | VERIFIED | planetary-core tests pass (adjacency/reciprocity/oracle suites) |
| 2 | 2:1 balanced quadtree, SSE, hysteresis | VERIFIED (automated) | lod/select.rs, lod/check.rs `is_balanced`/`max_level_jump`; lod_balance tests incl. negative control. Visual "no popping" is human |
| 3 | No cracks at LOD/face edges/corners | VERIFIED (CPU proof) | seam.rs: skirt depth vs gaps, negative control (depth 0 fails), f32 reconstruction. Visual confirmation is human |
| 4 | Borders agree via planet-space layered generators | VERIFIED | planetary-core generator tests + goldens (Windows only) |
| 5 | Planetary camera orbit-to-surface; app uses public SDK only | VERIFIED (automated) | planetary-terrain camera tests; `planetary-app --test sdk_only` policy; interactive flight not run |
| 6 | Scripted descent within frame budget on simulated clock, counted caps | VERIFIED with caveat | 7-test descent suite passes; uses max_level 13, not 16 (see warnings) |

## Planet-agnostic core / removability
- Root Cargo.toml: planetary-core, planetary-terrain, planetary-app each on own `extensions/planetary/` line. check-deps and check-canary pass.
- grep of `crates/` for "planet": only comments ("no planetary concepts") and tests about planetary *distance* (generic precision). No dependency on planetary crates.
- engine-sdk gpu_frame hook / destroy_mesh is generic (ADR-0009).
- `StreamManager::set_deterministic` added to engine-streaming outside plan: generic (determinism toggle), used only when settle_each_frame. Accepted as a minor scope deviation (WARNING, info-level).

## Warnings (weighed honestly)
1. **Selector design limit:** descent test passes at max_level 13 because the wanted set did not converge at 16 (node bounds inflated by ~6 km relief). The interactive default is max_level 19 / max_leaves 700 (lod/select.rs), so near-surface refinement will lag with coarse fallback. This does not falsify SC-2/3/5/6 as tested, but SC-6 evidence covers only level 13, and "surface-level" terrain fidelity at default parameters is unproven. NOT classified as a blocker gap; classified as a quality limitation for the human flight to judge. Suggested fix if the flight shows visible coarse terrain: use tighter per-node height bounds (per-patch min/max measured from generator rather than global relief inflation), or lower default max_level to the converged value (~13-14) / raise max_leaves.
2. Goldens Windows-only; hosted CI never run (ADR-0010 notes this).
3. Requirements checkboxes in REQUIREMENTS.md and ROADMAP plan checkboxes still unchecked (bookkeeping to update by orchestrator).

## Human verification
See frontmatter: windowed flight (default and `--descent`) and cross-OS CI. Status is human_needed because these items exist.


## Gap found by user flight (2026-11-25)

The user ran `planetary_flight` and `planetary_flight -- --descent` (debug builds). Result: while descending, surface detail did not render in; the scripted descent stuttered and then crashed. Earlier "warnings" about the default demo parameters are therefore reclassified as goal gaps G1-G3 above. The crash message was not captured; collect it (or reproduce) as the first step of gap closure. Window run is otherwise pending re-verification after gap closure.

## Update (Phase 8, 2026-11-26)

G1, G2 and G3 are closed by Phase 07.1. The sections above are kept as history.

- **Evidence:** `07.1-05-SUMMARY.md` and `07.1-17-SUMMARY.md` (descent and ascent at real default parameters, cube and hex), `07.1-01-SUMMARY.md` and `07.1-DIAGNOSIS.md` (telemetry and crash diagnosis), `07.1-03-SUMMARY.md` (crash-path fixes), `07.1-27-SUMMARY.md` (prepare speed). `07.1-18-SUMMARY.md` records the automated gate (fmt, clippy, 863 tests passed, check-deps, check-canary, cargo deny, cross-target `cargo check` only).
- **Human flight:** approved in 07.1-18 with caveats. The user's words: "approve on all the questions I would need meshes to exist for then to be definitive." Open caveats: C1 visual approval not definitive without meshes; C2 plan 26 present-mode and title readout not exercised by the human (the run was in a different, older checkout); C3 no user-supplied numbers (bench and telemetry rest on agent-measured evidence and ADR-0016); C4 Linux/macOS goldens pending hosted CI.
- **Linux/macOS CI item:** still open here; covered by `.planning/phases/08-hardening-decisions/08-CI-EVIDENCE.md` (plan 08-01).
- The closure is therefore bounded by C1-C4, not unconditional.
