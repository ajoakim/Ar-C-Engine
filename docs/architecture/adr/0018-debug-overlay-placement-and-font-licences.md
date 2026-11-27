# ADR-0018: Debug Overlay Placement and Font Licences

- Status: accepted
- Date: 2026-11-27
- Requirement/Phase: QA-02 / Phase 8

## Context

The Phase 8 debug overlay (QA-02) draws a `DiagnosticsSnapshot` with egui. Two constraints from the existing guardrails shape where the code lives.

1. `egui-wgpu` depends on `wgpu` directly. The cargo-deny `[bans]` entry for `wgpu` lists `engine-render-wgpu` as its only wrapper, so any other crate that pulls `egui-wgpu` makes `cargo deny check bans` fail. Without a rule of our own, a misplaced `egui-wgpu` would be caught only through that transitive wrapper check.
2. `egui` pulls `epaint_default_fonts`, whose bundled fonts carry the licences `OFL-1.1` (Hack, Noto Emoji, emoji-icon-font) and `Ubuntu-font-1.0` (Ubuntu-Light). Neither was in the `[licenses]` allow list, so `cargo deny check licenses` failed as soon as egui entered the tree.
3. `egui-winit` would add a second path from egui to `winit`, which only `engine-platform` may use.

## Decision

1. `debug-ui` (tier 4) depends on pure `egui` only. It has no wgpu, winit or egui-winit dependency and is tested headlessly through `frame_shapes`.
2. The painter uses `egui-wgpu` and lives only inside `engine-render-wgpu`, behind the cargo feature `egui-overlay`, off by default. The SDK hands the tessellated output to the backend through a feature-gated, GPU-type-free hand-off (planned in plan 08-08).
3. `egui-winit` is not used. `RawInput` is built by hand from the platform events the engine already has. The overlay is read-only (no widget input) and toggled by the F3 key.
4. xtask rule R2 (`wgpu-outside-allowed-crate`) now also covers a direct `egui-wgpu` dependency: it is allowed only in `engine-render-wgpu`. The `egui-wgpu-misplaced` negative fixture must exit 1, and CI runs it as a negative step beside `core-depends-on-ext` and `wgpu-winit-misplaced`. `deny.toml` also lists `egui-wgpu` as a `wgpu` wrapper.
5. Font-licence policy change, a deliberate project-wide decision confirmed by the user: `OFL-1.1` and `Ubuntu-font-1.0` are added to the `deny.toml` allow list, exactly those two, with a comment naming `epaint_default_fonts`. The fonts are embedded data, not linked code, so they impose no copyleft obligation on the engine's Rust code. The edit landed in plan 08-06 before egui entered the dependency tree; this ADR records it, and `cargo deny check` is re-run as a gate in plan 08-07.

## Alternatives Considered

- Own UI pipeline fed by a neutral mesh packet in `engine-render`: avoids `egui-wgpu` entirely but means writing and maintaining a texture-atlas and clipping painter. Rejected for P0; revisit if the egui-wgpu wgpu pin blocks a wgpu upgrade.
- Ship a permissive font instead and disable the egui default fonts: keeps the allow list unchanged, but needs a font asset, loading code and a licence review for that font. Rejected as more work for no behavioural gain.
- `egui-winit` for input: rejected; it links `winit` into a crate other than `engine-platform` and the overlay needs no input beyond F3.
- Rely on the cargo-deny wrapper alone: rejected; the xtask rule fails earlier, with a clearer message, and has a negative fixture.

## Consequences

- Until plan 08-08 adds the painter, no crate depends on `egui-wgpu`, so `cargo deny check` prints a benign unused-wrapper warning for the `egui-wgpu` wrapper. This is a known transitional state and not a failure.
- The allow list is wider by two font licences. Any further licence still needs its own explicit decision.
- The SDK gains a feature-gated hand-off for overlay output; the default build is unchanged.

## Evidence

- Rule unit test `egui_wgpu_only_in_render_wgpu` and fixture tests `fixture_egui_wgpu_misplaced_fails` and `fixture_egui_wgpu_misplaced_cli_exits_one` in `xtask`; `cargo run -p xtask -- check-deps --manifest-path xtask/fixtures/egui-wgpu-misplaced/Cargo.toml` exits 1.
- CI step "Negative - egui-wgpu misplaced (must fail)".
- `cargo deny check` passes with the two font licences and the `egui-wgpu` wrapper.
- Policy test `adr_0018_present` in `xtask/tests/docs_policy.rs`.
