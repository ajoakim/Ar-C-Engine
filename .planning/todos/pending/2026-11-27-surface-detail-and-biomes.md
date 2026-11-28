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

## Proposed order (assistant, 2026-11-27; user asked for the best order of the brainstormed ideas)

Finish v1.1 first: relief_scale knob (11-06B), Phase 12 LOD detail transition (prerequisite for texture work), Phase 13 debt.

Then v1.2 and later, by dependency:
1. Planet profile + seed (size, mass, relief height/sharpness, atmosphere on/off, sea level): every later item reads from it.
2. Terrain realism: more mountain land, ranges, erosion-like shaping (generator v4), driven by the profile.
3. Oceans: sea level datum, water shading, depth colour; cheap (height-based), reuses the atmosphere flag.
4. Biomes as ground colour and classification (desert, field, forest, snow) from height, slope, latitude, moisture; no new draw path.
5. Surface textures + bump/detail normals: texture arrays picked by biome id; needs the LOD transition done so detail does not pop.
6. Vegetation and features (trees, forests): needs an instanced draw path in the render API (generic, no planetary types); after biomes tell where trees go.
7. Clouds: an independent layer like the atmosphere ring; static or slow drift; after oceans and atmosphere flag.
8. Archetypes: rocky, ice, hot/lava, gas giant (virtual surface at a pressure depth); after profile, oceans, biomes, clouds exist.
9. Star systems with one to three stars: last, builds on archetypes.

## Open asset library (user request, 2026-11-27)

User words: "we need a open texture library for terrain, also open meshes for various trees and fauna".

Candidate sources (verify the licence on each asset page at download time; do not trust this list blindly):
- Terrain/material textures (PBR: albedo, normal, roughness, height): ambientCG (CC0), Poly Haven (CC0).
- Trees, plants, rocks, animals as low-poly glTF/GLB: Quaternius (CC0), Kenney (CC0 nature and animal kits), Poly Pizza (mixed CC0 and CC-BY, check each).
- Avoid: CC-BY-NC and CC-BY-ND (not usable in an open-source engine that may be used commercially), anything with unclear provenance, and Sketchfab models unless CC0 or CC-BY with attribution recorded.

Asset policy to decide in the milestone discuss step:
- Prefer CC0 only; allow CC-BY only with an attribution entry. Add `assets/LICENSES.md` (per asset: source URL, author, licence, date fetched, checksum) and a docs_policy-style test that every file under assets/ has an entry.
- Keep assets out of the engine core crates; they live with the planetary extension or an `assets/` folder; size budget (git LFS or download script instead of committing large textures).
- Format: textures as KTX2 or PNG (ktx2 is in v1.1 Future), meshes as glTF 2.0 (the engine already loads glTF via conventional-3d). Trees as instanced meshes need the instanced draw path (step 6 in the proposed order).
- Fauna implies animation (skinning) which is out of scope for P0 unless a milestone adds it; start with static meshes.
