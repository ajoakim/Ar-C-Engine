//! The cube-sphere quadtree as a [`Topology`].

use core::f64::consts::FRAC_PI_2;
use std::collections::BTreeSet;

use engine_sdk::DVec3;

use crate::adjacency::neighbor;
use crate::face::{Edge, Face};
use crate::lod::{Footprint, LodModel, leaves_of, patch_footprint};
use crate::mapping::{locate, vertex_dir};
use crate::mesh::{
    PatchData, build_patch_data, grid_indices, stencil_normal, uv_at_level, uv_sampled,
    uv_with_normal,
};
use crate::morph::{
    EDGE_SNAP_M, MorphHit, MorphStats, MorphTargets, blend_uv, chord_target, identity_morph,
    project_to_triangle,
};
use crate::patch_id::PatchId;
use crate::terrain::PlanetTerrain;
use crate::topology::{CellMesh, IndexVariant, Piece, Topology};

/// Six face roots, each a `cells_per_edge` x `cells_per_edge` grid per patch.
#[derive(Clone, Debug)]
pub struct CubeSphere {
    n: u32,
    roots: [PatchId; 6],
}

impl CubeSphere {
    #[must_use]
    pub fn new(cells_per_edge: u32) -> CubeSphere {
        CubeSphere {
            n: cells_per_edge,
            roots: Face::ALL.map(PatchId::root),
        }
    }

    #[must_use]
    pub fn cells_per_edge(&self) -> u32 {
        self.n
    }
}

impl Topology for CubeSphere {
    type Cell = PatchId;

    fn name(&self) -> &'static str {
        "cube-sphere"
    }

    fn roots(&self) -> &[PatchId] {
        &self.roots
    }

    fn level(&self, c: PatchId) -> u8 {
        c.level()
    }

    fn raw(&self, c: PatchId) -> u64 {
        c.raw()
    }

    fn from_raw(&self, raw: u64) -> Option<PatchId> {
        PatchId::from_raw(raw).ok()
    }

    fn children(&self, c: PatchId, out: &mut Vec<PatchId>) {
        if let Some(ch) = c.children() {
            out.extend(ch);
        }
    }

    fn parents(&self, c: PatchId, out: &mut Vec<PatchId>) {
        out.extend(c.parent());
    }

    fn primary_parent(&self, c: PatchId) -> Option<PatchId> {
        c.parent()
    }

    fn neighbors(&self, c: PatchId, out: &mut Vec<PatchId>) {
        for e in Edge::ALL {
            out.push(neighbor(c, e).patch);
        }
    }

    fn required_splits(&self, c: PatchId, out: &mut Vec<PatchId>) {
        let Some(parent) = c.parent() else {
            return;
        };
        out.push(parent);
        // Children of `c` sit at level + 1; their edge neighbours may be at most one level
        // coarser, so the ancestor of each neighbour at level - 1 must be split.
        let Some(level) = c.level().checked_sub(1) else {
            return;
        };
        let Some(children) = c.children() else {
            return;
        };
        for p in children {
            for e in Edge::ALL {
                if let Some(g) = neighbor(p, e).patch.ancestor_at(level) {
                    out.push(g);
                }
            }
        }
    }

    fn footprint(&self, c: PatchId) -> Footprint {
        patch_footprint(c)
    }

    fn locate(&self, dir: DVec3, level: u8) -> PatchId {
        locate(dir, level)
    }

    fn sample_angle0(&self) -> f64 {
        FRAC_PI_2 / f64::from(self.n)
    }

    fn pieces(&self, split: &BTreeSet<PatchId>, out: &mut Vec<Piece<PatchId>>) {
        out.extend(
            leaves_of(split)
                .into_iter()
                .map(|cell| Piece { cell, variant: 0 }),
        );
    }

    fn index_variants(&self) -> Vec<IndexVariant> {
        vec![IndexVariant {
            name: "grid",
            vertex_class: 0,
            indices: grid_indices(self.n),
        }]
    }

    fn build_mesh(
        &self,
        c: PatchId,
        terrain: &PlanetTerrain,
        model: &LodModel,
        debug_cells: bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<CellMesh> {
        let data = build_patch_data(terrain, model, c, cancelled)?;
        let targets = cube_morph_targets(terrain, c, &data);
        let mut m = data.mesh;
        let mut morph = if c.level() == 0 {
            identity_morph(&m.vertices)
        } else {
            targets.to_vertices(m.origin)
        };
        if debug_cells {
            let row = ((u32::from(c.level()) * 7).wrapping_add((c.raw() >> 7) as u32) % 16) as f32;
            let v = (row + 0.5) / 16.0;
            for vert in &mut m.vertices {
                vert.uv[1] = v;
            }
            for mv in &mut morph {
                mv.uv[1] = v;
            }
        }
        Some(CellMesh {
            raw: c.raw(),
            vertex_class: 0,
            origin: m.origin,
            vertices: m.vertices,
            morph,
            bound_radius: m.bound_radius,
            skirt_depth: m.skirt_depth,
            height_min: m.height_min,
            height_max: m.height_max,
        })
    }
}

type PosUv = (DVec3, [f32; 2]);

/// Parent-level vertex `(pi, pj)` of the parent of `c`: the child's own even grid vertex when it
/// lies in the child's footprint (it always does for the vertices the targets need), else sampled.
/// Attribute of child grid vertex `(ci, cj)` that coincides with parent vertex `(pi, pj)`, at the
/// slope of the parent level (the v2 ramp depends on the mesh normal). An interior parent vertex
/// has the stencil `pos(+-1)` of the parent grid, which are the child vertices two cells away: no
/// extra samples. Elsewhere (parent border, or the stencil leaves the child grid) the
/// face-independent border stencil of the parent level is sampled.
fn parent_level_uv(
    terrain: &PlanetTerrain,
    c: PatchId,
    data: &PatchData,
    (ci, cj): (u32, u32),
    (pi, pj): (u32, u32),
) -> [f32; 2] {
    let n = terrain.cells_per_edge();
    let w = n as usize + 1;
    let k = (cj as usize) * w + ci as usize;
    let uv = data.uvs[k];
    if terrain.params().generator_version < 2 {
        return uv;
    }
    let parent_interior = (1..n).contains(&pi) && (1..n).contains(&pj);
    let own_stencil =
        (2..=n.saturating_sub(2)).contains(&ci) && (2..=n.saturating_sub(2)).contains(&cj);
    if parent_interior && own_stencil {
        uv_with_normal(
            terrain,
            uv,
            stencil_normal(&data.pos, w, k, 2),
            data.dirs[k],
        )
    } else {
        uv_at_level(
            terrain,
            uv,
            c.level().saturating_sub(1),
            data.dirs[k],
            FRAC_PI_2 / f64::from(n),
        )
    }
}

fn parent_vertex(
    terrain: &PlanetTerrain,
    c: PatchId,
    data: &PatchData,
    pi: u32,
    pj: u32,
    stats: &mut MorphStats,
) -> PosUv {
    let n = terrain.cells_per_edge();
    let (x, y) = c.xy();
    let ci = i64::from(2 * pi) - i64::from(x & 1) * i64::from(n);
    let cj = i64::from(2 * pj) - i64::from(y & 1) * i64::from(n);
    if (0..=i64::from(n)).contains(&ci) && (0..=i64::from(n)).contains(&cj) {
        let k = (cj as usize) * (n as usize + 1) + ci as usize;
        let uv = parent_level_uv(terrain, c, data, (ci as u32, cj as u32), (pi, pj));
        return (data.pos[k], uv);
    }
    stats.parent_samples += 1;
    let p = c.parent().unwrap_or(c);
    let d = vertex_dir(p, n, pi, pj);
    let smp = terrain.sample(d);
    let r = terrain.params().radius + smp.height;
    (
        DVec3::new(d.x * r, d.y * r, d.z * r),
        uv_sampled(terrain, smp, p.level(), d, FRAC_PI_2 / f64::from(n)),
    )
}

/// Morph targets (f64, planet space) of every vertex of cube cell `c`, parallel to its mesh
/// vertices. Roots get identity targets.
///
/// The child grid halves the parent grid: a child vertex with even `(ox*n + i, oy*n + j)` IS a
/// parent vertex (same direction bits, so same sample and position); one odd index lies on a
/// parent lattice line and takes the canonical chord point; both odd lies inside one parent quad
/// and takes the triangle (by the grid's own diagonal rule) that contains its direction.
pub(crate) fn cube_morph_targets(
    terrain: &PlanetTerrain,
    c: PatchId,
    data: &PatchData,
) -> MorphTargets {
    let n = terrain.cells_per_edge();
    let w = n + 1;
    let wu = w as usize;
    let depth = data.mesh.skirt_depth;
    let mut stats = MorphStats::default();
    let mut position: Vec<DVec3> = Vec::with_capacity(data.mesh.vertices.len());
    let mut uv: Vec<[f32; 2]> = Vec::with_capacity(data.mesh.vertices.len());
    let (x, y) = c.xy();
    let (ox, oy) = (x & 1, y & 1);
    let root = c.level() == 0;
    for j in 0..w {
        for i in 0..w {
            let k = (j as usize) * wu + i as usize;
            stats.vertices += 1;
            let own = (data.pos[k], data.uvs[k]);
            if root {
                position.push(own.0);
                uv.push(own.1);
                continue;
            }
            let (ni, nj) = (ox * n + i, oy * n + j);
            let (qi, qj) = (ni / 2, nj / 2);
            let d = data.dirs[k];
            let target: Option<PosUv> = match (ni & 1, nj & 1) {
                (0, 0) => {
                    stats.copied += 1;
                    let uv = parent_level_uv(terrain, c, data, (i, j), (qi, qj));
                    Some((own.0, uv))
                }
                (1, 0) | (0, 1) => {
                    let (qi2, qj2) = if ni & 1 == 1 {
                        (qi + 1, qj)
                    } else {
                        (qi, qj + 1)
                    };
                    let a = parent_vertex(terrain, c, data, qi, qj, &mut stats);
                    let b = parent_vertex(terrain, c, data, qi2, qj2, &mut stats);
                    let t = chord_target(a, b, d);
                    if t.is_some() {
                        stats.chord += 1;
                    }
                    t
                }
                _ => {
                    let v00 = parent_vertex(terrain, c, data, qi, qj, &mut stats);
                    let v10 = parent_vertex(terrain, c, data, qi + 1, qj, &mut stats);
                    let v11 = parent_vertex(terrain, c, data, qi + 1, qj + 1, &mut stats);
                    let v01 = parent_vertex(terrain, c, data, qi, qj + 1, &mut stats);
                    // The grid splits each quad along v00 - v11; the direction lies in one of
                    // the two triangles (the equal-angle map is not linear, so take the one
                    // with the larger minimum edge distance).
                    let h1 = project_to_triangle(v00.0, v10.0, v11.0, d);
                    let h2 = project_to_triangle(v00.0, v11.0, v01.0, d);
                    let score = |h: &Option<MorphHit>| {
                        h.as_ref()
                            .map_or(f64::NEG_INFINITY, MorphHit::min_edge_dist)
                    };
                    let (hit, tri) = if score(&h1) >= score(&h2) {
                        (h1, [v00, v10, v11])
                    } else {
                        (h2, [v00, v11, v01])
                    };
                    hit.and_then(|h| {
                        let md = h.min_edge_dist();
                        if md < -EDGE_SNAP_M {
                            return None;
                        }
                        if md < 0.0 {
                            stats.extrapolated += 1;
                        } else {
                            stats.triangle += 1;
                        }
                        Some((h.point, blend_uv(h.w, [tri[0].1, tri[1].1, tri[2].1])))
                    })
                }
            };
            match target {
                Some((p, u)) => {
                    position.push(p);
                    uv.push(u);
                }
                None => {
                    stats.identity += 1;
                    position.push(own.0);
                    uv.push(own.1);
                }
            }
        }
    }
    for e in Edge::ALL {
        for t in 0..w {
            let (i, j) = e.vertex(n, t);
            let k = (j as usize) * wu + i as usize;
            let (p, u) = (position[k], uv[k]);
            let d = data.dirs[k];
            position.push(DVec3::new(
                p.x - d.x * depth,
                p.y - d.y * depth,
                p.z - d.z * depth,
            ));
            uv.push(u);
        }
    }
    MorphTargets {
        position,
        uv,
        stats,
    }
}

/// Planet-space (f64) morph targets of cube cell `c`, parallel to the vertices of its mesh (grid
/// then skirts). For seam and oracle tests.
#[doc(hidden)]
#[must_use]
pub fn cube_morph_targets_f64(
    terrain: &PlanetTerrain,
    model: &LodModel,
    c: PatchId,
) -> Option<MorphTargets> {
    let data = build_patch_data(terrain, model, c, &|| false)?;
    Some(cube_morph_targets(terrain, c, &data))
}
