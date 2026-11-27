---
created: 2026-11-27
title: Planet generator from seed: archetypes, water, profile config, star systems
area: planetary
files:
  - extensions/planetary/planetary-terrain
  - extensions/planetary/planetary-app
---

## Problem

User request (their words, during Phase 11 planning): "also add Water, and planet mountain range height sharpness can be set via config, if it has an atmosphere. or not, i want these to be the ability to create a planet from seed size mass adjusted, earth like rocky gas like ice hot lava.. make it easy to simulate a planet from seed value, and later expand to simulate a star system, one two or three stars."

## Solution

TBD. Candidate milestone v1.2 after Phase 13. Parts:
1. Water/ocean: sea level, water shading, behaviour with and without an atmosphere.
2. PlanetProfile config: mountain height and sharpness, atmosphere present or absent, size and mass (gravity, scale).
3. Archetypes from one seed: earth-like, rocky, gas giant, ice, hot/lava. Gas giants have no surface, so need non-surface rendering (new work).
4. Later: star systems with one, two or three stars.

Constraints: all in the planetary extension; deterministic from the seed; engine core untouched (CLAUDE.md); weather/day-night remain out of scope unless the milestone changes that. Builds on Phase 11 generator v3 (Earth-like defaults, no exaggeration knob there) and Phase 12 LOD transition.

## Gas giant design note (user, 2026-11-27)

User words: "gas planets, can be atmosphere and stop at a orbital point where we can deem the pressure too great to pass through essentially and virtual surface".

Interpretation to confirm in discuss: a gas giant is rendered as a deep atmosphere only, with no solid terrain. A "virtual surface" sits at an altitude where pressure is deemed too great to continue, and the camera stops there (a clamp, like the existing surface clearance). The depth could be derived from the planet profile (mass, radius, gas composition) so it is set from the seed. Open questions: how the atmosphere is drawn (banded colour layers, haze density rising with depth), whether the clamp is a hard stop or a soft slowdown, and whether pressure is shown in the debug overlay.
