---
phase: 03-platform-render-backend-vertical-slice
verified: 2026-11-25T00:00:00Z
status: human_needed
score: 3/4 roadmap truths fully verified automatically; 1 (real window triangle) needs human
human_verification:
  - test: "Run `cargo run -p engine-sdk --example triangle`"
    expected: "Window opens, RGB triangle renders (rotating), key/mouse input is logged, WASD moves camera, Escape/close exits cleanly"
    why_human: "Interactive window and visual output cannot be checked headlessly; deferred by user in plan 03-07"
    status: human_needed
---

# Phase 3 Verification

**Goal:** A window shows geometry rendered through an engine-owned backend boundary, with camera-relative precision proven.

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | App opens window, gets input, renders triangle via public SDK facade | HUMAN_NEEDED (code wired) | `engine-sdk/src/run.rs` run_app wires PlatformWindow loop to WgpuBackend and Runtime; example `triangle.rs` uses only the SDK prelude; ignored GPU smoke test passed on Intel Arc B580/Vulkan (orchestrator); facade tests pass. Real window run not performed. |
| 2 | Same frame runs headless on NullBackend; wgpu/winit confined | VERIFIED | `Runtime<B: RenderBackend>` is generic; `engine-sdk/tests/headless.rs` pass; wgpu/winit only appear in engine-platform, engine-render-wgpu, and sdk run.rs/tests (the SDK's run wiring and a facade guard test), enforced by `check-deps` OK (14 packages) |
| 3 | Frame built by extracting camera-relative RenderScene and executing render graph | VERIFIED | `Runtime::frame` calls `extract` then `compiled.build_frame_packet` then `backend.submit`; `graph.rs` (416 lines) with compile and tests |
| 4 | Camera at 6.4e6 m shows no jitter | VERIFIED | `engine-render/tests/precision.rs` asserts sub-mm (<1e-5) error across 867 items at R=6.4e6, and a naive f32 cast loses >=0.2 m; passes |

Requirements: REND-01, REND-02 (window/input code + translation tests; real run pending human), REND-03, CORE-03, WORLD-03 all supported by code and tests.

Automated checks run: `cargo test --workspace` all green (1 ignored GPU test), `cargo run -p xtask -- check-deps` OK. No TODO/unimplemented found in crates.

Gaps: none. Note: REQUIREMENTS.md checkboxes/traceability still show "Pending" for these IDs and ROADMAP plan checkboxes are unticked; update after the human run.
