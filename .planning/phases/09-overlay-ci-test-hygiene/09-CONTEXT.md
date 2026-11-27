# Phase 9: Overlay, CI & Test Hygiene - Context

**Gathered:** 2026-11-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Close v1.0 review items IN-01..03 and the two overlay display issues found in the Phase 8 flight: DEBT-01 (CI reads the toolchain version from rust-toolchain.toml), DEBT-02 (overlay painter rebuilds on surface format change, accepts a scale factor), DEBT-03 (image-diff absolute bad-pixel cap plus a small-change negative control), DEBT-04 (overlay counts render as integers), DEBT-05 (detail patches used/cap line fixed or relabelled, with a test).
</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion: technical hygiene phase, no user-facing design questions. Constraints: never weaken existing tests or goldens; keep egui confined per deny.toml; keep the sdk_only guard intact; sources are .planning/phases/08-hardening-decisions/08-REVIEW.md (IN-01..03) and .planning/todos/pending/2026-11-27-post-phase8-visual-feedback.md (overlay findings: counts print as floats, detail patches 2026/1500 and 1938/1500 above cap; investigate whether the cap is soft or the budget is not enforced before choosing fix vs relabel).
</decisions>

<code_context>
## Existing Code Insights

- crates/debug-ui (egui panel), crates/engine-sdk/src/overlay.rs (OverlayHost), crates/engine-render-wgpu/src/overlay.rs (painter, pixels_per_point fixed at 1.0), crates/engine-render-wgpu/tests/image_diff.rs (CHANNEL_TOLERANCE 2/255, MAX_BAD_FRACTION 0.5%), .github/workflows/ci.yml (toolchain 1.99.0 hard-coded in two places), DiagnosticsSnapshot types re-exported via engine-sdk.
</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond the success criteria in ROADMAP.md.
</specifics>

<deferred>
## Deferred Ideas

None.
</deferred>
