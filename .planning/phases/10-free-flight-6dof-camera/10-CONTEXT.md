# Phase 10: Free-Flight 6DOF Camera - Context

**Gathered:** 2026-11-27
**Status:** Ready for planning

<domain>
## Phase Boundary

The user can fly the planetary example freely in six degrees of freedom (CAM-01..06): roll about the view axis, strafe, vertical thrust along the camera up axis; orientation drift-free and normalised (f64, camera-relative rendering); orbit-to-surface flight and scripted descent unchanged; no planetary concept in any generic crate (check-deps, sdk_only stay green). 6DOF is kinematic camera control only, not a flight model.
</domain>

<decisions>
## Implementation Decisions

### Placement
- New quaternion free-flight camera lives in the planetary app / example (planetary-app). The conventional-3d FlyCamera is left untouched. Not added to engine-sdk or core.

### Orientation
- Free-flight is the default mode, orientation held as a quaternion (no gimbal lock, renormalised each update). A key toggles a planet-aligned mode that levels the horizon (planet-up). The scripted descent stays planet-aligned. A documented default and toggle satisfy CAM-05.

### Controls
- Keep existing keys: WASD/arrows move, Shift boost, right-mouse look. Add Z/X for roll left/right. Strafe stays on A/D. Vertical stays E/Space (up) and Q/Ctrl (down), acting along the camera up axis in free mode.

### Movement frame
- Free mode: strafe, up and down are camera-relative (spacecraft style). Planet-aligned mode keeps world/planet-up for vertical.

### Claude's Discretion
Toggle key choice, roll rate and sensitivity constants, how mode state is shown (window title or overlay), unit-test design for drift (long scripted roll/pitch/yaw sequence), how the camera integrates with the existing planetary_flight example input handling.
</decisions>

<code_context>
## Existing Code Insights

- extensions/conventional-3d/src/fly_camera.rs: yaw/pitch FlyCamera (f32, up fixed to +Y), must not be changed.
- extensions/planetary/planetary-app/src/descent.rs: scripted descent (pitch_down_radians etc.), must behave as before.
- planetary_flight example reads WASD/arrows, E/Space, Q/Ctrl, Shift, right mouse; position is f64 WorldPos with camera-relative rendering.
</code_context>

<specifics>
## Specific Ideas

User motivation: more degrees of freedom when free flying helps judging the visual work (terrain, LOD transitions) in later phases.
</specifics>

<deferred>
## Deferred Ideas

None.
</deferred>
