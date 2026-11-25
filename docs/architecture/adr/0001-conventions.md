# ADR-0001: Engine Conventions (Units, Axes, Handedness, Depth)

- Status: accepted
- Date: 2026-11-25
- Requirement/Phase: WS-04 / Phase 1

## Context

Conventions decided now are cheap; decided later they are rewrites. Every crate (math, world, spatial, render, planetary) must agree on units, axes, handedness and depth before any code depends on them. The engine targets planetary scale, so depth precision and floating-point precision are first-order concerns.

## Decision

- **Units:** SI units: lengths in meters, time in seconds, angles in radians internally. Degrees are allowed only at API/config edges and must carry a `_deg` suffix. HLD `radius_meters: 6371000` is consistent with this.
- **Axes and handedness:** right-handed, Y-up, -Z forward, +X right. This matches glam `look_to_rh` / `perspective_rh` and glTF 2.0. glTF assets are right-handed Y-up with +Z as the asset front; importers convert at load. The planetary extension may use internal planet-centered frames (for example a Z-up ECEF-like frame) but must convert through engine frame transforms; engine world and render code never assumes planetary axes.
- **Depth:** reversed-Z with a Depth32Float depth buffer, clear depth 0.0, depth compare `Greater` (or `GreaterEqual`), and an infinite far plane. NDC depth range is [0, 1] (wgpu/WebGPU).
- **Precision:** world positions are f64; render transforms are camera-relative f32 (the camera position is subtracted on the CPU in f64 before upload).
- **Matrices and winding:** column-major matrices (glam), counter-clockwise front faces, back-face culling by default.
- **Texture coordinates:** UV origin is top-left (WebGPU convention).

## Alternatives Considered

- Z-up right-handed: rejected, diverges from glTF and glam defaults and forces conversions everywhere.
- Left-handed Y-up: rejected, diverges from glTF and glam `_rh` helpers.
- Standard (non-reversed) Z: rejected, depth precision collapses at planetary distances.

## Consequences

- The planetary extension pays the cost of frame conversion.
- The WebGPU NDC [0, 1] and UV-origin assumptions are re-verified in Phase 3 when the first real renderer exists.
- All numeric APIs must document units; degrees never appear without the `_deg` suffix.

## Evidence

- `.planning/research/STACK.md` (reversed-Z, camera-relative rendering).
- HLD.md `radius_meters` (planet radius expressed in meters).
