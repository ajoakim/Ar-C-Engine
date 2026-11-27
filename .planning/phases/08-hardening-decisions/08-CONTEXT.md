# Phase 8: Hardening & Decisions - Context

**Gathered:** 2026-11-26
**Status:** Ready for planning

<domain>
## Phase Boundary

The prototype is observable, regression-tested, documented, and its open technical decisions are recorded. Delivers PLAN-08 (minimal atmosphere, first item cut if schedule slips), QA-01 (unit, integration, criterion performance and image-diff tests in CI), QA-02 (debug overlays for LOD and streaming/memory budget), QA-03 (architecture, assumptions, limitations, SDK usage docs) and WS-05 (ADRs for rendering API, ECS, spatial backend, LOD transition, each citing a prototype or benchmark).

</domain>

<decisions>
## Implementation Decisions

### Atmosphere and debug overlays (PLAN-08, QA-02)
- Atmosphere: analytic limb-glow and sky-colour shader pass fed by the existing haze/fog uniforms (ADR-0015 is the precursor and this supersedes the haze). Lives in the planetary extension only.
- Overlay UI: new `debug-ui` crate on egui behind a feature flag; engine core stays free of egui.
- Overlay contents: LOD level histogram, selected/drawn counts, streaming queue, memory and upload budget, frame/prepare times (existing telemetry columns).
- Wiring: planetary app feeds an engine-core diagnostics snapshot so the overlay is not planet-specific.

### CI and regression tests (QA-01)
- Software adapter: lavapipe on Linux CI, WARP on Windows. Image tests skip with a clear message when no adapter is found.
- Image diff: render to texture at fixed size, compare to goldens with per-channel tolerance and max-differing-pixels budget; goldens stored per platform.
- Performance: criterion benches for selection, mesh build and streaming; CI records results and does not hard-fail on timing.
- Goldens on Linux/macOS generated on hosted CI and committed (closes the "pending hosted CI" item from 07.1).

### Docs and decision ADRs (QA-03, WS-05)
- ECS ADR: record "no ECS in P0" with a small hecs vs slotmap-world benchmark so the ADR cites a measurement.
- Existing ADRs 0005/0006/0010: audit for prototype/benchmark evidence; add reference or amendment where missing, do not rewrite.
- Docs set under `docs/`: architecture overview, assumptions, limitations, SDK usage guide, each checked by `docs_policy`.
- Carry-overs: update the stale Phase 7 `07-VERIFICATION.md` (gaps G1-G3 closed by 07.1) and the STATE.md metrics/current-focus.

### Claude's Discretion
Exact atmosphere constants and colours, overlay layout, bench parameters, and doc structure are at Claude's discretion within the above.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- Telemetry columns and per-scope prepare counters in `planetary-app` (07.1-24/27).
- Haze/fog uniforms and `haze_for_altitude` (ADR-0015); present-mode/title readout (ADR-0016).
- `xtask` docs_policy tests, check-deps, check-canary; ADRs 0000-0016; `.github/workflows/ci.yml`.

### Established Patterns
- Crate boundaries enforced by check-deps; engine-render-backend is the only wgpu crate; planetary code only in the planetary extension.
- Goldens recorded from printed test output, FNV-pinned; ADRs added with a docs_policy test.

### Integration Points
- New `debug-ui` crate depends on egui (check egui/wgpu/winit version alignment first, per stack doc).
- Atmosphere pass hooks through the SDK per-frame hook (ADR-0009) and scene fog state.

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond the success criteria — open to standard approaches. Open item from 07.1: Linux/macOS goldens pending hosted CI; plan 26 present-mode/title checks were not human-exercised.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope. Full LUT-based atmospheric scattering is not planned.

</deferred>
