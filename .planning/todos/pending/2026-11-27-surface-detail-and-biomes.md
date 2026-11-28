---
created: 2026-11-27
title: Surface detail and biomes: textures, bump, trees, fields, deserts, oceans, clouds
area: planetary-render
files:
  - extensions/planetary/planetary-terrain
  - extensions/planetary/planetary-app
---

## Problem

User request after flying Phase 11 v3 (their words): "now next step would be detailed textures loading on surface, bump textures, realistic terrain, surface features, trees forests fields deserts, oceans. clouds"

Groups:
1. Surface detail: detailed textures loaded on the surface, bump/detail normal maps (ties to the deferred bump detail of ADR-0020 and the Phase 12 LOD detail transition; texture streaming/ktx2 are in v1.1 Future).
2. Realistic terrain: more varied landforms (erosion-like shapes, more mountain land), driven by the relief_scale/profile config.
3. Surface features and biomes: trees, forests, fields, deserts (biome classification from height, slope, latitude, moisture; instanced vegetation needs a draw path beyond one mesh per patch).
4. Oceans (also in backlog 999.1: sea level, shading).
5. Clouds (a static or slowly animated cloud layer; weather and day-night remain out of scope per CLAUDE.md unless a milestone changes that).

## Solution

TBD. Candidate milestone v1.2 "Living Surface" (biomes, oceans, textures/bump, clouds) before or merged with the planet-from-seed generator (999.1). Constraints: planetary extension only; deterministic from the seed; engine core stays general-purpose (generic additions such as texture arrays or instancing must carry no planetary types); keep rendering from blocking on generation; budgets and the 8 ms descent p95 apply.
