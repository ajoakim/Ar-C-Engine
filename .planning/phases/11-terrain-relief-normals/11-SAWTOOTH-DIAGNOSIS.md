# 11 Sawtooth diagnosis: v3 mountain tops with high `--relief`

Status: diagnosis only, no code changed. Evidence from two temporary tests (deleted):
a software-adapter render of PlanetaryApp (hex, v3, relief 1/2/3, ~300 m altitude, mountain site)
and a numeric mesh-sampling study (`PlanetTerrain::height_at_dir`, 337 mountain-mask sites,
random 2 km lines, true height vs piecewise-linear interpolation at the vertex step).

## Root cause
The mountains layer is `A * r^2` with `r = 1 - |2 fbm - 1|`. The `abs` makes every crest a true
C0 crease (slope jumps from +s to -s at v = 0.5). A crease is not band-limited: linear
interpolation between mesh vertices cuts the peak by `~ slope_jump * step`, and where the vertex
row crosses the crest at a varying phase the cut alternates, giving a regular row of triangular
teeth with period equal to the vertex spacing. It is not an octave, wavelength or normals problem.

Numbers (max vertical interpolation error along a line, divided by vertex step; p90 / max over
sites, identical at every step from 8 m to 134 m, i.e. refining the mesh does not shrink the
relative error):

| relief | full v3 p90 | full v3 max | max slope p90 |
|--------|-------------|-------------|---------------|
| 1 | 0.10 | 0.5 | 0.81 |
| 2 | 0.20 | 1.0 | 1.61 |
| 3 | 0.31 | 1.5 | 2.42 |

The mountains term alone gives the same figures (0.304 / 1.50 at step 17 m, relief 3), so the
mountains crease is the whole effect. Replacing `|x|` by `sqrt(x^2 + 0.1^2) - 0.1` on the same
term drops err/step to p90 0.011-0.07, max 0.007-0.106 (about 20-50x), while the max slope only
goes 2.44 -> 1.89.

LOD budget link: selection refines until `kappa * cell * proj_scale / d <= tau` (kappa 0.25,
tau 2 px), so `cell * proj / d ~ 8 px` at every distance. A crease error of `e * cell` is then
`e * 8` px on screen at every distance: p90 0.8 / 2.4 / 2.5 px and max 4 / 8 / 12 px at relief
1 / 2 / 3. The teeth have a constant pixel height and spacing that shrinks with distance (as in
the screenshot: teeth narrow toward the right where the crest recedes), and they grow linearly
with relief_scale because the crease slope jump is multiplied by it. `kappa` is a fixed roughness
assumption (0.25); real v3 slopes at crests are 0.8 (relief 1) to 2.4 (relief 3), so the assumed
error budget is exceeded by 2x to 12x at crests.

Dark streaks: a crest vertex's height-derived normal straddles the crease, so adjacent
triangles on the two flanks, and the dark far-side flank at steep slopes (65 deg at relief 3), light
very differently from vertex to vertex. Consequence of the same crease, not tested separately.

## Hypotheses
1. Octave under-sampling: REFUTED. Hex L16 spacing is 4.2 m (sample_angle0 0.0435 rad at n=16,
   L16 = 6.6e-7 rad). Mountain octaves have lattice cells 13.6 km, 6.8, 3.4, 1.7 km, 0.85 km
   (f470..f7520); hills f200..f1600 give 32 km .. 4 km. Even the L12 spacing (67 m) is 12x finer
   than the finest octave. Removing the top octaves (5 -> 4 -> 3) lowers the slope (p50 1.36 ->
   1.06 -> 0.88) but the crease error stays (max 1.4-1.8 at 4 octaves, 1.0-1.3 at 3).
2. LOD morph/skirts: not the cause of static teeth (camera at 0 m/s, regular spacing equals the
   cell spacing). Not independently varied.
3. Normal stencil mismatch: secondary; the streaks follow from the crease (see above).
4. Fine-octave slope explosion with relief: REFUTED as stated. relief_scale multiplies the
   whole layer linearly, so slope and crease error both scale x s; they are not dominated by
   fine octaves (each octave adds the same gradient `w_k f 2^k`, so each contributes equally).
   Capping fine-octave growth would not remove the crease.

## Render evidence
Renders (scratchpad `saw/`): relief 1 at the same site is smooth; relief 3 shows zigzag
crests at distance (saw_N_r3.png, saw2_S_3000_r3.png), a few very coarse facets close up
(saw2_S_1500_r3.png). Many views are dark because the steep flank faces away from the sun.

## Fix options, ranked
1. Round the crease in the v3 mountains shape: `ax = sqrt(x^2 + eps^2) - eps` with eps about 0.1
   (also for the v3 ridge term via a v3-only shape; v2 `ridge_shape` must stay bit-identical).
   Pure function of the direction, so shared-border and cross-LOD consistency hold; ax is
   1-Lipschitz and `r <= 1`, so LOD residual and height bounds stay sound; one sqrt replaces one
   abs, no cost change. Trade-off: changes v3 heights at relief 1.0 (v3 not yet golden, but the
   `relief_scale.rs` fingerprint 0x1b54f6ebf399a5d9 and any v3 snapshot must be re-recorded, so it
   needs user approval). v1/v2 unaffected. Choose eps so that crest rounding width exceeds the
   vertex step at the budget distance (larger eps = softer crests; 0.1 gives ~20-50x less error).
   Variant 1b (keeps relief-1 bit identity): `eps = 0.1 * (s - 1) / s` or similar so eps = 0 at
   scale 1; teeth at relief 1 remain (max 0.5 step, about 4 px) but scaling no longer worsens them.
2. Make LOD slope-aware: `kappa_eff = kappa * max(1, relief_scale)` (or measure slope) in the
   selection. Bit-identical at relief 1. Reduces tooth px by 1/s but triangle count grows about
   s^2 at crests (relief 3: ~9x in mountains), threatening the 8 ms descent gate and the 1.5 mesh
   ratio; the crease is still there, only smaller. Reasonable only as a partial complement.
3. Normal smoothing near crests (wider stencil): cosmetic, hides dark streaks only, does not
   change silhouette teeth, and would break border-normal consistency unless applied uniformly.
4. Band-limit or cap fine octaves / reduce octave count: not recommended, evidence above (does
   not remove the crease, and changes the calibrated look).
