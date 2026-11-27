---
created: 2026-11-27
title: Post-Phase-8 visual feedback: LOD texture pop, flat terrain, flight DOF
area: planetary-render
files:
  - extensions/planetary/planetary-app/examples/planetary_flight.rs
  - extensions/planetary/planetary-terrain
---

## Problem

Reported by the user after flying Phase 8 (their words): "i still see texture blurring and then sharpening when moving from one level to another, i think there should be more of transition or opacity boundary between textures, not sure if that is the best approach. the surface is smooth. adding different elevations hills mountains, maybe a gridded bump mapping based on color would help. i would like to have more degrees of freedom when free flying would help with the visuals as well"

1. LOD blur-to-sharp pop at level changes. Geomorphing (ADR-0014) blends vertex positions only, not colour/detail.
2. Terrain reads as smooth: needs more elevation variety (hills, mountains) and bump/normal shading from height or colour.
3. Free-flight camera: more degrees of freedom (roll, 6DOF).

## Solution

TBD. Ideas: cross-fade or morph colour detail between LOD levels (parent/child blend, alpha-dither); more generator layers and amplitude (generator v3); height-derived normals in the lit shader; camera roll and strafe/vertical axes in the example. Candidate for a post-v1.0 phase or a gap-closure phase. Not for Phase 8 scope.
