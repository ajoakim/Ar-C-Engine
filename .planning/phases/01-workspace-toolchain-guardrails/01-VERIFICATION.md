---
phase: 01-workspace-toolchain-guardrails
verified: 2026-11-25T00:00:00Z
status: passed
score: 4/4 must-haves verified locally (hosted CI portion pending)
human_verification:
  - test: "Push repo to a GitHub remote and run .github/workflows/ci.yml on ubuntu-latest and windows-latest"
    expected: "Both matrix jobs and the cargo-deny job go green; the negative-fixture steps (inverted with '!') confirm check-deps and cargo-deny fail on the bad fixtures"
    why_human: "No remote exists, so hosted CI and the Linux build cannot be run here. Only Windows was verified locally."
---

# Phase 1: Workspace, Toolchain & Guardrails Verification Report

**Goal:** Developers can build a workspace whose architectural boundaries are enforced automatically from day one.
**Status:** human_needed. All local checks pass, and only hosted CI is unverified.

| # | Success criterion | Status | Evidence |
|---|---|---|---|
| 1 | Fresh checkout builds with the pinned toolchain (>=1.95, edition 2024) | VERIFIED on Windows, Linux pending CI | `rust-toolchain.toml` pins 1.99.0. Root `Cargo.toml` has edition 2024 and rust-version 1.95. `cargo build --workspace` succeeds. |
| 2 | Coarse crates with documented dependency direction | VERIFIED | 13 crates plus xtask are workspace members. `docs/architecture/dependency-direction.md` exists. `docs_cover_all_crates` and `layer_table_matches_workspace` tests pass. |
| 3 | CI fails on core->extension dependency or wgpu/winit misplacement, shown by a failing negative test | VERIFIED locally, hosted run pending | `cargo xtask check-deps` prints OK for 14 packages. `xtask/tests/fixtures_negative.rs` has 3 tests, all passing, that make both fixtures fail, including a CLI nonzero exit. `cargo deny check bans licenses sources advisories` passes. `ci.yml` has an ubuntu+windows matrix, a cargo-deny job, and inverted negative steps. |
| 4 | ADR template and conventions ADR | VERIFIED | `docs/architecture/adr/0000-template.md` and `0001-conventions.md` exist. ADR-0001 fixes meters (SI), right-handed Y-up with -Z forward, and reversed-Z Depth32Float with NDC [0,1]. The `adr_conventions_present` test passes. |

**Requirements:** WS-01 and WS-02 are satisfied (WS-01 on Windows only until CI runs). WS-03 is satisfied locally with CI pending. WS-04 is satisfied.

## Commands run
- `cargo build --workspace` passed.
- `cargo test --workspace` passed with no failures.
- `cargo xtask check-deps` printed "OK (14 workspace packages checked)".
- `cargo deny check` passed (advisories, bans, licenses, sources).

## Notes (non-blocking)
- `cargo deny` warns about an "unmatched wrapper" for winit and wgpu entries in `deny.toml`. The cause is that the engine-platform and engine-render-wgpu crates don't use those dependencies yet. It is harmless now and will resolve once Phases 3 and later add them.
- The crates are intentionally empty skeletons, which is expected for Phase 1.
- ROADMAP checkboxes and the REQUIREMENTS "Pending" statuses are not yet updated. This is bookkeeping for the orchestrator.

## Gaps
None.

## Update 2026-11-27 (status human_needed -> passed)

The original status above is kept as history. The hosted CI item is closed: GitHub Actions run 46042013890 (head c44af29) concluded success on ubuntu, windows and macos build jobs, cargo-deny and canary, including the inverted negative-fixture steps (evidence: `.planning/phases/08-hardening-decisions/08-11-SUMMARY.md` line 61 and `08-CI-EVIDENCE.md`). Linux build is therefore verified. Remaining stale item, not a gap: `01-VALIDATION.md` is still draft.
