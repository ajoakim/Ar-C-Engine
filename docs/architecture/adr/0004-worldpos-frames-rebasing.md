# ADR-0004: WorldPos, Coordinate Frames and Origin Rebasing

- Status: accepted
- Date: 2026-11-25
- Requirement/Phase: WORLD-02, WORLD-04 / Phase 2

## Context

Planetary-scale worlds put positions at around 6.4e6 m from the origin. An f32 has a ulp of 0.5 m there, so millimeter detail is lost. Later phases (render extraction, planetary rendering) need a precision model that is hard to misuse.

## Decision

- `WorldPos` in `engine-math` (tier 0) is opaque: a private f64 `DVec3` plus a `FrameId`. It has no public fields, no `From<DVec3>` and no `Add`/`Sub` between positions, so frames are never mixed implicitly.
- Coordinate frames are explicit handles in a `FrameRegistry`, each with a parent `DAffine3` transform. Conversion goes through the lowest common ancestor so nearby frames never round-trip through large root coordinates.
- Origin rebasing is an explicit `rebase(frames, store, frame, new_origin)` over a `PositionStore` of registered positions. It shifts the frame transform, compensates child frames and subtracts the origin from stored locals. The trigger policy belongs to the app or a module; engine-math has no automatic trigger.
- Camera-relative f32 output is produced by subtracting in f64 first and then casting.
- Invalid input (non-finite values, unknown frames, singular transforms) returns `MathError`, never panics.

## Alternatives Considered

- Raw f32 positions: rejected, 0.5 m ulp at 6.4e6 m.
- i64 cell plus f32 offset: deferred to a later ADR spike; the opaque type lets internals change without API churn.
- Implicit `Add`/`Sub` on positions: rejected, it hides frame mixing.

## Consequences

- Tests use tolerances (1e-9 m for conversion round trips, 1e-8 m for rebasing at 6.4e6 m) because f64 rebasing is not bit-exact.
- Repeated rebases accumulate bounded drift.
- Callers must register positions in a `PositionStore` for them to be rebased.
- Internals are swappable without changing the public API.

## Evidence

Unit tests in `crates/engine-math/src/frame.rs` and `rebase.rs`, including pairwise offset preservation at 6.4e6 m and 1000 repeated rebases.
