---
phase: 11-terrain-relief-normals
plan: 06D
type: execute
wave: 9
depends_on: ["11-06C"]
files_modified:
  - extensions/planetary/planetary-core/src/terrain.rs
  - extensions/planetary/planetary-core/src/lod/model.rs
  - extensions/planetary/planetary-core/tests/height_dist_v3.rs
  - extensions/planetary/planetary-core/tests/relief_scale.rs
  - extensions/planetary/planetary-app/src (LIGHT_AMBIENT only, if no golden changes)
  - docs/architecture/adr/0020-generator-v3-relief-and-height-derived-normals.md
  - .planning/phases/11-terrain-relief-normals/11-06D-SUMMARY.md
  - .planning/ROADMAP.md
  - .planning/STATE.md
autonomous: true
gap_closure: true
requirements: [TERR-01]

must_haves:
  truths:
    - "User feedback after flying 11-06C: mountain tops look flat-topped at nearly one elevation; heights should follow a bell curve; shadowed slopes are pitch black"
    - "Only generator >= 3 changes; v1/v2 bit-identical (11-01 v2 mesh byte golden, GOLDEN_HEIGHTS_V1/V2, residual pins); exact IEEE ops only; r in [0,1] and the range-height multiplier in a known bounded interval so LodModel residual and max_relief stay sound proofs"
    - "Bands never loosened: land slope p99 25-35 deg, p50 < 8, plains 0.20-0.60, mountain share 0.15-0.35, max_relief 9-11 km; if one cannot be met, STOP and report"
    - "A new test pins the distribution shape (peaks not all equal, bell-like) with a negative control using the old shape"
    - "No flight verdict, no default flip, no v3 goldens, no requirement ticks, no push; terrain.rs changed so the flight approval is void"
    - "The ambient change is made only if no golden or test expectation changes; otherwise reverted and reported"
---

<objective>
Gap fix: v3 height distribution. Task 1 measures it, Task 2 fixes it (range-height multiplier plus de-saturated crest shape), Task 3 re-verifies everything, Task 4 raises the ambient light if no golden changes.
</objective>

<tasks>

<task type="auto">
<name>Task 1: Measure land and peak height distributions on current v3</name>
<files>extensions/planetary/planetary-core/tests/height_dist_v3.rs</files>
<action>Histogram land heights and hill-climbed mountain peak heights (mean, median, std, skew, percentiles) and the mountains term itself at relief 1. Explain the cause.</action>
<verify>cargo test --release -p planetary-core --test height_dist_v3 -- --nocapture</verify>
</task>

<task type="auto" tdd="true">
<name>Task 2: Bell-shaped mountain heights</name>
<files>extensions/planetary/planetary-core/src/terrain.rs, extensions/planetary/planetary-core/src/lod/model.rs, extensions/planetary/planetary-core/tests/height_dist_v3.rs, extensions/planetary/planetary-core/tests/relief_scale.rs</files>
<action>Multiply the mountains term by a bounded range-height field (average of independent low-frequency noise fields, approximately normal) and de-saturate the crest shape. Update LodModel terms with proofs, recalibrate within bands, re-record the relief_scale fingerprint, add the distribution test with an old-shape negative control.</action>
<verify>cargo test -p planetary-core --locked</verify>
</task>

<task type="auto">
<name>Task 3: Re-verify bands, soundness, normals, cost gates, render before/after</name>
<files>docs/architecture/adr/0020-generator-v3-relief-and-height-derived-normals.md</files>
<action>Run relief_v3, lod_bounds_v3 (relief 1/2/3, cube/hex), normals_v3, mesh-ratio and strict descent gates; render before/after PNGs of a mountain region and inspect them. Update ADR-0020.</action>
<verify>bash scripts/mesh-ratio-check.sh 3 2 1.5; descent bench --strict at relief 1, 2, 3</verify>
</task>

<task type="auto">
<name>Task 4: Raise LIGHT_AMBIENT if no golden changes</name>
<files>planetary-app lighting constant</files>
<action>Grep which goldens render with app lighting; raise ambient to about 0.08-0.12 only if none change, else revert and report.</action>
<verify>cargo test -p planetary-app --locked</verify>
</task>

</tasks>
