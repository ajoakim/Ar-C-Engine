# 11-FLAT-DIAGNOSIS: "all flat, no way to distinguish hills and mountains"

Method: temporary integration test (not committed; copy in the session scratchpad as `zz_flat_diag.rs.txt`) that builds
`PlanetaryApp` exactly as `planetary_flight.rs` does (AppSettings + `terrain.terrain = demo_v2/demo_v3`), forces the
camera pose, runs frames on the real `WgpuBackend::new_offscreen_software` (WARP) and reads the framebuffer.
PNGs: `<scratchpad>/out/` (`g2hex_mtn_10km.png`, `g3hex_mtn_10km.png`, `user_g3cube_plusX_563m.png`,
`user_g3cube_target_563m.png`, and the 2/50 km and cube variants).

## Findings per question

1. `--generator` reaches the interactive path: YES. `planetary_flight.rs` sets `settings.terrain.terrain` before
   `PlanetaryApp::with_clock(settings, ..)`, which clones `settings.terrain` into `TerrainSystem::new`. In the built
   app, height at the sun target is 610.9 m (v2) vs 1483.0 m (v3); v3 box heights p50 1335 / max 2581 vs v2 p50 558 /
   max 1245. v2 and v3 renders at the same pose differ strongly (v2 = featureless green plain, v3 = rolling hills with
   shaded slopes). `--generator 3 --topology cube` also differs from hex only by topology. Not a root cause.
   (Gap: there is no startup line printing the active generator version.)
2. Normals/lighting reach the shader: YES. Planet patches use `lit_textured.wgsl` (N.L with
   `ambient + light_color * ndl`, `light_color` = colour * 1.8 intensity). Slopes show clearly in the v3 render.
3. Palette: varies with height and (v3) slope; colour bands are visible from orbit and in the renders. Fine.
4. Resolution: not an issue. Cube L16 spacing = R * (pi/2/32) / 2^16 = 4.8 m. Drawn levels at 563 m reach L16 (30-50
   patches per level 7..16); at 2-50 km the levels go to 8..12 (spacing 100 m .. 2 km at 50 km), against hills
   (wavelength ~ 1/200 rad = 32 km, 4 octaves down to 4 km) and mountains (13 km down to 0.85 km). Features are
   resolved; detail budget/LOD are not the cause.

## Root causes, ranked

### 1. The sun is a fixed world-space direction that only gives 35 deg elevation at one point of the planet (HIGH, observed)
`relief_light(sun_target(script, radius))` sets one world direction for the whole session; interactive free flight
(no script) uses the descent-script target `(0.53, 0.78, 0.32)`. The free-flight start is `+X` over the orbit; sun
elevation at `+X` is -14.2 deg (below the horizon). There `ndl = 0` everywhere on near-flat ground, so the surface is
lit only by `LIGHT_AMBIENT = 0.025`: near-uniform dark colour (`user_g3cube_plusX_563m.png`: near-black ground), and
the sun-tinted atmosphere gives a dusk sky (pinkish-grey in the user's screenshot, same cause). With the sun at or
below the horizon, no slope can show, regardless of generator. Where the sun is 35 deg up (target region) the same
generator-3 cube view at 563 m shows hills with shaded slopes (`user_g3cube_target_563m.png`), and v3 at 10 km is
clearly hilly (`g3hex_mtn_10km.png`) while v2 is flat (`g2hex_mtn_10km.png`).

### 2. Near +X the terrain itself is a flat lowland and mountains are rare (HIGH, measured)
v3 height at `+X` is 620 m and the maximum over 3-130 km east of `+X` is 616 m: the surface is a plain. The flat
horizon is real: no terrain in that direction rises above the plain. Mountains are gated by `smoothstep(0.44,0.74,mask)`:
over 100k random directions, of land only 5.3 % exceeds 3500 m (v3) vs 0.1 % (v2), 14 % exceeds 2500 m. A 3.5 km
peak does show above the horizon at range (horizon dip at 563 m ~0.75 deg), but only if one exists in that sector,
and distance haze (`HAZE_DENSITY` 1e-5/m, ~40 % at 85 km, ~70 % at 150 km) fades it toward the sky colour.

### 3. Relief is physically gentle at low altitude (MEDIUM)
Hills are 600 m over ~32 km (about 2 deg slopes); at 563 m the 563 m render shows soft undulation only. This is
accurate scale (0.05 % of radius), not a bug, but with the sun low it is invisible. No vertical exaggeration exists.

### Not causes
Generator flag plumbing, shader/pipeline choice, palette, mesh resolution, near/far, LOD level selection.

## Proposed minimum fix (not applied; no code changed)
- A (fix root cause 1): make the relief light follow the camera: recompute `light.direction` each frame from the camera
  position direction with `sun_direction_for_target(camera_dir)` (35 deg elevation, fixed azimuth) in free flight
  (keep the fixed sun for scripted descents so goldens/tests stay deterministic). Alternative: a slowly rotating sun
  with a day/night cycle, but HLD §22.4 excludes day-night in P0, so prefer the camera-following relief light;
  a debug key to raise the sun is a cheaper variant.
- B: raise `LIGHT_AMBIENT` (0.025 -> ~0.08) so a sun below the horizon is not black; optional.
- C (observability): print the active generator version, topology and sun elevation at the camera in the startup
  line / window title; add a test that `AppSettings` built from `--generator N` has `terrain.generator_version == N`
  (the example's arg parsing is not unit tested beyond the usage string).
- D (only if mountains must be common near the start): the Phase 11 constants/relief bands need no change for
  visibility; if the demo start should show mountains, start the free-flight camera at a high-relief direction
  (e.g. near the target region, 2.6 km max within 230 km box) rather than `+X`.
Phase 11 relief constants and render path do not need to change; the lighting model (fixed sun) and the start
location do.
