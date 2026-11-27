# Roadmap: General-Purpose 3D Engine

## Milestones

- ✅ **v1.0 Engine Prototype** — Phases 1-8 incl. 7.1 (shipped 2026-11-27) — [archive](milestones/v1.0-ROADMAP.md)
- 🚧 **v1.1 Visual Fidelity & Flight** — Phases 9-13 (in progress)

## Phases

<details>
<summary>✅ v1.0 Engine Prototype (Phases 1-8) — SHIPPED 2026-11-27</summary>

Phases 1-8 plus inserted Phase 7.1, 90 plans, 45/45 requirements. Full details: [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md).

</details>

### 🚧 v1.1 Visual Fidelity & Flight (In Progress)

**Milestone Goal:** Make the planetary reference look and fly better (seamless LOD detail transitions, real terrain relief with lit normals, 6DOF free flight) while closing the v1.0 tech debt, without adding planetary assumptions to the engine core.

- [x] **Phase 9: Overlay, CI & Test Hygiene** - Close IN-01..03 and fix overlay count formatting and the detail-patch cap display
- [x] **Phase 10: Free-Flight 6DOF Camera** - Roll, strafe and vertical movement in the planetary example without breaking orbit-to-surface flight
- [ ] **Phase 11: Terrain Relief & Normals** - Generator v3 with hills and mountains plus height-derived normals and bump shading
- [ ] **Phase 12: LOD Detail Transition** - Remove the blur-then-sharpen pop at level changes with a blended colour/detail transition
- [ ] **Phase 13: Validation & CI Debt Closure** - 03-VALIDATION, Nyquist for phases 1-5, macOS goldens decision, bench job decision

## Phase Details

### Phase 9: Overlay, CI & Test Hygiene
**Goal**: The diagnostics overlay is accurate and robust, and CI and image-diff checks stop hiding small regressions
**Depends on**: Nothing (v1.0 shipped)
**Requirements**: DEBT-01, DEBT-02, DEBT-03, DEBT-04, DEBT-05
**Success Criteria** (what must be TRUE):
  1. Changing the version in `rust-toolchain.toml` alone is enough for CI toolchain checks to follow it (no second hard-coded version string remains)
  2. Overlay counts such as "selected" and "queue pending" display as integers, while times and ratios keep decimals
  3. The "detail patches used / cap" line is either never above the cap or is labelled so a reader knows what the two numbers mean, and a test pins this
  4. Reconfiguring the surface format rebuilds the overlay painter, and the overlay can take a window scale factor so it is legible on HiDPI
  5. An image-diff test fails on a small-feature change (a negative control of a few pixels) while still reporting max delta
**Plans**: 4 plans

Plans:
- [x] 09-01-PLAN.md — CI derives toolchain from rust-toolchain.toml; image-diff absolute bad-pixel cap + small-change negative control (DEBT-01, DEBT-03)
- [x] 09-02-PLAN.md — Held-mesh overlay line uses the true base+detail cap, pinned by tests (DEBT-05)
- [x] 09-03-PLAN.md — Integer counts, painter rebuild on format change, window scale factor (DEBT-02, DEBT-04)
- [x] 09-04-PLAN.md — Full local gate and requirement bookkeeping (DEBT-01..05)

### Phase 10: Free-Flight 6DOF Camera
**Goal**: The user can fly the planetary example freely in all six degrees of freedom, which also makes judging the later visual work easier
**Depends on**: Phase 9
**Requirements**: CAM-01, CAM-02, CAM-03, CAM-04, CAM-05, CAM-06
**Success Criteria** (what must be TRUE):
  1. The user can roll about the view axis, strafe left/right and move up/down along the camera axes in the planetary example
  2. A long scripted sequence of roll, pitch and yaw inputs leaves the camera orientation normalised and drift-free (unit test passes)
  3. The existing orbit-to-surface flight and scripted descent behave as before, with a documented toggle or default between free-flight and planet-aligned orientation
  4. `check-deps` and the sdk-only guard stay green: no planetary concept appears in any generic crate
**Plans**: 3 plans

Plans:
- [x] 10-01-PLAN.md — Z/X/F key codes; pure f64 quaternion FreeFlight controller with drift, roll-axis, camera-relative and levelling tests (CAM-01..04, CAM-06)
- [x] 10-02-PLAN.md — Integrate into planetary_flight input handling, document default and F toggle, scripted descent unchanged (CAM-01..03, CAM-05, CAM-06)
- [x] 10-03-PLAN.md — Full local gate, human flight checkpoint, requirement bookkeeping (CAM-01..06)

**UI hint**: yes

### Phase 11: Terrain Relief & Normals
**Goal**: The planet surface shows convincing hills and mountains that read under light, at a bounded generation cost
**Depends on**: Phase 10
**Requirements**: TERR-01, TERR-02, TERR-03, TERR-04, TERR-05, TERR-06
**Success Criteria** (what must be TRUE):
  1. Selecting generator v3 yields visibly more varied relief than v2 and is deterministic across runs for a given seed, while v1 and v2 still reproduce their existing goldens bit-for-bit
  2. Patch normals come from height data and agree across patch and LOD seams within the tested tolerance (no lighting seams)
  3. The terrain shader uses those normals so relief is visible under directional light, covered by an image-diff golden
  4. Measured worker time and hex descent p95 stay within the stated bounds against the v2 baseline, and rendering still never blocks on generation
  5. After a human flight confirms the relief, v3 is the app default and an ADR records layers, normals method and measured cost
**Plans**: 10 plans

Plans:
- [x] 11-01-PLAN.md — Generator-version axis on the mesh bench, `--generator` flag, v2 mesh byte golden, mesh ratio script, v2 baselines (TERR-02, TERR-05)
- [x] 11-02-PLAN.md — Generator v3 hills and mountains layers, additive, guard v3-inert equals v2 (TERR-01, TERR-02)
- [x] 11-03-PLAN.md — Relief calibration (slope/relief bands) and LodModel v3 bounds (TERR-01)
- [x] 11-04-PLAN.md — v3 cost bounds enforced by command: mesh ratio <= 1.5, hex descent p95 < 8 ms (TERR-05)
- [ ] 11-05-PLAN.md — v3 border stencil and rock thresholds, normal seam and cross-LOD tolerance tests (TERR-03)
- [ ] 11-06-PLAN.md — Pre-flight cost evidence, ADR-0020 draft, human flight of v3 with hash-bound verdict line (TERR-05, TERR-06)
- [ ] 11-07-PLAN.md — Pin v3 goldens, make v3 the app default after approval, accept ADR-0020 (TERR-01, TERR-02, TERR-06)
- [ ] 11-08-PLAN.md — Terrain relief image-diff golden through LitTextured and CI wiring (TERR-04)
- [ ] 11-09-PLAN.md — Linux golden via CI regeneration, each push explicitly user-authorised (TERR-04)
- [ ] 11-10-PLAN.md — Full gate, invariance audit, requirement bookkeeping (TERR-01..06)

**UI hint**: yes

### Phase 12: LOD Detail Transition
**Goal**: Level changes during flight no longer show a blur-then-sharpen pop in surface colour and detail
**Depends on**: Phase 11
**Requirements**: VIS-01, VIS-02, VIS-03, VIS-04, VIS-05, VIS-06
**Success Criteria** (what must be TRUE):
  1. During a descent the user sees surface colour and detail blend across level changes instead of popping, confirmed in a human flight
  2. A scripted descent test measures sharpness/colour change across level changes and passes a stated threshold, with the v1.0 behaviour recorded as the failing baseline
  3. The blend follows the same morph factor as vertex geomorphing, adds no synchronous waits, introduces no seam cracks, and the coarse-fallback path also blends
  4. Any new blend input in the generic render API carries no planetary types, and an ADR extends or supersedes ADR-0014 with the measurement behind the choice
**Plans**: TBD
**UI hint**: yes

### Phase 13: Validation & CI Debt Closure
**Goal**: The remaining v1.0 validation paperwork and CI gaps are closed or explicitly documented as decisions
**Depends on**: Phase 12
**Requirements**: DEBT-06, DEBT-07, DEBT-08, DEBT-09, DEBT-10
**Success Criteria** (what must be TRUE):
  1. `03-VALIDATION.md` has its Wave 0 rows resolved against current tests and truthful `nyquist_compliant` / `wave_0_complete` flags
  2. Phases 01, 02, 04 and 05 each have an audited VALIDATION.md with gaps filled by tests or a recorded justification
  3. macOS image-diff goldens pass on a macOS runner, or the skip is documented with reason, CI behaviour and revisit condition
  4. The bench job is either blocking with a stated regression threshold or recorded as intentionally non-blocking with the reason
**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1-8 (incl. 7.1) | v1.0 | 90/90 | Complete | 2026-11-27 |
| 9. Overlay, CI & Test Hygiene | v1.1 | 4/4 | Complete | 2026-11-27 |
| 10. Free-Flight 6DOF Camera | v1.1 | 3/3 | Complete | 2026-11-27 |
| 11. Terrain Relief & Normals | v1.1 | 4/10 | In progress | - |
| 12. LOD Detail Transition | v1.1 | 0/TBD | Not started | - |
| 13. Validation & CI Debt Closure | v1.1 | 0/TBD | Not started | - |

## Backlog

Candidates live in `.planning/todos/pending/`; the post-Phase-8 visual feedback todo is covered by Phases 9-12.

### Phase 999.1: Planet generator from seed (BACKLOG)

**Goal:** Create a planet from a seed with a size and mass, as an archetype (earth-like, rocky, gas giant, ice, hot/lava), with water and a config for mountain height and sharpness, with or without an atmosphere; later simulate a star system of one, two or three stars.
**Requirements:** TBD (candidate milestone v1.2)
**Plans:** 0 plans
Detail: `.planning/todos/pending/2026-11-27-planet-generator-from-seed.md`
