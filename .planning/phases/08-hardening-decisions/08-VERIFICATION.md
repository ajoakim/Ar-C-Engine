---
phase: 08-hardening-decisions
verified: 2026-11-27T00:00:00Z
status: human_needed
score: 5/5 roadmap success criteria verified (code level); 2 human items not explicitly confirmed
overrides_applied: 1
overrides:
  - must_have: "Minimal atmosphere as a shader pass"
    reason: "Delivered as opaque limb-ring mesh plus sun-tinted clear colour/fog (ADR-0019); user accepted the deviation"
    accepted_by: "user"
    accepted_at: "2026-11-27"
human_verification:
  - test: "Windowed run: read startup info and window title for present mode (07.1 C2)"
    expected: "Title/readout shows the active present mode"
    why_human: "Not explicitly confirmed by the user in the flight"
  - test: "Windowed run with --features debug-overlay, press F3, check each field"
    expected: "LOD state, per-budget bars (e.g. detail patches) and frame/prepare ms render and update"
    why_human: "Field-by-field not explicitly confirmed; headless test only covers the toggle hand-off"
---

# Phase 8: Hardening & Decisions Verification Report

**Goal:** Prototype observable, regression-tested, documented, open decisions recorded.
**Re-verification:** No.

## Success criteria

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Minimal atmosphere | VERIFIED (override) | `planetary-app/src/atmosphere.rs` (529 lines); `settings.atmosphere` defaults true, `--no-atmosphere` flag in `examples/planetary_flight.rs`; `tests/atmosphere_ring.rs` passes (2 ok) with Linux and Windows goldens; ADR-0019 |
| 2 | Debug overlays show LOD and budget state | VERIFIED (code), human items open | `crates/debug-ui/src/lib.rs` renders budget progress bars; `engine-render-wgpu/src/overlay.rs`; `overlay_snapshot.rs`; F3 wired at `lib.rs:752`; `tests/overlay_toggle.rs` passes. Overlay is behind the non-default `debug-overlay` feature (CI has a dedicated feature-path job) |
| 3 | CI: unit, integration, criterion, image-diff on software adapter | VERIFIED | `.github/workflows/ci.yml` has build, image-diff (3 OS), bench (non-blocking, continue-on-error), debug-overlay feature, canary, deny jobs. Run 46042013890 (head c44af29) concluded success via `gh`. Goldens: Linux (lavapipe) and Windows (WARP) PNGs committed, none for macOS (job skips, see Warnings). Benches exist in engine-spatial, planetary-core (select, mesh), planetary-app (stream), spikes/ecs-bench |
| 4 | Architecture, assumptions, limitations, SDK usage docs | VERIFIED | `docs/architecture-overview.md` (92), `assumptions.md` (50), `limitations.md` (39), `sdk-usage.md` (198); xtask `docs_policy` tests pass |
| 5 | ADRs for rendering API, ECS, spatial backend, LOD technique cite prototype or benchmark | VERIFIED | ADR-0017 (ECS, measured table from `spikes/ecs-bench`, hecs confined to the spike); amendments (Phase 8) in ADR-0005, 0006, 0010; ADR-0018, 0019 present; README index rows 0017-0019 |

## Guardrails (run locally)

- `cargo test -p planetary-app --test atmosphere_ring --test overlay_toggle`: pass.
- `cargo test -p xtask` (docs_policy, check-deps fixtures): all pass.
- `cargo run -p xtask -- check-deps`: OK (20 packages).
- `deny.toml`: wgpu wrappers include `egui-wgpu`; font licences OFL-1.1 and Ubuntu-font-1.0 allowed, referencing ADR-0018; CI also runs the egui-wgpu-misplaced negative fixture and cargo-deny negative.
- Full 20-minute suite not run; relied on CI run evidence (184 ok per OS).

## Requirements

PLAN-08 (satisfied, ring-mesh deviation accepted), QA-01, QA-02, QA-03, WS-05: satisfied. No orphaned requirements.

## Warnings (non-blocking)

- No macOS goldens; macOS image-diff skips GPU tests (no software adapter). Image-diff is effectively enforced on Linux and Windows only.
- Bench job is non-blocking by design; no performance regression gate.
- ADR-0006 body still says the benchmark is "deferred" (the Phase 8 amendment supplies the evidence).
- bevy_ecs not benchmarked (reasoned in ADR-0017).
- HLD.md intentionally untracked; user visual feedback deferred to a todo (accepted).

## Gaps

None blocking. Status is human_needed only because the two items above (present-mode/title readout and F3 field-by-field) were never explicitly confirmed by the user.
