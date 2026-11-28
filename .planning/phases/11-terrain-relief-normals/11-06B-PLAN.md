---
phase: 11-terrain-relief-normals
plan: 06B
type: execute
wave: 7
depends_on: ["11-06A"]
files_modified:
  - extensions/planetary/planetary-core/src/terrain.rs
  - extensions/planetary/planetary-core/src/lod/model.rs
  - extensions/planetary/planetary-core/tests/relief_scale.rs
  - extensions/planetary/planetary-core/tests/lod_bounds_v3.rs
  - extensions/planetary/planetary-app/src/lib.rs
  - extensions/planetary/planetary-app/examples/planetary_flight.rs
  - extensions/planetary/planetary-app/tests/free_flight_app.rs
  - extensions/planetary/planetary-app/README.md
  - docs/architecture/adr/0020-generator-v3-relief-and-height-derived-normals.md
  - .planning/phases/11-terrain-relief-normals/11-06B-SUMMARY.md
  - .planning/ROADMAP.md
  - .planning/STATE.md
autonomous: true
gap_closure: true
requirements: [TERR-01, TERR-05]

must_haves:
  truths:
    - "TerrainParams gains relief_scale: f64 (default 1.0, validated finite and in 0.1..=8.0 for v3) that multiplies only the ridge, hills and mountains amplitudes in generator v3; TerrainParams stays Copy; generator version stays 3"
    - "At relief_scale 1.0 v3 heights, masks, max_relief and LodModel residuals are bit-identical to the pre-change values (fingerprint recorded before the edit); v1 and v2 outputs, goldens and the 11-01 v2 mesh byte golden are untouched"
    - "max_relief() and the v3 LodModel bounds scale with relief_scale; the 2000-pair soundness test (cube and hex) passes strictly at 1, 2 and 3"
    - "planetary-app exposes AppSettings::set_relief_scale and the example flag --relief <scale> (usage error when non-finite or outside 0.1..=8.0); the startup line prints relief scale"
    - "Cost at 1.0 still passes the existing gates (mesh ratio 3 2 1.5, strict 8 ms descent); cost at 2 and 3 is measured on hex and cube and reported, the gate is not loosened; ADR-0020 stays proposed and records D-02 superseded by the user decision of 2026-11-27 plus the measured table"
    - "No flight verdict, no default flip, no v3 goldens, no requirement ticks, no push: terrain.rs changed, so the 11-06 flight approval is void and the user must re-fly"
---

<objective>
Gap fix after the user flew v3 (with the 11-06A sun fix): only gentle rolling hills, no mountains. User decision (2026-11-27, supersedes D-02 "no exaggeration knob"): an explicit, config-settable relief exaggeration for generator v3.
</objective>

<tasks>

<task type="auto" tdd="true">
<name>Task 1: relief_scale in the v3 terrain and LOD bounds</name>
<files>extensions/planetary/planetary-core/src/terrain.rs, extensions/planetary/planetary-core/src/lod/model.rs, extensions/planetary/planetary-core/tests/relief_scale.rs, extensions/planetary/planetary-core/tests/lod_bounds_v3.rs</files>
<action>Record a v3 fingerprint (heights, masks, max_relief, residuals) before editing. Add the field, validation, scaling in compose_v3 / max_relief / LodModel v3 terms (exact at 1.0). Tests: fingerprint equality at 1.0, v1/v2 ignore the field, validation, heights and slopes grow, soundness at 1, 2, 3 on cube and hex.</action>
<verify>cargo test -p planetary-core --locked (full crate, v2 byte golden and v1/v2 residual pins unchanged)</verify>
<done>All green; v3 at 1.0 bit-identical.</done>
</task>

<task type="auto">
<name>Task 2: Plumb --relief through planetary-app</name>
<files>extensions/planetary/planetary-app/src/lib.rs, extensions/planetary/planetary-app/examples/planetary_flight.rs, extensions/planetary/planetary-app/tests/free_flight_app.rs, extensions/planetary/planetary-app/README.md</files>
<action>AppSettings::set_relief_scale (order-independent with set_generator), `--relief <scale>` flag, `relief scale` in the startup look line, example header and README.</action>
<verify>cargo clippy -D warnings on touched crates; cargo test -p planetary-app --locked</verify>
<done>Flag validated, startup line shows the scale.</done>
</task>

<task type="auto">
<name>Task 3: Cost measurement, ADR-0020 and bookkeeping</name>
<files>docs/architecture/adr/0020-generator-v3-relief-and-height-derived-normals.md, extensions/planetary/planetary-app/README.md, .planning/ROADMAP.md, .planning/STATE.md</files>
<action>At 1.0 re-run `bash scripts/mesh-ratio-check.sh 3 2 1.5` and the strict 8 ms descent. Measure `--bench-descent --frames 600 --generator 3 --relief 2|3` on hex and cube; record worst-band p95 and the highest passing scale as the supported range. Update the ADR draft (status proposed), README, ROADMAP (12 plans, 5/12) and STATE.</action>
<verify>Numbers recorded in the ADR table and the SUMMARY; gate unchanged at 8 ms.</verify>
<done>Supported range documented; 11-06 checkpoint to be re-flown by the user.</done>
</task>

</tasks>
