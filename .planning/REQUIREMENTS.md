# Requirements: General-Purpose 3D Engine

**Defined:** 2026-11-27
**Milestone:** v1.1 Visual Fidelity & Flight
**Core Value:** The engine core must remain general-purpose and independent of any specialized capability (planetary rendering, physics, scripting, etc.) — specialized implementations are validated through demanding reference applications but never become assumptions embedded in the core.

Scope rule for every requirement below: planetary-specific work (terrain generation, hex/cube topology, planetary camera, planet shading) lives only in the planetary extension. Any capability added to a generic crate (render, SDK, platform, core) must be expressed without planetary concepts. v1.0 requirements are archived in `.planning/milestones/v1.0-REQUIREMENTS.md`.

## v1.1 Requirements

### VIS: LOD detail transition

- [ ] **VIS-01**: When the terrain LOD level changes during flight, surface colour and detail blend between the parent and child level over a transition window instead of switching in one frame (no visible blur-then-sharpen pop).
- [ ] **VIS-02**: The colour/detail blend is driven by the same per-patch morph factor (or an equivalent continuous screen-space-error factor) that drives vertex geomorphing, so geometry and appearance transition together.
- [ ] **VIS-03**: A scripted descent test measures frame-to-frame surface sharpness/colour change across level changes and asserts it stays below a stated threshold; the v1.0 behaviour is recorded as the failing baseline.
- [ ] **VIS-04**: The transition adds no synchronous wait on generation or loading, no new cracks at patch seams, and the coarse-fallback path still shows a blended (not popped) result.
- [ ] **VIS-05**: Any new render-side blend input (for example a per-vertex or per-draw blend factor) is exposed through the generic render API without planetary types; the planetary extension supplies the values.
- [ ] **VIS-06**: An ADR extends or supersedes ADR-0014 with the chosen transition technique and the measurement that justified it.

### TERR: Terrain relief and normals

- [ ] **TERR-01**: Generator version 3 produces visibly more varied relief (hills and mountain ranges at several scales) than v2, selected by `generator_version`, deterministic across runs and platforms for a given seed.
- [ ] **TERR-02**: Generator v1 and v2 outputs remain bit-identical to their existing golden data when selected explicitly.
- [ ] **TERR-03**: Each terrain patch carries surface normals derived from its height data (not only from the sphere), and normals agree across patch and LOD-level seams within a tested tolerance (no lighting seams).
- [ ] **TERR-04**: The terrain shader lights the surface using the height-derived normals (bump shading), so relief reads under directional light at flight altitudes; an image-diff golden covers the result.
- [ ] **TERR-05**: Generator v3 and normal computation stay within a stated worker-time budget measured against the v2 baseline (v2 is about +23 % over v1), and the descent p95 frame time on hex does not regress beyond the stated bound; rendering never blocks on generation.
- [ ] **TERR-06**: v3 becomes the planetary app default only after a human flight confirms the relief reads as hills and mountains; an ADR records the v3 layers, amplitudes, normal method and measured cost.

### CAM: Free-flight 6DOF camera

- [ ] **CAM-01**: In the planetary example/app the user can roll the camera about its view axis with dedicated keys or mouse input.
- [ ] **CAM-02**: The user can strafe left/right along the camera right axis.
- [ ] **CAM-03**: The user can move up/down along the camera up axis (vertical thrust), independent of the planet local up.
- [ ] **CAM-04**: Camera orientation is stored so repeated roll, pitch and yaw do not drift or lose normalisation (unit test of a long input sequence), and the camera stays in f64 world space with camera-relative rendering.
- [ ] **CAM-05**: The existing orbit-to-surface flight (speed scaling by altitude, scripted descent) still works unchanged, with a toggle or documented default for free-flight versus planet-aligned orientation.
- [ ] **CAM-06**: 6DOF input mapping lives in the example or planetary extension; any generic camera/controller type added to a non-planetary crate has no planetary assumptions, and `check-deps` stays green.

### DEBT: Tech-debt sweep

- [x] **DEBT-01**: (IN-01) CI derives the expected Rust toolchain version from `rust-toolchain.toml` instead of a hard-coded `1.99.0` string; bumping the toolchain needs one edit.
- [x] **DEBT-02**: (IN-02) The overlay renderer is rebuilt when the surface format changes (and the font atlas re-uploaded), and an optional window scale factor can be passed so the overlay is not tiny on HiDPI windows.
- [x] **DEBT-03**: (IN-03) Image-diff tolerance uses an absolute bad-pixel cap (or tighter fraction) so a small-feature regression fails, and a negative control proves a small change is caught while the max-delta report is kept.
- [x] **DEBT-04**: Overlay counts (selected, queue pending, detail patches, etc.) render as integers; only genuinely fractional values (times, ratios) show decimals.
- [x] **DEBT-05**: The "detail patches used / cap" overlay line is explained and fixed: either the cap is enforced and used never exceeds it, or the label states the cap is soft and what the two numbers mean; a test pins the behaviour.
- [ ] **DEBT-06**: `03-VALIDATION.md` is completed: Wave 0 rows resolved against the current tests, `nyquist_compliant` and `wave_0_complete` set truthfully.
- [ ] **DEBT-07**: Nyquist validation gaps for phases 01 and 02 are audited and filled (tests added or a recorded justification per gap), VALIDATION.md frontmatter updated.
- [ ] **DEBT-08**: Nyquist validation gaps for phases 04 and 05 are audited and filled likewise.
- [ ] **DEBT-09**: macOS image-diff goldens either exist and pass on a macOS runner, or the skip is documented (reason, CI behaviour, revisit condition) in docs and an ADR/assumption entry.
- [ ] **DEBT-10**: The CI bench job is either made blocking with a stated regression threshold, or the decision to keep it non-blocking is recorded with the reason.

## Future Requirements

Deferred, not in v1.1.

- True atmospheric scattering pass replacing the limb-ring mesh (ADR-0019 revisit)
- Texture-based detail/normal maps and compressed texture containers (ktx2)
- GPU-driven rendering, indirect draws, cluster/Nanite-style LOD DAG
- HiDPI polish beyond the overlay scale factor
- winit 0.31 migration once stable
- Real geospatial data ingestion (P2) and persistent world edits (P2)

## Out of Scope

| Feature | Reason |
|---------|--------|
| ECS adoption | ADR-0017: spike did not justify it; CLAUDE.md scope discipline |
| Physics, collision with terrain | Optional future capability; not committed (CLAUDE.md) |
| Gameplay scripting, editor, plugin marketplace | Deferred until a real requirement emerges (CLAUDE.md) |
| Ray tracing, advanced GPU-driven rendering | Future rendering capabilities |
| Weather, day-night, seasons, clouds | Later phases |
| Planetary concepts in engine core / generic crates | Core Value; planetary logic only in the planetary extension (HLD §22.4) |
| Flight-model physics (inertia, collisions, thrust simulation) | 6DOF here means kinematic camera control only |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| DEBT-01 | Phase 9 | Complete |
| DEBT-02 | Phase 9 | Complete |
| DEBT-03 | Phase 9 | Complete |
| DEBT-04 | Phase 9 | Complete |
| DEBT-05 | Phase 9 | Complete |
| CAM-01 | Phase 10 | Pending |
| CAM-02 | Phase 10 | Pending |
| CAM-03 | Phase 10 | Pending |
| CAM-04 | Phase 10 | Pending |
| CAM-05 | Phase 10 | Pending |
| CAM-06 | Phase 10 | Pending |
| TERR-01 | Phase 11 | Pending |
| TERR-02 | Phase 11 | Pending |
| TERR-03 | Phase 11 | Pending |
| TERR-04 | Phase 11 | Pending |
| TERR-05 | Phase 11 | Pending |
| TERR-06 | Phase 11 | Pending |
| VIS-01 | Phase 12 | Pending |
| VIS-02 | Phase 12 | Pending |
| VIS-03 | Phase 12 | Pending |
| VIS-04 | Phase 12 | Pending |
| VIS-05 | Phase 12 | Pending |
| VIS-06 | Phase 12 | Pending |
| DEBT-06 | Phase 13 | Pending |
| DEBT-07 | Phase 13 | Pending |
| DEBT-08 | Phase 13 | Pending |
| DEBT-09 | Phase 13 | Pending |
| DEBT-10 | Phase 13 | Pending |

**Coverage:**
- v1.1 requirements: 28 total (VIS 6, TERR 6, CAM 6, DEBT 10)
- Mapped to phases: 28
- Unmapped: 0

---
*Requirements defined: 2026-11-27*
*Last updated: 2026-11-27 after milestone v1.1 start*
