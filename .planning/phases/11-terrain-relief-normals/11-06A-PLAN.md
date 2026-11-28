---
phase: 11-terrain-relief-normals
plan: 06A
type: execute
wave: 6
depends_on: ["11-06"]
files_modified:
  - extensions/planetary/planetary-app/src/lib.rs
  - extensions/planetary/planetary-app/examples/planetary_flight.rs
  - extensions/planetary/planetary-app/tests/free_flight_app.rs
  - extensions/planetary/planetary-app/README.md
  - .planning/phases/11-terrain-relief-normals/11-06A-SUMMARY.md
  - .planning/ROADMAP.md
  - .planning/STATE.md
autonomous: true
gap_closure: true
requirements: [TERR-04]

must_haves:
  truths:
    - "In free flight (no script, no pose override) the light direction is recomputed each frame with sun_direction_for_target(camera position), so the sun stands at SUN_ELEVATION_DEG (35) above the horizon at the camera at +X, -X and +Y"
    - "Scripted descents and pose_override paths keep the fixed sun; existing descent tests are unmodified"
    - "The startup line prints the generator version, topology and sun elevation at the camera; --generator N yields terrain.generator_version == N"
    - "terrain.rs is byte-identical (git hash-object 07439dc86cd9273e3d4627b8e3b6625195936ee0); LIGHT_AMBIENT, the start direction and all terrain constants are unchanged; no golden changes; no flight verdict, default flip, golden pin or requirement tick"
---

<objective>
Gap fix from 11-FLAT-DIAGNOSIS.md root cause 1: the fixed world-space sun sits 14 degrees below the horizon at the free-flight start (+X), so terrain looks flat and dark. Make the sun follow the camera in free flight only (fixed-elevation headlight, no day/night cycle, HLD 22.4) and add observability (C).
</objective>

<tasks>

<task type="auto">
<name>Task 1: Sun follows the camera in free flight; startup readout</name>
<files>extensions/planetary/planetary-app/src/lib.rs, extensions/planetary/planetary-app/examples/planetary_flight.rs</files>
<action>In PlanetaryApp::update free-flight branch set light.direction = sun_direction_for_target(camera position) after the free-flight update. Add AppSettings::set_generator, light_direction, sun_elevation_at_camera_deg, look_line; append look_line to the startup line. Do not touch LIGHT_AMBIENT, start direction or terrain.rs.</action>
<verify>cargo clippy -p planetary-app --all-targets -- -D warnings; terrain.rs hash unchanged</verify>
<done>Startup line shows generator, topology, sun elevation.</done>
</task>

<task type="auto">
<name>Task 2: Tests and docs</name>
<files>extensions/planetary/planetary-app/tests/free_flight_app.rs, extensions/planetary/planetary-app/README.md</files>
<action>Tests: elevation at +X/-X/+Y equals the configured value (>0); scripted descent and pose override keep the fixed sun; --generator N (set_generator) gives generator_version N; startup look line. README and example header note the follow-camera sun.</action>
<verify>cargo test -p planetary-app --locked (full crate)</verify>
<done>All green with existing goldens unchanged.</done>
</task>

</tasks>
