//! The cube-sphere quadtree as a [`Topology`].

use core::f64::consts::FRAC_PI_2;
use std::collections::BTreeSet;

use engine_sdk::DVec3;

use crate::adjacency::neighbor;
use crate::face::{Edge, Face};
use crate::lod::{Footprint, LodModel, leaves_of, patch_footprint};
use crate::mapping::locate;
use crate::mesh::{build_patch_cancellable, grid_indices};
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
        let mut m = build_patch_cancellable(terrain, model, c, cancelled)?;
        if debug_cells {
            let row = ((u32::from(c.level()) * 7).wrapping_add((c.raw() >> 7) as u32) % 16) as f32;
            let v = (row + 0.5) / 16.0;
            for vert in &mut m.vertices {
                vert.uv[1] = v;
            }
        }
        Some(CellMesh {
            raw: c.raw(),
            vertex_class: 0,
            origin: m.origin,
            vertices: m.vertices,
            bound_radius: m.bound_radius,
            skirt_depth: m.skirt_depth,
            height_min: m.height_min,
            height_max: m.height_max,
        })
    }
}
