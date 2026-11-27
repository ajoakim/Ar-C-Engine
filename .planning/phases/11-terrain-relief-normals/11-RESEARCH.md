# Phase 11: Terrain Relief & Normals - Research

**Researched:** 2026-11-27
**Domain:** Deterministic procedural terrain (value-noise layers), height-derived vertex normals, lit-shader consumption, cost budgets (Rust, planetary extension only)
**Confidence:** MEDIUM-HIGH (code and ADR facts are verified by reading the repo; the v3 amplitude/slope numbers and the cross-LOD normal tolerance are design estimates that Wave 0 must measure)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Relief:** Hills at small scale plus mountain ranges at large scale, several scales, visible height range well above v2. Deterministic for a given seed. Not an extreme, exaggerated profile.
- **Normals:** Per-vertex normals computed from height in the generator from neighbouring heights, sampled across patch edges (shared-border sampling, extending the existing crack-free approach) so lighting has no seams. Adds a normal to the vertex payload (payload v5); older payload versions keep working and their goldens stay bit-identical.
- **Cost bound:** Worker time at most +50% versus the v2 baseline and hex descent p95 under 8 ms (v2 hex descent p95 is about 4 ms; v2 cost +23% over v1). Rendering never blocks on generation.
- **Bump detail:** Height-based normals only in this phase. Colour-based or procedural fine detail bump is deferred (record in ADR as future work).
- **Default switch:** v3 becomes the app default only after a human flight confirms the relief; ADR records layers, normals method and measured cost.

### Claude's Discretion
Number and frequencies of noise layers, amplitude constants, ridge/hills blend, rock-slope threshold recalibration, how normals are packed in the payload, shader lighting model details, benchmark parameters.

### Deferred Ideas (OUT OF SCOPE)
- Colour/procedural fine detail bump mapping (shader-side) - later phase.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TERR-01 | v3 generator, more varied relief, selected by `generator_version`, deterministic | Section "Generator versioning" and "v3 layer design" |
| TERR-02 | v1/v2 bit-identical to existing goldens | Section "Golden protection" (additive-only rules, cross-version guard tests) |
| TERR-03 | Height-derived normals agree across patch and LOD seams within tested tolerance | Section "Normals" (finding F1: normals already exist; what is actually missing is the v3-tuned stencil and the cross-level tolerance test) |
| TERR-04 | Terrain shader lights with those normals; image-diff golden | Section "Renderer / shader consumption" (no engine API change needed; golden test design) |
| TERR-05 | Cost within budget vs v2; no blocking | Section "Cost model" and "Cost benches" |
| TERR-06 | v3 default only after human flight; ADR | Section "Default switch and ADR" |
</phase_requirements>

## Summary

**Three findings change the shape of this phase and two need a user decision.**

**F1 (needs user decision): the payload already carries a height-derived per-vertex normal; a "payload v5 that adds a normal" is not needed.** `CELL_PAYLOAD_VERSION = 4` has 52 bytes per vertex: position(3), **normal(3)**, uv(2), morph_position(3), morph_uv(2) as f32 `[VERIFIED: planetary-core/src/cell_payload.rs]`. Both meshers fill that normal from the displaced surface: interior vertices from a central difference of the f64 surface positions (`stencil_normal`-style, cube `mesh.rs` and hex `hex/mesh.rs`), border vertices from the face-independent 4-tap stencil `border_normal_f64` that samples `terrain.surface_point` at `d +/- delta*t1`, `d +/- delta*t2` `[VERIFIED: mesh.rs, hex/mesh.rs]`. Same-level shared borders are already bit-equal (test `border_normals_identical_across_same_level_neighbors`) `[VERIFIED: tests/mesh.rs]`. The lit shader already reads `@location(1) nrm`, transforms it by `draw.normal` and does `max(dot(n, -light_dir), 0)` `[VERIFIED: lit_textured.wgsl]`. The reason the user saw "a smooth surface" is **amplitude and slope, not missing normals**: measured v2 land slopes at mesh resolution are `1 - n.d` p50 ~1.2e-5, p99 ~1.1e-4, max < 2e-4, i.e. about 0.3 degrees median, under 1.2 degrees maximum `[CITED: ADR-0015 decision 5]`. A 0.3 degree tilt changes N.L by well under 1 percent: invisible. So the work is (a) a v3 generator whose land slopes at mesh resolution reach several to tens of degrees, (b) a v3-gated stencil/threshold tuning, (c) the missing cross-LOD normal tolerance test and image golden. Recommendation: **keep the vertex layout and payload version 4 byte layout**; if the user wants the literal "v5", make v5 a header-only bump (same 52-byte vertex, a flags field recording "normals are height-derived, stencil step = grid spacing") and keep the v4 decoder path. Decision D1 below.

**F2 (risk to TERR-05): the descent p95 is dominated by main-thread selection, and selection cost grows with the LOD bound, which grows with amplitude.** `LodModel.height.max = max_relief()` and `residual[L]` are sums of Lipschitz terms of every layer `[VERIFIED: lod/model.rs, ADR-0015 decision 3]`. ADR-0015 measured the hex descent p95 rising 11.0 to 13.7-15.4 ms when v2 added a ridge layer (wider bound, more nodes alive: hex max nodes visited 907 to 1221) `[CITED: ADR-0015 Evidence]`. v3 with a 3500 m mountain layer will raise `residual[L]` and `max_relief` more than v2 did. The 8 ms bound (v2 now ~4.1 ms `[CITED: 07.1-27-SUMMARY]`) is therefore protected by the **tightness of the new Lipschitz bounds and the mask gate**, not by octave count. This must be measured early (Wave 1), not at the end.

**F3 (worker cost): cost is linear in noise octaves, so the +50 percent budget is an octave budget.** v1 = 22 octaves (base 5 + detail 14 + mask 3), v2 = 27, v2/v1 = 1.227 which matches the recorded +23 percent `[VERIFIED: terrain.rs defaults + ADR-0015]`. The Phase 8 mesh bench ran with `TerrainParams::default()` (v1) `[VERIFIED: planetary-core/benches/mesh.rs]`: hexagon L8 n16 2.84 ms, cube patch n32 1.56-1.60 ms `[CITED: 08-BENCH-RESULTS.md]`, i.e. about 0.129 ms and 0.071 ms per octave per mesh. Derived v2 baseline: hexagon ~3.49 ms, cube ~1.92 ms. The +50 percent ceiling is v2 x 1.5 = hexagon 5.24 ms (+13.5 octaves, 40.5 total), cube 2.87 ms. Recommended v3: 34 octaves (+26 percent over v2), leaving ~24 points of headroom for the larger node count and stencil changes.

**Primary recommendation:** Add generator v3 as two new versioned layers (`planet.hills`, 4 octaves; `planet.mountains`, 5 octaves) gated by the existing 3-octave `mask` (no extra mask octaves), retune the v3 ridge to 3 octaves, keep base/detail/mask/ridge functions and seeds untouched, keep payload layout, tune normals/rock thresholds under `generator_version >= 3` only, extend the generic-free tests (cross-level normal agreement, v3 goldens, terrain image-diff golden), and measure `v3 / v2` on a bench extended with a generator-version axis before touching the app default.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| v3 layers, composition, version dispatch | planetary-core (`terrain.rs`) | - | Planetary-specific; HLD 22.4 forbids it in generic crates |
| LOD error/height bounds for v3 | planetary-core (`lod/model.rs`) | - | Bound must follow the generator or selection under-refines |
| Height-derived normals, border stencil | planetary-core (`mesh.rs`, `hex/mesh.rs`) | - | CPU, worker thread, shares `surface_point` with positions |
| Rock-slope ramp thresholds (colour from slope) | planetary-core (`mesh.rs`) | planetary-terrain (palette) | Depends on normal, which is mesh output |
| Payload encode/decode | planetary-core (`cell_payload.rs`) | planetary-terrain (`uploader.rs`) | Topology-agnostic payload, no render types |
| Normal consumption (N.L) | engine-render-wgpu `lit_textured.wgsl` (existing, unchanged) | planetary-app (light values) | Generic: any lit mesh with normals |
| Sun/ambient tuning for relief readability | planetary-app (`lib.rs` constants) | - | Already app-owned via generic `DirectionalLight` |
| Relief image-diff golden | planetary-app tests (`tests/golden/<os>`) | - | Mesh builder is planetary code; crates/ may not depend on extensions |
| Cost benches | planetary-core benches, planetary-app `planetary_flight` | - | Existing harnesses, add a generator-version axis |

## Standard Stack

No new dependencies. Everything is existing workspace code `[VERIFIED: Cargo workspace, no new crate needed]`.

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `engine_sdk::procedural` (`fbm_3d`, `value_noise_3d`, `layer_seed`, `Generator`, `LayerStack`) | in-repo | Seeded deterministic noise and layer stack | ADR-0008: exact-op noise, golden-pinned; third-party noise crates rejected |
| glam `DVec3` via `engine_sdk` | 0.34.x | f64 normal math | Already used by the meshers |
| criterion | 0.8.2 | Mesh-build cost bench | Existing `benches/mesh.rs` |
| proptest | 1.12 | Residual-bound and seam property tests | Existing pattern (`v2_relief_bound_over_direction_pairs_cube_and_hex`) |
| png (test dep) | existing | Image-diff golden I/O | Pattern in `planetary-app/tests/atmosphere_ring.rs` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| New octave layers for hills/mountains | Re-amplify existing layers (no new octaves) | Zero cost, but changes v1/v2 behaviour only if gated; amplitude of an fbm with halving gain has the same slope per octave, so more amplitude at the same frequencies mostly makes taller but not steeper-looking relief. Rejected as the sole approach; fine as a tuning knob inside v3 params |
| Value noise (current) for mountains | Gradient/simplex noise | Needs a new exact-op noise primitive and new goldens in `engine-procedural` (generic crate churn). Rejected for P0; value-noise blockiness is hidden by the ridge transform and domain gating |
| Analytic-gradient normals at every vertex | Existing grid-difference normals | 4 extra full terrain samples per vertex = ~5x sample cost, violates the +50 percent bound. Rejected |
| Per-vertex packed (octahedral) normal | Plain f32x3 | Saves 8 B of 52 B per vertex but needs a new `VertexLayout` variant in `engine-render` (generic API change) and a shader decode. Rejected for this phase |

**Installation:** none.

## Generator versioning and golden protection (TERR-01, TERR-02)

### How versions are selected today `[VERIFIED: terrain.rs, system.rs, ADR-0015]`
- `TerrainParams.generator_version: u32`; `TerrainParams::default()` is **v1** (so every existing core test/golden exercises v1); `TerrainParams::demo_v2()` = default + `generator_version: 2` (ridge 2500 m, f 12, 5 oct).
- `TerrainConfig::default()` in planetary-terrain uses `TerrainParams::demo_v2()`, so the app and descent suites run v2 (`system.rs:~291-330`; test at `system.rs:1354` asserts `generator_version == 2`, a **test that must be updated** at the default switch, not before).
- `validate()` rejects `generator_version` outside `1..=2` (`terrain.rs`); the test `validation_covers_generator_version_and_ridge_fields` asserts version 3 is rejected `[VERIFIED: tests/generator.rs:269-278]`. **This is not a golden**; it must change to reject 0 and 4. It is the one intentional edit to an existing v1/v2-era test.
- `PlanetTerrain::new` builds the `LayerStack` (cube path) per version and derives `LayerSeeds`; `sample(d)` is the pointwise function the hex path uses; `patch_samples` composes the stack output (cube). Both paths use `compose`/`compose_v2` so results are bit-identical `[VERIFIED: terrain.rs, ADR-0015 decision 2]`.
- `palette_for` picks the 8-row ramp palette for `generator_version >= 2`, so v3 reuses it unchanged `[VERIFIED: system.rs]`.
- `LodModel::for_topology` has an `if generator_version >= 2` residual branch (`lod/model.rs:113`), `mesh.rs` has `< 2` gates at lines 230 and 265, `topology/cube.rs:199` another. Every `>= 2` gate automatically includes v3: audit each one (they mean "v2-or-newer surface attribute v2", which v3 wants, but the *threshold constants* need a v3 branch).
- Seeds: layer seed = `splitmix64(splitmix64(world ^ fnv1a64(name)) ^ version)`, independent of layer index, so **adding layers never perturbs existing layers' seeds** `[CITED: ADR-0008 decision 7]`.

### Golden protection rules (prescriptive)
1. **Additive only.** New functions `hills_at`, `mountains_at`, `compose_v3`, new `HillsLayer`/`MountainsLayer`; `sample()` gets a leading `if generator_version >= 3` branch; the existing v1/v2 branches, `compose`, `compose_v2`, `ridge_at`, `base_at`, `detail_at`, `mask_at` are not edited. New code lives inside the existing `// BORDER-CRITICAL` region (so `banned_ops.rs` scans it) using only `+ - * / floor sqrt abs` and conversion of integers < 2^53 `[CITED: ADR-0008 decision 8]`. The mountain gate is a polynomial smoothstep (`t*t*(3-2t)`), exact-op.
2. **New layer names/versions, untouched old ones.** `planet.hills` and `planet.mountains` with `LAYER_VERSION_V3 = 3` (new constants). v3 keeps `planet.base/detail/mask` (version 1) and `planet.ridge` (version 2) with identical seeds, so a v3 terrain with hills/mountains amplitude forced to zero must reproduce v2 heights bit-for-bit *when the shared params are equal*. Make this a guard test (`v3_without_new_layers_equals_v2_heights`).
3. **Params stay a superset.** Add `hills_*` and `mountains_*` fields to `TerrainParams` with defaults that are inert for v1/v2 (they are ignored there, like the ridge fields are ignored for v1). Because `TerrainParams` derives `PartialEq` and is constructed with struct-update syntax in tests (`..Self::default()`), adding fields is source compatible for `..default()` users; grep for struct literals without `..` (`terrain.rs` `Default` impl only, verified by reading `demo_v2`). A new `TerrainParams::demo_v3()` = `demo_v2()` fields + `generator_version: 3` + v3 retunes (ridge octaves 3, etc.). **Do not change `demo_v2()` or `default()` values.**
4. **Every existing golden stays as a regression test and runs unchanged:** `GOLDEN_HEIGHTS_V1 0xf067ef3fb83fbd54`, `GOLDEN_DIRS_V1 0x3697d55ac59d0aa2`, `GOLDEN_HEIGHTS_V2 0x3096173351c359fb` (1 and 4 threads), `GOLDEN_MESH`, `GOLDEN_HEX_MESH_V3/V4`, palette FNV pins `[CITED: 07.1-25-SUMMARY, ADR-0015 Evidence]`. The ADR-0014/0015 routine of "the v1 goldens are kept as `*_v1` tests" continues. Because `lod/model.rs` and the surface-v2 code are edited, the guard is that the v1/v2 goldens (including a **v2 mesh+normal+uv byte hash**, add one if missing: the existing mesh golden is v1 only) stay green.
5. **Add `GOLDEN_HEIGHTS_V3`** recorded with the same `check_goldens` at 1 and 4 threads (`goldens_match_at_1_and_4_threads_v3`), plus a v3 mesh golden (cube patch and hex cell bytes), plus a cross-platform check by the CI matrix (Windows computed locally; the Linux run is the first confirmation, ADR-0008 decision 9).
6. **Edge: shared surface-v2 helpers.** `ramp_value`, `uv_v2`, `uv_at_level`, `uv_with_normal`, `ROCK_SLOPE_LO/HI` are used by v2. For v3, add `ramp_value_v3` (or pass the version into one function) with its own constants. v2 calls must keep calling the old constants. Test: `surface_v2.rs` (`ramp_formula_endpoints`, `rock_ramp_is_not_inert_on_real_land`) unchanged and green.

### Prior art for the pattern
Plan 07.1-25 did exactly this for v2 (new layer, new version constant, residual branch, `*_v1` goldens kept, `demo_v2`), so the plan can copy its task structure `[VERIFIED: 07.1-25-SUMMARY]`.

## v3 layer design (TERR-01)

### What reads as relief (the quantity to design to)
Relief readability is governed by **slope at mesh resolution** (height amplitude divided by half-wavelength), not by height. v2: ridge 2500 m at f=12 (first feature ~530 km) with halving gain gives ~1 percent gradients, which is the 0.3-1.2 degree slope measured `[CITED: ADR-0015]`. Target for v3 (design values, `[ASSUMED]` until measured; the plan must calibrate with a `measure_real_slopes`-style test as ADR-0015 did):

| Class | Typical land slope at mesh resolution | Share of land |
|-------|---------------------------------------|---------------|
| Plains | < 1 degree | ~40% |
| Hills | 2-8 degrees | ~35% |
| Mountains | 10-30 degrees, p99 ~30 | ~25% |

"Not extreme" (locked): cap p99 land slope near 30 degrees and peak height contribution so total `max_relief` stays about 2x v2 (v2: 3000+800+2500 = 6300 m; v3 target about 9000-11000 m, Earth-like, versus the 6371 km radius).

### Proposed layers (all value-noise fbm via `fbm_3d`, exact-op)
| Layer | Name / version | f | Octaves | Amplitude | Feature size (R/f ... R/(f 2^(o-1))) | Role |
|-------|----------------|---|---------|-----------|----------------------------------------|------|
| base | `planet.base` v1 | 1.5 | 5 | 3000 | unchanged | continents |
| detail | `planet.detail` v1 | 24 | 14 | 800 | unchanged | fine detail |
| mask | `planet.mask` v1 | 3 | 3 | - | unchanged | also the **mountain-range gate** (no extra octaves) |
| ridge | `planet.ridge` v2 | 12 | **3** (was 5) | 2500 -> **1800** | 530 km ... 130 km | broad ranges |
| hills (new) | `planet.hills` v3 | 64 | 4 | 600 | 100 km ... 12 km | rolling hills everywhere on land |
| mountains (new) | `planet.mountains` v3 | 40 | 5 | 3500 | 160 km ... 10 km | ridged mountain belts |

(Values are starting points, `[ASSUMED]`; the plan fixes them by the calibration test and a Wave 0 slope measurement. Frequency x 2^octaves for the largest case is 64 x 16 = 1024, far below the 2^40 validate limit.)

Composition (`compose_v3`, all `+ - * /`):
```
w  = 0.25 + 0.75*mask                       // existing detail weight
g  = smoothstep(0.50, 0.70, mask)           // range gate from the existing mask (0 on ~half the land)
h  = hills_amp * (2*fbm_h - 1) * (1 - 0.6*g)     // hills quieter inside ranges
m  = mountains_amp * ridge_shape(fbm_m)          // r*r, r = 1 - |2 fbm - 1| (same shape fn as ridge)
height = max(sea, base + detail*w + ridge*w + h + m*g)
```
- The gate reuses the **already computed** 3-octave mask: zero extra octaves, and ranges are large-scale features (mask f=3, features ~2000 km) which reads as "mountain ranges at large scale" while the 5-octave ridged noise supplies the individual peaks at 160 km down to 10 km.
- Mask distribution is concentrated near 0.5 for a 3-octave value-noise fbm `[ASSUMED]`, so pick the smoothstep edges from the measured mask percentile (aim for ~25-30 percent of land with g > 0.5), not from the numbers above.
- Sea floor: keep `max(sea_level, ...)`; hills/mountains are added only through the same final `max`, so ocean stays flat (as in v2). Optionally gate hills by `land = clamp((base - sea)/1000, 0, 1)` if hills on the sea floor are wasted work; skipping is an optimisation that must preserve determinism (compute both branches deterministically; do not branch the *noise evaluation* on float comparisons that differ across platforms, branch only on exact-op comparisons of exact-op values, which is safe).

### Cost estimate per layer (measured v2 cost model)
Per-octave all-in cost per mesh, derived from the Phase 8 bench (v1 params) `[CITED: 08-BENCH-RESULTS.md]`: hexagon L8 n16: 2.84 ms / 22 oct = 0.129 ms; cube n32: ~1.58 ms / 22 oct = 0.072 ms. Linear model validated by v1 to v2 = +22.7 percent octaves = reported +23 percent `[VERIFIED: ADR-0015 consequences]`.

| Config | Octaves | vs v2 | Hexagon est. | Cube est. | Within +50% (hex 5.24 ms / cube 2.87 ms) |
|--------|---------|-------|--------------|-----------|------------------------------------------|
| v1 | 22 | -19% | 2.84 ms (measured) | 1.58 ms (measured) | - |
| v2 (baseline) | 27 | 0% | ~3.49 ms (derived) | ~1.94 ms (derived) | baseline |
| **v3 recommended** (ridge 3, hills 4, mountains 5) | 34 | **+26%** | ~4.4 ms | ~2.4 ms | yes |
| v3 max allowed | 40 | +48% | ~5.2 ms | ~2.9 ms | limit |
| hills +4 only, ridge kept 5 | 31 | +15% | ~4.0 ms | ~2.2 ms | yes |
| hills +4, mountains +5, ridge 5 | 36 | +33% | ~4.6 ms | ~2.6 ms | yes |

Additional cost sources outside the linear model, to watch: (a) cube `LayerStack` runs `vertex_dir` once per layer (2 new layers = 2 more passes; small versus octaves); (b) the gate `smoothstep` and compose arithmetic (negligible); (c) **more nodes selected/built** because the LOD bound grows (F2): more cells means more total worker time per descent, not per cell; (d) the border-normal 4 taps and morph parent sampling multiply sample cost and are already inside the per-mesh figure.

**The v2 baseline must be re-measured, not trusted from the derivation:** `benches/mesh.rs` only builds `TerrainParams::default()`. Add `generator_version` as a bench axis (v1, v2, v3) so the +50 percent test compares like with like on the same machine in the same session (07.1-27 itself warns that cross-session comparisons drift: 07.1-25 recorded hex 13.7-15.4 ms where the unmodified code later measured 10.9-11.1) `[CITED: 07.1-27-SUMMARY baseline caveat]`.

### LOD bounds for v3 (must not be skipped)
`LodModel::for_topology` must gain a `generator_version >= 3` branch. Derivation, in the style of the v2 comment (prove each term in the doc comment and pin with the property test over 2000 seeded direction pairs for levels 1..12, cube and hex, `[CITED: ADR-0015 decision 3]`):
- `max_relief()` adds `hills_amplitude + mountains_amplitude` (and v3 uses its own ridge amplitude). `height.max` feeds culling bounds; looser = more nodes visible, so do not over-pad.
- `residual[L]` adds: hills `A_h * 2 V_h * (1 - 0.6 g)` bound by `A_h * 2 V_h` plus gate variation `A_h * 2 * 0.6 * L_g * V_m`... (the planner derives exactly); mountains `A_m * (4 V_m' g_max + r*r_max * L_g * V_m)` with `r*r <= 1`; smoothstep gate Lipschitz constant on `[a,b]` is `1.5/(b-a)`. The gate's Lipschitz constant multiplies `V_m` (mask variation): a narrow gate (b-a = 0.2) gives 7.5x, which inflates the bound and therefore node count. **Keep the gate wide (b-a >= 0.3) or accept a larger bound deliberately.** This is the single biggest lever on F2.
- The bound need not be tight against the actual terrain, only valid; but looseness costs selection time. Measure `residual[L]` against the sampled max difference (the property test already computes it) and report the ratio in the ADR.

## Normals (TERR-03)

### What exists (F1 detail) `[VERIFIED: mesh.rs, hex/mesh.rs, cube.rs]`
- Interior grid vertex: `normalize(cross(pos[i+1]-pos[i-1], pos[j+1]-pos[j-1]))` in f64 (cube), the sector-grid analogue in hex. Free (no extra samples).
- Border vertex (`i==0||j==0||i==n||j==n`, hex `s==0||t==0||s+t==n`): `border_normal_f64(terrain, level, d)`: tangent basis from `d` only, step `delta = (pi/2)/(n * 2^level)`, 4 surface samples, outward-flipped. Depends only on (direction, level), therefore bit-identical across patches and cube faces of the same level.
- Skirt vertices copy the border normal; morph targets carry no second normal (ADR-0014: "the shading normal is not morphed").
- Rock colour (`uv.y` ramp) is a function of that normal and `dir`, level-dependent.

### What is missing for TERR-03 (the real work)
1. **Cross-level agreement is untested and unbounded.** Same-level borders are bit-equal; a level-L border and the coincident level-(L+1) border vertices use `delta_L` vs `delta_{L+1}` (a factor 2), so their normals differ by the terrain's variation between the two stencil scales. v2 terrain is so smooth this never mattered; v3 mountains put real energy at those scales. Success criterion 2 says "agree across patch and LOD seams within the tested tolerance": the tolerance and test do not exist yet.
2. **Border vs interior stencil mismatch within one patch.** Interior uses the actual grid spacing (one cell, varies across the cube face); border uses the constant `(pi/2)/(n 2^L)` (hex also uses this cube constant although the hex sample angle is `LodModel.sample_angle0 / 2^L` per cell and is not `pi/2 / n`). When terrain has energy near cell scale (v3), the border row can shade differently from the first interior row: a visible line along every patch edge even though neighbours agree with each other.
3. **Slope-driven rock thresholds** (`ROCK_SLOPE_LO/HI = 1e-5/1e-4`) saturate immediately on v3 slopes (a 10 degree slope is `1 - cos = 0.015`, 150x the HI threshold), so v3 would be all-rock on mountains and all-grey. Needs v3 constants (suggested LO 0.01 ~8 degrees, HI 0.10 ~26 degrees, to be calibrated with the measured slope distribution).

### Recommended method (v3 only, gated by `generator_version >= 3`)
- **Keep** the grid-difference interior normal and the 4-tap face-independent border stencil (no added samples, no payload layout change, same exact-op rules).
- **Change (v3 only):** the border step becomes `delta_v3(level) = LodModel.theta(level) / n` (equivalently the true angular grid spacing of the topology: `sample_angle0 / 2^level` is the cell angle used by the LOD model; cube `sample_angle0 = pi/2 / n`, hex per `Topology::sample_angle0`). This makes border and interior stencils measure the same scale on both topologies. `border_normal_f64` takes the model (or step) as an additional argument via a new function (`border_normal_step_f64`); the old function and its two callers for v1/v2 stay bit-identical.
- Cost: unchanged (same 4 taps). The hex and cube meshers' only edit is `if v3 { border_normal_step } else { border_normal_f64 }`.
- **Optional, only if the user flight shows shimmer or flat-looking distant relief:** a "half-cell" step for interior-free detail is *not* planned (needs per-vertex taps). Recorded in the ADR as future work together with the deferred colour/procedural bump.

### Seam test design (tolerances)
| Seam | Required property | Test (cube and hex) |
|------|-------------------|---------------------|
| Same level, neighbouring patches/cells (incl. across cube faces, hex pentagons) | **Bit-equal** border normals | extend `border_normals_identical_across_same_level_neighbors` and `hex_seam.rs` with v3 terrain |
| Level L vs L+1 coincident border vertices (2:1 balance) | angle <= `TOL_LOD_DEG` | new: for seeded patches at levels 3..14, compare the child's border-vertex normal with the parent's border normal at the same direction; report max and p99; pin `TOL_LOD_DEG` = measured p99.9 x 1.5 (`[ASSUMED]` starting guess 3 degrees for mountains, 1 degree for hills; Wave 0 measures) |
| Child interior vertex coincident with a parent vertex (morph handover) | same measure as above | same test; this is exactly the lighting step at morph completion (ADR-0014 does not morph normals), so one measurement answers both "LOD seam" and "morph interplay" |
| Interior vs border within one patch | no more than `TOL_ROW_DEG` jump between row 0 and row 1 normals averaged over a patch | new; guards risk 2 above (should be near the interior-to-interior variation) |
| Negative control | perturbing one border normal by 0.1 degree must fail the bit-equal test | pattern exists: `seam_negative_control_perturbed_normal_breaks_equality` in `surface_v2.rs` |

If measured cross-level disagreement exceeds a comfortable value, the available mitigations in order of cost: (1) tame the highest octaves of mountains (fewer octaves or lower gain above the mesh scale), (2) lower the gate amplitude near cell scale, (3) record the limitation as ADR-0014 did and rely on skirts and morph. A level-independent fixed-step normal is rejected (aliasing at coarse levels and temporal shimmer when LOD changes).

### Payload and memory (answers "packing, v5 layout, memory budget")
- No new bytes needed: normal is already 3 f32 in the 52 B vertex. Memory budget impact of normals: **0**.
- Memory impact of v3: none per vertex; the vertex count per patch is unchanged (set by `cells_per_edge`). Selected/held node count rises with the LOD bound (F2); `detail_budget_patches: 1500` caps detail patches `[VERIFIED: system.rs config]`; check bytes in the held-mesh budget at the worst descent frames.
- **If a v5 header is wanted anyway:** v5 = same 76-byte header with a version of 5 plus the `0,0,0` padding bytes (offset 17..20, now required zero) replaced by a `u8 normal_kind` + 2 zero bytes; vertex stays 52 B. `decode_cell_payload` today accepts exactly `version == CELL_PAYLOAD_VERSION` and rejects v3 (test `cell_payload_v3_is_rejected`) `[VERIFIED: cell_payload.rs]`; extend to accept `{4, 5}`, map v4 to `normal_kind = 0` (geometry stencil), keep `encode_cell_payload` writing the version of the generator in use. Payloads are in memory between worker and uploader and never persisted `[VERIFIED: grep: encode/decode used only in planetary-core and tests]`, so there is no on-disk compatibility burden; "older payload versions keep working" then only means the decoder and its tests keep accepting v4 bytes.
- Packed normal (octahedral 2 x i16 or 2 x u8) would cut 4-8 B/vertex (8-15 percent of the 52 B) but requires a new generic `VertexLayout` and shader decode: out of scope (see Alternatives).

## Renderer / shader consumption (TERR-04) and the generic-API question

**No change to the engine render API, `ShaderId`, `VertexLayout` or `lit_textured.wgsl` is required.** `[VERIFIED: engine-render/src/backend.rs, lit_textured.wgsl]`
- `VertexLayout::PosNormalUvMorph` already declares `nrm` at location 1; `vs_morph` writes `o.nrm = draw.normal * i.nrm` and `fs_main` does `normalize(i.nrm)` then `ndl = max(dot(n, -light_dir), 0)`; ambient and light colour come from the generic `FrameLighting`/`DirectionalLight`.
- Changing `lit_textured.wgsl` (even a wrap-lighting or hemispheric-ambient tweak) would change the engine-render-wgpu canary goldens (`lit_quad_directional.png`, `morph_half.png`, `fog_gradient.png` per platform) and is a generic engine change. **Flag:** avoid. Everything the relief needs (sun elevation, intensity, ambient) is already app-owned: `LIGHT_INTENSITY` 1.8, `LIGHT_AMBIENT` 0.025, sun 35 degrees above the local horizon at the target (`sun_direction_for_target`) `[VERIFIED: planetary-app/src/lib.rs]`. A lower sun (20-25 degrees) raises relief contrast; this is a planetary-app constant tweak judged in the human flight, not an API change.
- If a later phase (VIS-04 in Phase 12 adds "a generic blend input") needs a new attribute or a normal morph, that is when a generic `VertexLayout` change is justified; keep it planet-free (e.g. `PosNormalUvMorphN`, a second normal attribute, described only as "second normal to blend toward"). Not needed now. **Recommendation:** do not pre-empt Phase 12 here; bundle any layout change with Phase 12's single layout change.

Options if the user *does* want normal morphing in this phase (not recommended): (a) generic `VertexLayout::PosNormalUvMorphN` + shader entry `vs_morph_n` (engine change, canary goldens untouched if it is additive, but grows the vertex 52 -> 64 B, +23 percent); (b) child normals derived with the parent's stencil step for vertices that coincide with parent vertices, written into the existing `normal` field (no API change; the vertex then shades like the parent from the start and changes at handover instead; trades a pop for a pop). Neither is justified before the measurement above shows the handover step is visible.

## Architecture Patterns

### System flow (data path of one patch/cell)
```
camera --> select_cells (main thread; uses LodModel bounds: residual[L], height.max)   <-- v3 bounds
              |
              v  requests (priority, budget, cancel)
        StreamManager --> worker thread --> build_patch / build_hex_cell
                                              |  terrain.sample(d) [v1|v2|v3 compose]  (hills, mountains gated by mask)
                                              |  interior normal (grid diff)  + border normal (face-independent 4-tap, v3 step)
                                              |  surface attribute v2: ramp(mask, normal.d)  [v3 thresholds]
                                              |  morph targets (parent surface, parent-level ramp)
                                              v
                                       encode_cell_payload (v4 52 B/vertex; normal already present)
              main thread poll <--- CpuReady --> uploader (budgeted) --> GPU mesh (PosNormalUvMorph)
                                                                             |
        FrameLighting (sun, ambient; app-owned) ---------------------------> lit_textured.wgsl: N.L * palette(uv)
```

### Project structure (changes only)
```
extensions/planetary/planetary-core/src/
  terrain.rs        # + hills/mountains params, hills_at, mountains_at, compose_v3, layers, demo_v3, validate 1..=3
  lod/model.rs      # + v3 residual and max_relief
  mesh.rs           # + border_normal_step_f64, ramp_value_v3 / v3 thresholds, version-aware uv_v2
  hex/mesh.rs, topology/cube.rs, morph.rs   # select v3 stencil and thresholds
  cell_payload.rs   # (optional) v5 header flag
planetary-core/tests/  generator.rs, surface_v2.rs, mesh.rs, seam.rs, hex_seam.rs, lod_bounds.rs, + new relief/normal tests
planetary-core/benches/mesh.rs   # generator-version axis
planetary-app/tests/golden/{windows,linux}/terrain_relief_v3.png  + test
```

### Anti-patterns to avoid
- Editing `compose_v2`/`ridge_at`/`border_normal_f64`/`ROCK_SLOPE_*` in place (breaks TERR-02 silently if values coincide on few test points).
- Using `sin/cos/pow/exp` for ridges or gates (banned, ADR-0008; `banned_ops.rs` scans `src`).
- Normal from a screen-space derivative in the shader (rejected in ADR-0015; also colour-bump is deferred).
- Per-vertex full-resolution analytic normals (5x cost).
- Letting amplitude grow without re-deriving `residual[L]`: selection will under-refine (cracks/pop) or, if padded loosely, over-refine (F2).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Noise | New noise primitive | `fbm_3d`/`value_noise_3d` | Golden-pinned exact-op; third-party noise has unstable output (ADR-0008) |
| Per-layer seeding | Ad hoc seeds | `layer_seed(world, name, version)` | Order-independent, additive |
| Seam-safe border normals | New stencil per topology | `border_normal_f64` (+ step variant) | Face- and cell-independent by construction |
| Image-diff | New diff code | pattern in `atmosphere_ring.rs` / `image_diff.rs` (tolerance 2/255, 0.5% bad pixels, `ARC_*` env flags) | Phase 9 added the small-feature negative control |
| Cost measurement | Wall-clock asserts in tests | criterion benches + `planetary_flight --bench-descent` | Wall-clock asserts are flaky (ADR-0008 decision 5) |

## Runtime State Inventory
Not a rename/migration phase. State categories: **Stored data** none (payloads are in-memory only, `[VERIFIED: grep]`); **live service config** none; **OS-registered state** none; **secrets/env vars** none new (`ARC_UPDATE_GOLDENS`, `ARC_REQUIRE_GOLDENS`, `ARC_REQUIRE_ADAPTER` already exist); **build artifacts** none.

## Common Pitfalls

### Pitfall 1: LOD bound mismatch
**What goes wrong:** v3 heights exceed `height.max` or vary more than `residual[L]` -> culling removes visible mountains, under-refinement, cracks beyond skirt depth. **Why:** bound was derived for v2 layers. **Avoid:** v3 branch in `LodModel` and the 2000-pair property test for both topologies; also `v3_heights_stay_within_max_relief` (copy of the v2 test). **Warning signs:** `lod_bounds` failures, mountain tops flicker.

### Pitfall 2: Descent p95 regresses through selection, not workers (F2)
**Why:** larger bound -> more nodes. **Avoid:** wide gate (Lipschitz), re-measure `--bench-descent` per tuning step, compare nodes visited (ADR-0015 reported max nodes visited). Budget: p95 < 8 ms vs v2 ~4.1 ms.

### Pitfall 3: Skirt depth and mountains
`skirt_depth = SKIRT_MARGIN * eps(L-1)` where `eps` is the geometric error from the LOD model `[VERIFIED: mesh.rs]`. A larger `residual` increases `geometric_error`, so skirts deepen automatically; verify there is no visible skirt "curtain" artefact on steep slopes at flight altitudes (human flight item).

### Pitfall 4: Rock/palette saturation
v3 slopes saturate the v2 ramp thresholds; palette height bands (`uv.x = (h - sea)/(max_relief - sea)`) shift with the larger `max_relief` so the snow line moves down in absolute terms. Recalibrate and check `rock_ramp_is_not_inert_on_real_land`-style test for v3 (neither inert nor saturated).

### Pitfall 5: Golden churn
Every v3 change to a constant changes `GOLDEN_HEIGHTS_V3`/mesh/image goldens. **Avoid:** freeze the constants (Wave 1 calibration) before recording any golden; record goldens last. Linux image goldens can only be produced by the CI `workflow_dispatch` step with `ARC_UPDATE_GOLDENS=1` (goldens never come from hardware adapters; the existing process) `[VERIFIED: image_diff.rs header, ADR/09-CONTEXT]`; local runs can write only the Windows (WARP) golden. Plan this as a checkpoint: with `ARC_REQUIRE_GOLDENS=1` in CI, a missing Linux golden fails the build, so the plan must include the CI regeneration step before the phase gate.

### Pitfall 6: Cross-platform float drift
f64 exact-op code is bit-stable across platforms; **f32 conversions and the final f32 normal** are also deterministic. Risks are only introduced by using banned ops or FMA contraction; `mul_add` banned; no `target-cpu=native`. Mesh byte goldens therefore should match across Windows/Linux; image goldens differ per software rasteriser, hence per-platform pngs.

### Pitfall 7: Morph interplay
The normal is not morphed; at morph t=1 the child's position equals the parent surface but its normal is the child's. With mountains the shading step at handover equals the cross-level normal disagreement measured above; also the ramp colour already carries parent-level slope (ADR-0015 decision 6), so colour and normal can be inconsistent at handover by that step. Test and report; accept per ADR-0014 if the step is under tolerance.

## Code Examples

### Version dispatch (pattern; additive branch first)
```rust
// Source: pattern from planetary-core/src/terrain.rs (v2 branch), extended
pub fn sample(&self, d: DVec3) -> TerrainSample {
    let p = &self.params;
    let base = base_at(self.seeds.base, p, d);
    let detail = detail_at(self.seeds.detail, p, d);
    let mask = mask_at(self.seeds.mask, p, d);
    let height = if p.generator_version >= 3 {
        let ridge = ridge_at(self.seeds.ridge, p, d);
        let hills = hills_at(self.seeds.hills, p, d);          // A_h * (2 fbm - 1)
        let mtn = mountains_at(self.seeds.mountains, p, d);    // A_m * r*r
        compose_v3(p, base, detail, mask, ridge, hills, mtn)
    } else if p.generator_version >= 2 { /* unchanged */ } else { /* unchanged */ };
    TerrainSample { height, mask }
}
```

### Exact-op gate and composition
```rust
// exact-op only: + - * / and comparisons on exact-op values
fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
pub fn compose_v3(p: &TerrainParams, base: f64, detail: f64, mask: f64,
                  ridge: f64, hills: f64, mtn: f64) -> f64 {
    let w = 0.25 + 0.75 * mask;
    let g = smoothstep(p.range_gate_lo, p.range_gate_hi, mask);
    (base + detail * w + ridge * w + hills * (1.0 - 0.6 * g) + mtn * g).max(p.sea_level)
}
```

### Cross-level normal agreement test (shape)
```rust
// for seeded patches at levels 3..14: child border/coincident vertex normal vs parent normal
let ang = |a: DVec3, b: DVec3| a.dot(b).clamp(-1.0, 1.0).acos().to_degrees(); // test code only, not in src
assert!(max_angle <= TOL_LOD_DEG, "p99={p99} max={max_angle}");
eprintln!("cross-level normal angle: p50={p50} p99={p99} max={max_angle}"); // recorded in the ADR
```
(`acos` is allowed in test code; `banned_ops.rs` scans `src` only `[VERIFIED: banned_ops.rs scans CARGO_MANIFEST_DIR/src]`.)

### Relief image-diff golden (shape)
Build one real v3 patch with `build_patch` (cube, level ~8, `n = 16`, camera ~50 km above a mountain region found by a deterministic search over seeded directions with max slope), upload via `PosNormalUv` + `LitTextured`, directional light from the low sun, render 64x64 or 96x96 on the software adapter, compare to `tests/golden/<os>/terrain_relief_v3.png` with 2/255 and 0.5 percent as in `atmosphere_ring.rs`. Add two pixel assertions independent of the golden (as that test does): (1) luminance standard deviation of the lit land above a floor (relief visible), (2) the same scene with all normals replaced by `d` (smooth-sphere normals) has measurably lower variance than with height-derived normals (proves the normals, not the colours, make the relief). This is the TERR-04 evidence that survives a missing golden.

## State of the Art

| Old | Current | Impact |
|-----|---------|--------|
| v1: base+detail+mask | v2: + ridge (ADR-0015) | v2 still ~1 percent gradients |
| v2 mesh normals only visible via rock colour | v3: slope large enough that N.L shading reads | relief reads under the 35 degree sun |
| ADR-0014: normal not morphed | unchanged in P11, measured | decide in Phase 12 if needed |

**Deprecated/outdated:** the CONTEXT premise that normals are *added* to the payload (they are not new; see F1).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Cost per octave stays linear for the new layers (~0.129 ms/hexagon, 0.072 ms/cube patch per octave) | Cost estimate | Budget table off; mitigated by Wave 1 bench with a version axis |
| A2 | Layer parameters (f 64/40, A 600/3500/1800, octaves 4/5/3, gate 0.50-0.70) give p99 slope ~30 degrees and 25 percent mountain share | v3 layer design | Relief too flat or too steep; fixed by calibration test, not a design change |
| A3 | Mask (3-octave value noise fbm) is concentrated near 0.5 so gate edges need measured percentiles | Composition | Gate may cover too much/too little land |
| A4 | Cross-level border normal disagreement is a few degrees (TOL_LOD_DEG start 3/1) | Seam tests | Tolerance chosen too tight or loose; pinned from measurement |
| A5 | `descent p95` is mostly selection and so grows with LOD bound looseness more than octaves | F2 | If wrong, worker cost dominates and the +50 percent bench is the binding gate |
| A6 | Wide gate (b-a >= 0.3) keeps the Lipschitz residual inflation tolerable | LOD bounds | Selection cost regresses beyond 8 ms; re-tune amplitudes |
| A7 | Hex `border_normal` using the cube constant `pi/2/(n 2^L)` mismatches the hex grid spacing | Normals | If it already matches, the v3 step variant is unnecessary; verify numerically in Wave 0 |
| A8 | Lowering the sun to 20-25 degrees improves relief readability | Renderer | Purely visual; judged in the human flight |

## Open Questions

1. **D1 (needs user decision): payload v5.** The locked decision says normals are "added" in payload v5; they already exist in v4 (52 B). Options: (a) no bump, document in the ADR (recommended, zero churn, nothing persisted); (b) header-only v5 with `normal_kind` flag, v4 still decoded; (c) packed normal (needs a generic `VertexLayout` change, not recommended).
   - Recommendation: (a); if the user wants traceability, (b).
2. **D2 (needs user decision): exaggeration.** "Not an extreme profile" is locked, but a real-Earth-like profile has 6 km on a 6371 km radius; relief reads from the ground and from low altitude only. Is p99 slope ~30 degrees with Earth-scale height (9-11 km total) acceptable, or does the user want a stylised vertical exaggeration (no ADR yet)? Recommendation: Earth-like heights, steeper-than-Earth local gradients through small feature sizes (10-100 km), no explicit exaggeration factor.
3. **Is the descent p95 < 8 ms measured with workers settled or free-running?** Wave 0 should confirm (`--bench-descent` uses `prepare + gpu_frame`) `[VERIFIED: 07.1-27-SUMMARY: "prepare plus gpu_frame"]`; this means the binding gate is selection cost (F2). Status: answered, see F2. **RESOLVED.**
4. **Generator selection in the CLI.** `planetary_flight` has no `--generator` flag (usage string lists none) `[VERIFIED: examples/planetary_flight.rs:65]`. The bench plan needs `--generator <1|2|3>` (maps to `TerrainParams`/`TerrainConfig`). Small app change; not a planetary-type leak. **RESOLVED (needed).**
5. **Does the render golden need a Linux golden before the phase gate?** Yes (`ARC_REQUIRE_GOLDENS=1` in CI). Linux golden comes from the CI `workflow_dispatch` regeneration; local Windows golden from WARP. **RESOLVED (plan a CI checkpoint).**
6. **Normal tolerance numbers.** Unknown until measured (A4). Wave 0 measurement task. OPEN.
7. **Is the engine render API change needed?** No for TERR-04. **RESOLVED.** A generic change only becomes relevant for normal morph (see Renderer section); not recommended now.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rustc/cargo | build, tests, benches | yes | 1.99.0 / cargo 1.99.0 | - |
| criterion | cost benches | yes (workspace) | 0.8.2 | - |
| Software GPU adapter (WARP on Windows, lavapipe on Linux CI) | image-diff goldens | Windows WARP yes (existing goldens); Linux only on CI | - | `ARC_REQUIRE_ADAPTER` unset skips; CI sets it |
| macOS goldens | image-diff | no (Phase 13 DEBT decision) | - | skipped by design |
| Human with a display | TERR-06 flight | required | - | blocks default switch (checkpoint) |

**Missing with no fallback:** human flight (a plan checkpoint, not automatable). Never kill cargo/rustc processes (project instruction from the caller).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (nextest allowed), proptest 1.12, criterion 0.8.2, image-diff harness with `png` |
| Config file | workspace `Cargo.toml`; env flags `ARC_UPDATE_GOLDENS`, `ARC_REQUIRE_GOLDENS`, `ARC_REQUIRE_ADAPTER` |
| Quick run | `cargo test -p planetary-core --test generator --test surface_v2 --test mesh --test hex_seam --test lod_bounds` |
| Full suite | `cargo test --workspace --locked` (plus `ARC_REQUIRE_GOLDENS=1` in CI) |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TERR-01 | v3 deterministic at 1 and 4 threads; cube stack equals pointwise `sample`; seed sensitive | unit/golden | `cargo test -p planetary-core --test generator v3` | Wave 0 (new tests in existing file) |
| TERR-01 | v3 heights within `max_relief`; relief statistics (height range > v2, slope percentiles in target bands) | unit | `cargo test -p planetary-core --test relief_v3` | Wave 0 new file |
| TERR-01 | `validate()` accepts 1..=3, rejects 0 and 4; v3 params validated | unit | `cargo test -p planetary-core --test generator validation` | edit existing test |
| TERR-02 | v1 and v2 goldens unchanged; v3 with new layers inert equals v2; v2 mesh byte hash | golden | `cargo test -p planetary-core --test generator --test mesh --test hex_mesh --test surface_v2` | exists (+ new v2 mesh golden) |
| TERR-03 | same-level borders bit-equal (v3); cross-level angle <= TOL; interior vs border row; negative control | unit/property | `cargo test -p planetary-core --test normals_v3` | Wave 0 new |
| TERR-03 | LOD residual/max_relief bounds hold for v3 on cube and hex | property | `cargo test -p planetary-core --test lod_bounds v3` | Wave 0 (extend) |
| TERR-04 | relief image-diff golden (+ normals-vs-sphere variance control) | image | `cargo test -p planetary-app --test terrain_relief` | Wave 0 new + goldens (Windows local, Linux via CI) |
| TERR-05 | v3/v2 mesh-build time ratio <= 1.5 hex and cube; descent p95 < 8 ms | bench | `cargo bench -p planetary-core --bench mesh -- --quick` (with generator axis); `cargo run -p planetary-app --release --example planetary_flight -- --bench-descent --frames 600 --generator 3` | Wave 0 (add axis and flag) |
| TERR-05 | rendering never blocks on generation | existing | `cargo test -p planetary-app --test streaming_lag --test app_headless` | exists |
| TERR-06 | default is v3 only after human approval; ADR present | manual + doc | human flight checklist; `xtask` docs policy test for the ADR | checkpoint |

### Sampling Rate
- Per task commit: quick run above (core crate only, under a minute after the first build).
- Per wave merge: full suite on the touched crates; benches once per wave that changes amplitudes.
- Phase gate: full workspace suite green with `ARC_REQUIRE_GOLDENS=1` including a CI run that produced the Linux golden, then the human flight.

### Wave 0 Gaps
- [ ] bench axis `generator_version` in `planetary-core/benches/mesh.rs` and `--generator` flag in `planetary_flight` (measure v2 vs v3 in the same session)
- [ ] measurement test for v3 slope percentiles at mesh resolution and mask percentiles (calibrates gate edges, rock thresholds)
- [ ] measurement test for cross-level and row normal disagreement (pins TOL values)
- [ ] `tests/relief_v3.rs`, `tests/normals_v3.rs`, `planetary-app/tests/terrain_relief.rs`
- [ ] v2 mesh/normal/uv byte golden (TERR-02 guard for the shared helpers touched by v3)

## Security Domain

`security_enforcement` is not set in `.planning/config.json` (absent = enabled), so assessed: this phase is deterministic local computation with no network, auth, session or persistence.

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2/V3/V4 | no | - |
| V5 Input Validation | yes (limited) | `TerrainParams::validate()` extended: finite, positive amplitudes, octaves `1..=24`, `f * 2^o < 2^40`, gate edges `lo < hi`, within `[0, 1]`; `decode_cell_payload` already validates magic, version, id, class, counts, finite floats, padding, range |
| V6 Cryptography | no | hashes are non-cryptographic (ADR-0008 decision 10) |

| Pattern | STRIDE | Mitigation |
|---------|--------|------------|
| Hostile payload bytes (if a v5 flag is added) | Tampering/DoS | strict decoder: unknown `normal_kind` rejected, length exact, finite floats, existing tests extended |
| Division by zero in gate when `hi == lo` | DoS | validate `lo < hi` |
| Unbounded octaves | DoS | existing `1..=24` and `2^40` rules apply to the new layers |

## Sources

### Primary (HIGH confidence, read in this session)
- `extensions/planetary/planetary-core/src/{terrain.rs, mesh.rs, cell_payload.rs, morph.rs, hex/mesh.rs, topology/cube.rs, lod/model.rs}`; `planetary-terrain/src/system.rs`; `planetary-app/src/lib.rs`
- `crates/engine-render/src/backend.rs`, `crates/engine-render-wgpu/src/shaders/lit_textured.wgsl`, `crates/engine-procedural/src/noise.rs`
- `docs/architecture/adr/0008`, `0014`, `0015`; `.planning/phases/08-hardening-decisions/08-BENCH-RESULTS.md`; `.planning/phases/07.1-.../07.1-25-SUMMARY.md`, `07.1-27-SUMMARY.md`
- tests: `planetary-core/tests/{generator,mesh,surface_v2,banned_ops,cell_payload}.rs`, `planetary-app/tests/atmosphere_ring.rs`, `planetary-core/benches/mesh.rs`

### Secondary (MEDIUM)
- Derived cost table (arithmetic on the cited measurements).

### Tertiary (LOW, needs measurement)
- All v3 amplitude/frequency/slope/tolerance numbers (A2-A4, A6, A8).

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH (no new dependencies; all in-repo)
- Architecture and versioning/golden protection: HIGH (pattern proven by plan 07.1-25)
- v3 constants and normal tolerances: LOW-MEDIUM (design estimates; Wave 0 measures)
- Cost: MEDIUM (linear octave model validated once by v1 to v2; v2 baseline derived, must be re-measured)

**Research date:** 2026-11-27
**Valid until:** 30 days (repo-internal; invalidated by changes to `terrain.rs`, `lod/model.rs` or the lit shader)
