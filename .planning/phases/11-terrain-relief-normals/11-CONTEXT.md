# Phase 11: Terrain Relief & Normals - Context

**Gathered:** 2026-11-27 (updated after research with two user answers, D-01 and D-02)
**Status:** Ready for planning

<domain>
## Phase Boundary

Generator v3 gives convincing hills and mountains at several scales; patch normals derive from height data and agree across patch and LOD seams; the terrain shader uses them so relief reads under directional light; cost stays bounded; v3 becomes the app default only after the user flies it and an ADR records layers, normals method and measured cost (TERR-01..06). v1 and v2 reproduce their goldens bit-for-bit. All work in the planetary extension; generic render API gains no planetary concepts.
</domain>

<decisions>
## Implementation Decisions

### User answers after research (2026-11-27)
- **D-01 (payload):** Keep payload v4 and the 52-byte vertex layout. There is NO payload v5. The v4 payload already carries a height-derived per-vertex normal (interior: grid difference of the displaced surface; border: face-independent 4-tap stencil) and `lit_textured.wgsl` already uses it. The ADR documents this. The "payload v5" sentence in the Normals decision below is superseded.
- **D-02 (relief amplitude):** Earth-like relief with no explicit vertical-exaggeration knob: hills about 600 m, mountains about 3.5 km amplitude, land slope p99 about 30 degrees, total relief about 9-11 km. These constants are starting estimates; they are calibrated by the Wave 0 measurements (slope and mask percentiles) before any golden is recorded.

### Relief
- **D-03:** Hills at small scale plus mountain ranges at large scale, several scales, visible height range well above v2. Deterministic for a given seed. Not an extreme, exaggerated profile (see D-02 for numbers).

### Normals
- **D-04:** Per-vertex normals computed from height in the generator from neighbouring heights, sampled across patch edges (shared-border sampling, extending the existing crack-free approach) so lighting has no seams. ~~Adds a normal to the vertex payload (payload v5)~~ superseded by D-01: the normal already exists in payload v4; this phase delivers the cross-LOD/seam tolerance test, the v3 border-step fix (generator >= 3 only) and v3 rock-slope thresholds. Older payload versions keep working and their goldens stay bit-identical. Requirement wording of TERR-03/TERR-04 is satisfied by verification of the existing normal path plus the new tests.

### Cost bound
- **D-05:** Worker time at most +50% versus the v2 baseline and hex descent p95 under 8 ms (v2 hex descent p95 is about 4 ms; v2 cost +23% over v1). Rendering never blocks on generation.

### Bump detail
- **D-06:** Height-based normals only in this phase. The user's idea of colour-based or procedural fine detail bump is deferred (record in ADR as future work).

### Default switch
- **D-07:** v3 becomes the app default only after a human flight confirms the relief; ADR records layers, normals method and measured cost. `TerrainConfig::default()` stays `demo_v2` until that approval.

### Claude's Discretion
Number and frequencies of noise layers, amplitude constants (within D-02), ridge/hills blend, rock-slope threshold recalibration, shader lighting model details (no change to `lit_textured.wgsl` planned), benchmark parameters.
</decisions>

<code_context>
## Existing Code Insights

- Generator v2 (ridge layer) is the app default (ADR-0015; plan 07.1-25); v1 and v2 goldens must stay bit-identical.
- Payload v4 carries a vertex morph attribute (ADR-0014) and the height-derived normal; lit shader exists with directional light; PosColor pipeline.
- Cost baselines: 08-BENCH-RESULTS.md and 07.1-27 summary (hex prepare p95 3.6 ms; descent p95 4.1 ms).
- Image-diff goldens per platform in crates/engine-render-wgpu/tests/golden and planetary-app/tests/golden; ARC_REQUIRE_GOLDENS=1 in CI for Linux and Windows.
</code_context>

<specifics>
## Specific Ideas

User feedback after Phase 8 flight: "the surface is smooth. adding different elevations hills mountains". Todo: .planning/todos/pending/2026-11-27-post-phase8-visual-feedback.md.
</specifics>

<deferred>
## Deferred Ideas

- Colour/procedural fine detail bump mapping (shader-side) - later phase.
- Packed (octahedral) normals and normal morphing (need a generic VertexLayout change) - consider with Phase 12.
</deferred>
