# ADR-0019: Minimal Atmosphere - Opaque Limb Ring and Sun-Tinted Sky

- Status: accepted
- Date: 2026-11-27
- Requirement/Phase: PLAN-08 / Phase 8

## Context

[ADR-0015](0015-scale-contrast-and-distance-haze.md) added an altitude-faded distance haze and sky colour as a stand-in and named PLAN-08 as its successor. From orbit that haze gives no limb: the planet's edge meets the space clear colour with nothing between. PLAN-08 is the first item to cut if the schedule slips, so this is the minimum that gives a limb glow from orbit and a sky colour near the surface, with no change to the engine core (HLD 22.4, Core Value: planet numbers stay in the planetary extension).

Engine facts that bound the design (verified in the code): `ShaderId` is a closed enum with two shaders; the pipeline blend state is `REPLACE` only; every `PassPacket` clears colour and depth; depth is reverse-Z (`Greater`, clear 0.0); `SceneState` carries a `DistanceFog` and a `clear_color`; the unlit shader has no fog.

## Deviation from CONTEXT

`08-CONTEXT.md` asked for an "analytic limb-glow and sky-colour shader pass fed by the existing haze/fog uniforms". This ADR does not deliver a shader pass. It delivers an opaque limb-ring mesh plus clear-colour and fog changes, drawn with the existing unlit pipeline. Reasons:

- `ShaderId` is a closed enum. A new atmosphere shader is a change to `engine-render` and `engine-render-wgpu`, and an atmosphere type in the engine core is forbidden by HLD 22.4 and the Core Value.
- The blend state is `REPLACE` only, so a true additive or alpha glow pass over the planet also needs a core blend-mode change on `PipelineDesc`.
- Every pass clears its targets, so a second full-screen sky pass is impossible without core changes either.

The user is asked to accept the ring or defer PLAN-08. The question is put in the plan 12 human flight checkpoint, and the answer is recorded there; until then this ADR is accepted on the engineering facts and the visual result is unjudged.

## Decision

All of it lives in `planetary-app` (`src/atmosphere.rs`); no engine-core, render or SDK type, no shader and no blend mode is added.

1. **Limb ring.** One camera-facing annulus of `VertexPosColor` vertices (64 segments, 384 vertices as a triangle list) in the plane through the planet centre perpendicular to the camera axis, drawn as a normal `RenderItem` through the unlit pipeline. The inner radius comes from the tangent cone: `r_in = d * R / sqrt(d^2 - R^2)`, with `d` the camera distance to the centre and `R` the nominal radius plus the maximum terrain relief, evaluated as `R / sqrt(1 - (R/d)^2)` so a huge `d` cannot overflow. The item is scaled by `0.98 * r_in`, so the inner edge overlaps the planet disc and reverse-Z depth lets the disc occlude it (the ring plane passes through the planet centre, behind the nearest surface). The outer edge is `GLOW_SPREAD` (2) times the real atmosphere height (100 km) beyond the inner edge, in unit-ring terms `1 + 2 * 100 km / R`.
2. **Seamless fade.** Outer vertex colours are exactly the frame's clear colour. Inner vertex colours are the sky colour at that limb point mixed towards the clear colour by `ring_alpha(altitude)`, which is 0 at and below 20 km (`HAZE_FULL_ALTITUDE`), 1 at and above 200 km (`HAZE_NONE_ALTITUDE`) and linear between. Opaque `REPLACE` therefore shows no seam, and below 20 km no ring item is emitted (fog and clear colour take over). With no alpha blending the "alpha" is colour mixing only.
3. **Sky colour.** `atmosphere_for_view(altitude, light, up)` extends `haze_for_altitude`: same density and altitude fade, but the sky colour (fog colour and the clear colour's sky share, kept equal so the horizon matches the sky) is tinted by the sine `s` of the sun elevation at the local vertical. At `s >= 0.30` the result is bit-identical to `haze_for_altitude`; below it the sky warms towards a twilight colour (up to 60 percent at the horizon sun) and dims to 15 percent at `s <= -0.10`. The ring's inner colour uses the same function with the limb point's outward direction as `up`. The default descent puts the sun at 35 degrees above the target horizon, so the tint is zero there.
4. **Per-frame mesh.** Colours change every frame (the clear colour fades with altitude), so the ring mesh is rebuilt each frame in `gpu_frame` after the terrain has rewritten `scene.items`: last frame's handle is destroyed first, then a new one created (ADR-0009; a double destroy is an `Err`). The cost is about 10 KB of vertex data per frame. The app exposes `ring_buffer_count()` (0 or 1) so the terrain leak test counts the ring exactly.
5. **Switches.** `AppSettings::atmosphere` (default on) and the example flag `--no-atmosphere`. Off keeps exactly the pre-existing haze path (`haze_for_altitude`) and emits no ring; `--no-haze` still turns everything off.
6. **Numerical safety.** `ring_inner_radius` returns `None` for non-finite input, `R <= 0` or `d <= R * (1 + 1e-6)`; `ring_frame` returns `None` for an invisible ring, a degenerate pose or a non-finite clear colour; `ring_alpha` and `sky_color` are NaN-safe. Nothing at or inside the surface can produce a non-finite vertex.

This supersedes the haze precursor of ADR-0015 (decision 9) in behaviour only: its decision text is untouched, `DistanceFog` stays a generic render feature, and `--no-atmosphere` restores the ADR-0015 haze exactly.

## Alternatives Considered

- A shader pass (the CONTEXT wording): needs a new `ShaderId` and an atmosphere uniform in the render core; rejected for the reasons in the deviation section.
- Alpha-blended or additive glow: needs a blend mode on `PipelineDesc` plus an alpha unlit shader, both core changes. Kept as the fallback below.
- Full scattering with look-up tables: out of scope (CONTEXT deferred it).
- A static unit ring rescaled each frame: not possible because the vertex colours depend on altitude and sun direction every frame.

## Consequences

- The glow is an opaque band with a vertex-colour gradient. It cannot brighten the planet disc, show terminator scattering on the lit surface, or blend with stars (there are none).
- The ring is only as good as its parameters (height 100 km, spread 2, 64 segments, the twilight constants); they are first guesses.
- One extra draw item and one extra GPU buffer while the ring is shown; `MAX_DRAWS_PER_FRAME` leaves the terrain a margin for it, and the ring is skipped (not an error) if the list is full.
- The dependency table gains a dev-only edge: `planetary-app` has `engine-render`, `engine-render-wgpu` and `png` as dev-dependencies for the image scene (a core crate cannot test extension code, rule R1). The shipped crate's `[dependencies]` and the `sdk_only` guard are unchanged.

## Fallback

If the opaque ring looks wrong in the plan 12 flight, the next step is a blend mode on `PipelineDesc` plus an alpha unlit shader (a core change, out of scope for the minimal atmosphere), or deferring PLAN-08 and keeping the ADR-0015 haze with `--no-atmosphere` behaviour as the default.

## Evidence

- Unit tests in `planetary-app` (`cargo test -p planetary-app --lib atmosphere`): the tangent-cone radius against the closed form and `tan(asin(R/d))`, monotone decrease towards `R` with distance, `None` at `d = R`, `d < R`, `d` within epsilon and for NaN or infinite input, finite radius at `d = 1e300`; ring alpha bands and monotone; vertex count `6 * segments`; outer vertex colour equal to the clear colour for altitudes from 25 km to 1e12 m; all coordinates and colours finite; sky equal to the haze for a high sun and for degenerate vectors; fog validity and the horizon-equals-sky invariant for sun elevations from -80 to 30 degrees.
- Image scene `atmosphere_ring` (`cargo test -p planetary-app --test atmosphere_ring`), WARP golden for Windows: the disc is drawn before the ring and still hides it (overlap pixel equals the disc pixel), the ring pixel outside the disc is neither disc nor clear and blue-dominant, and a pixel beyond the outer edge equals the clear colour. Linux and macOS goldens are plan 11's.
- Visual quality is NOT evidenced here. No human has seen the ring; the plan 12 human flight judges it and records the accept-or-defer answer.
