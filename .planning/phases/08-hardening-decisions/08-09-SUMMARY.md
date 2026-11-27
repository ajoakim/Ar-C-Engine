# Phase 8 Plan 09: Architecture, assumptions, limitations and SDK usage docs Summary

Four published docs under `docs/` (architecture overview, assumptions, limitations, SDK usage guide), each checked by `docs_policy` tests, linked from the README, with no dependency on the untracked HLD.md.

## Commits
- a7865e0 test(08-09): failing docs_policy tests (RED)
- 9108538 docs(08-09): architecture overview and SDK usage guide
- 383a4b6 docs(08-09): assumptions, limitations and README links
- docs commit: this summary, ROADMAP, STATE, REQUIREMENTS

## What was done
- `docs/architecture-overview.md`: layer table by tier (including `debug-ui`, `ecs-bench`, `xtask`), enforced rules, rendering split, extension model and canary, streaming and LOD, diagnostics and overlay, no-ECS decision, and an index of ADR-0001 to ADR-0018. It never cites HLD (a test asserts the string "HLD" is absent from the overview, SDK guide, assumptions and limitations).
- `docs/sdk-usage.md`: how to run `triangle` (no flags, windowed only), `basic_scene` (`--model`, `--headless`) and `planetary_flight` (every flag of its `USAGE` constant, enforced by a test), the `App` trait, `gpu_frame` (ADR-0009), `Runtime::frame` and `frame_with`, `PresentConfig` and the present-mode options, the feature-gated `overlay_frame` (`egui-overlay` / `debug-overlay`, F3), tests, `ARC_*` image-diff variables and per-target bench commands. Snippets are verbatim excerpts with their source path (triangle.rs, basic_scene.rs, overlay_hook.rs) plus one short `PresentConfig` example built from the real types.
- `docs/assumptions.md`: toolchain (Rust 1.99.0, wgpu 30.0.2, winit 0.30.13 from the manifests), conventions and precision, single-planet radius 6,371,000 m, determinism contract, software-adapter goldens, scope exclusions; each item cites its ADR or file.
- `docs/limitations.md`: Phase 07.1 complete-with-caveats (C1-C4), Linux/macOS image goldens pending (plan 11), hosted benches not run (plan 11), atmosphere not built (plan 10; described only as haze), hex trailing cube on convergence (73 vs 50 frames), present-bound Fifo cadence (about 31 ms frames with about 11 ms CPU on the Arc B580), overlay not seen in a window (plan 12), no ECS and its limits (ADR-0017), hash grid update cost 8-24x (ADR-0006 amendment), f32 origin quantisation, unmorphed normal, haze first guesses, ADR-0011 hex caveats, Wayland display handle open, no macOS software adapter, read-only overlay. The CPU goldens are stated as verified on all three OS by hosted run 45947523739; only GPU image goldens are pending.
- README: new `## Documentation` section linking the four docs and the ADR index.
- xtask `docs_policy`: four new tests (overview + SDK guide, SDK names/flags drift guard, assumptions + limitations, README links).

## Verification of facts
Commands in the SDK guide were run or copied from example headers: `basic_scene --headless 30` (10 draws, 30 submits), `planetary_flight --headless 30` (hex, 53 draws, 120 meshes), and the same with `--features debug-overlay` (identical headless output). Version pins, radius and defaults were read from `Cargo.toml`, `rust-toolchain.toml` and `planetary-core/src/terrain.rs`. Numbers in limitations are quoted from the ADRs and phase summaries, not re-measured.

## Checks
- `cargo test -p xtask --locked --test docs_policy`: 21 passed (17 before, 4 added).
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean.
- `cargo test --workspace --locked`: 894 passed, 0 failed, 11 ignored (890 before plus the 4 new docs_policy tests). No flaky tests.

## Deviations from Plan
- TDD gate: the RED commit is `test(08-09)` and the GREEN commits are `docs(08-09)` (the deliverables are documents, not feature code).
- The plan said to tick QA-03 only if complete; it is marked complete in REQUIREMENTS.md because all four named documents now exist and are policy-tested. Plans 10 and 11 may change two limitation items (atmosphere, hosted goldens and benches); plan 12 must re-check `docs/limitations.md`.
- `docs/architecture/dependency-direction.md` and ADR-0001 still contain "HLD" references from earlier phases. Left untouched (out of scope); the new docs do not repeat them. Worth deciding together with the HLD.md question in plan 12.
- STATE.md edited by hand (position block, stopped_at, completed_plans); `state.advance-plan` was not run (known to fail on this file).

## Not run
- No windowed run of any example (including F3). `triangle` was not run. Hosted CI was not run. `cargo deny` was not run (no dependency changes). No push.

## Known Stubs
None.

## Self-Check: PASSED
Files and commits a7865e0, 9108538, 383a4b6 verified present.
