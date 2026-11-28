---
phase: 11-terrain-relief-normals
plan: 06C
type: execute
wave: 8
depends_on: ["11-06B"]
files_modified:
  - extensions/planetary/planetary-core/src/terrain.rs
  - extensions/planetary/planetary-core/tests/crease_v3.rs
  - extensions/planetary/planetary-core/tests/generator_v3.rs
  - extensions/planetary/planetary-core/tests/relief_scale.rs
  - docs/architecture/adr/0020-generator-v3-relief-and-height-derived-normals.md
  - .planning/phases/11-terrain-relief-normals/11-06C-SUMMARY.md
  - .planning/ROADMAP.md
  - .planning/STATE.md
autonomous: true
gap_closure: true
requirements: [TERR-01, TERR-03]

must_haves:
  truths:
    - "The v3 mountains shape and a NEW v3-only ridge shape use ax = sqrt(x^2 + 0.1^2) - 0.1 instead of |x| (x = 2 fbm - 1); r = 1 - ax stays in [0, 1], ax is 1-Lipschitz, so the LodModel residual and height bounds stay sound and max_relief is unchanged by the rounding"
    - "v2 ridge_shape/ridge_at and every v1/v2 path are untouched and bit-identical (11-01 v2 mesh byte golden, GOLDEN_HEIGHTS_V1/V2, v1/v2 residual pins)"
    - "Only exact IEEE ops (*, +, -, sqrt); banned_ops scan passes (sqrt is not banned)"
    - "Relief bands at 1.0, soundness at 1/2/3 (cube and hex), normals tests and cost gates are re-verified, never loosened; any constant re-tune stays inside the user-approved bands"
    - "A new regression test bounds the crease interpolation error / step at relief 1, 2, 3 and a negative control with the old abs shape fails the bound"
    - "No flight verdict, no default flip, no v3 goldens, no requirement ticks, no push; terrain.rs changed so the 11-06 flight approval is void"
---

<objective>
Gap fix after the user flew v3 at high relief: sawtooth mountain tops. Root cause (11-SAWTOOTH-DIAGNOSIS.md): the `abs` in the mountains shape is a C0 crease; linear interpolation between mesh vertices cuts crests by about slope_jump * step and the error grows with relief_scale. User-approved fix (ranked option 1): round the crease with `ax = sqrt(x^2 + eps^2) - eps`, eps = 0.1.
</objective>

<tasks>

<task type="auto" tdd="true">
<name>Task 1: Rounded crease in the v3 mountains and ridge shapes</name>
<files>extensions/planetary/planetary-core/src/terrain.rs, extensions/planetary/planetary-core/tests/generator_v3.rs, extensions/planetary/planetary-core/tests/relief_scale.rs</files>
<action>Add round_abs, ridge_shape_v3, ridge_at_v3; mountains_at uses round_abs; compose in sample() and the cube RidgeLayer use the v3 ridge when generator_version >= 3. Re-record the relief_scale fingerprint; adapt the v3-vs-v2 inert test (ridge now differs by design).</action>
<verify>cargo test -p planetary-core --locked (banned_ops, v2 byte golden, goldens, residual pins)</verify>
<done>v1/v2 bit-identical; v3 heights deterministic; fingerprint re-recorded.</done>
</task>

<task type="auto" tdd="true">
<name>Task 2: Re-verify bands, soundness, normals; crease regression test</name>
<files>extensions/planetary/planetary-core/tests/crease_v3.rs, extensions/planetary/planetary-core/src/terrain.rs</files>
<action>Re-run relief_v3, lod_bounds_v3, normals_v3, surface_v3; re-tune within the approved bands only if a band fails (never loosen). Add crease_v3.rs: 320 mountain-mask sites, true heights interpolated at vertex steps 17 and 67 m, bound on crease error / step per relief scale, negative control with the old abs shape (kept only in the test).</action>
<verify>cargo test -p planetary-core --locked --test relief_v3 --test lod_bounds_v3 --test normals_v3 --test crease_v3</verify>
<done>All bands pass; the control fails the bound, the fix passes.</done>
</task>

<task type="auto">
<name>Task 3: Cost gates, ADR-0020 and bookkeeping</name>
<files>docs/architecture/adr/0020-generator-v3-relief-and-height-derived-normals.md, .planning/ROADMAP.md, .planning/STATE.md</files>
<action>After `rm -rf target/criterion/mesh_build` run `bash scripts/mesh-ratio-check.sh 3 2 1.5` and strict 8 ms descent at relief 1, 2, 3 on hex and cube. Update ADR-0020 (still proposed), ROADMAP (13 plans, 5/13) and STATE.</action>
<verify>fmt --check, clippy -D warnings, full cargo test of planetary-core and planetary-app.</verify>
<done>Numbers recorded; the user must re-fly (--generator 3 --relief 1, 2, 3).</done>
</task>

</tasks>
