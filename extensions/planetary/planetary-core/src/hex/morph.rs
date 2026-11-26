//! Hex geomorph targets: the parent-level mesh surface at each child vertex direction.
//!
//! A hex child is not nested in its parent's sample lattice the way a cube child is: a centre child
//! shares the parent's sector axes (half spacing), but an edge child sits on the midpoint between
//! two parents with its own sector axes and straddles both. So the target is found by LOCATING
//! the vertex direction in the parent's sector triangles (primary parent first, then the second):
//!
//! 1. per sector, the cone `(centre, corner_k, corner_k+1)` gives the approximate lattice
//!    position `(s, t)` and a sector ranking;
//! 2. the triangles around the seed are tested by direction (a triangle's cone does not depend on
//!    the terrain heights, so no terrain is sampled for rejected triangles);
//! 3. only the chosen triangle's three vertices are evaluated (taken from the child's own samples
//!    when the lattice points coincide, else sampled);
//! 4. when no seeded triangle contains the direction, all triangles of the sector are scanned.
//!
//! Exact-op like the rest of the morph code; not border-critical.

use engine_sdk::DVec3;

use super::cell_id::CellId;
use super::mapping::{CellFrame, LatticePoint, sample_dir};
use super::mesh::{diag_grid, grid_index, outer_skirt_index, sector_vertices};
use crate::mesh::{uv_at_level, uv_sampled};
use crate::morph::{
    EDGE_SNAP_M, MorphHit, MorphStats, MorphTargets, project_to_triangle, resolve_hit,
};
use crate::terrain::PlanetTerrain;

/// A direction whose best sector cone is further than this (fraction of the sector) outside is
/// not in the parent at all. The cone `(centre, corner_k, corner_k+1)` is flat; where a sector
/// spans two icosahedron faces its true outer border bends away from it by up to about a tenth of
/// the sector, hence the generous margin (rejection only saves work, it never decides a hit).
const CONE_REJECT: f64 = 0.25;

/// Best ranked sectors a direction is walked in before it is declared outside the parent.
const WALK_SECTORS: usize = 3;

/// Everything the builder already computed for the child's own grid.
pub(super) struct HexOwn<'a> {
    pub frame: &'a CellFrame,
    pub dirs: &'a [DVec3],
    pub pos: &'a [DVec3],
    pub uvs: &'a [[f32; 2]],
    pub depth: f64,
}

type Vtx = (DVec3, [f32; 2], DVec3);
/// Best candidate so far: (min weight, triangle corners, s, t).
type BestTri = (f64, [(u32, u32); 3], u32, u32);

struct ParentGrid<'a> {
    terrain: &'a PlanetTerrain,
    level: u8,
    frame: CellFrame,
    n: u32,
    sectors: u32,
    dirs: Vec<Option<DVec3>>,
    verts: Vec<Option<Vtx>>,
    /// Canonical lattice point -> index into the child's own grid arrays.
    own_lookup: &'a [(LatticePoint, u32)],
    own: &'a HexOwn<'a>,
    cones: Vec<[DVec3; 3]>,
}

impl<'a> ParentGrid<'a> {
    fn new(
        terrain: &'a PlanetTerrain,
        parent: CellId,
        n: u32,
        own_lookup: &'a [(LatticePoint, u32)],
        own: &'a HexOwn<'a>,
    ) -> ParentGrid<'a> {
        let frame = CellFrame::new(parent, n);
        let sectors = u32::from(frame.sectors());
        let total = (sectors * sector_vertices(n)) as usize;
        let centre = sample_dir(frame.sample(0, 0, 0));
        let cones = (0..sectors)
            .map(|k| {
                let k = k as u8;
                [
                    centre,
                    sample_dir(frame.sample(k, n, 0)),
                    sample_dir(frame.sample(k, 0, n)),
                ]
            })
            .collect();
        ParentGrid {
            terrain,
            level: parent.level(),
            frame,
            n,
            sectors,
            dirs: vec![None; total],
            verts: vec![None; total],
            own_lookup,
            own,
            cones,
        }
    }

    fn dir(&mut self, k: u32, s: u32, t: u32) -> DVec3 {
        let i = grid_index(self.n, k, s, t) as usize;
        if let Some(d) = self.dirs[i] {
            return d;
        }
        let d = sample_dir(self.frame.sample(k as u8, s, t));
        self.dirs[i] = Some(d);
        d
    }

    /// Position, uv and direction of parent vertex `(k, s, t)`.
    fn vertex(&mut self, k: u32, s: u32, t: u32, stats: &mut MorphStats) -> Vtx {
        let i = grid_index(self.n, k, s, t) as usize;
        if let Some(v) = self.verts[i] {
            return v;
        }
        let lp = self.frame.sample(k as u8, s, t);
        let canon = lp.canonical();
        let found = self
            .own_lookup
            .binary_search_by(|(p, _)| p.cmp(&canon))
            .ok()
            .map(|x| self.own_lookup[x].1 as usize);
        let v = if let Some(g) = found {
            let uv = uv_at_level(self.terrain, self.own.uvs[g], self.level, self.own.dirs[g]);
            (self.own.pos[g], uv, self.own.dirs[g])
        } else {
            stats.parent_samples += 1;
            let d = sample_dir(lp);
            let smp = self.terrain.sample(d);
            let r = self.terrain.params().radius + smp.height;
            (
                DVec3::new(d.x * r, d.y * r, d.z * r),
                uv_sampled(self.terrain, smp, self.level, d),
                d,
            )
        };
        self.verts[i] = Some(v);
        v
    }

    /// Direction-only containment test of triangle `(a, b, c)` of sector `k`.
    fn cone_test(&mut self, k: u32, tri: [(u32, u32); 3], d: DVec3, stats: &mut MorphStats) -> f64 {
        stats.cone_tests += 1;
        let a = self.dir(k, tri[0].0, tri[0].1);
        let b = self.dir(k, tri[1].0, tri[1].1);
        let c = self.dir(k, tri[2].0, tri[2].1);
        // Edge distance on the unit sphere scaled to metres.
        let r = self.terrain.params().radius;
        project_to_triangle(a, b, c, d).map_or(f64::NEG_INFINITY, |h| h.min_edge_dist() * r)
    }

    /// Triangle `idx` of the sector: `base = (s, t)`, `lower` selects the `(s+1,t),(s+1,t+1),(s,t+1)`
    /// triangle.
    fn tri_of(s: u32, t: u32, lower: bool) -> [(u32, u32); 3] {
        if lower {
            [(s + 1, t), (s + 1, t + 1), (s, t + 1)]
        } else {
            [(s, t), (s + 1, t), (s, t + 1)]
        }
    }

    /// Walks from the seed cell `(sf, tf)` of sector `k` toward the triangle containing `d`: every
    /// step tests the triangles around the current best base cell and moves to the best one while
    /// the minimum edge distance improves. Returns the best triangle and its score.
    fn walk(
        &mut self,
        k: u32,
        sf: f64,
        tf: f64,
        d: DVec3,
        stats: &mut MorphStats,
    ) -> Option<(f64, [(u32, u32); 3])> {
        let n = self.n;
        let snap = -EDGE_SNAP_M;
        let mut bs = (sf.max(0.0).floor() as u32).min(n.saturating_sub(1));
        let mut bt = (tf.max(0.0).floor() as u32).min(n.saturating_sub(1));
        if bs + bt >= n {
            bt = n - 1 - bs;
        }
        let mut seen: Vec<(u32, u32, bool)> = Vec::new();
        let mut best: Option<BestTri> = None;
        for step in 0..(4 * n + 4) {
            let mut moved = false;
            let mut cands: Vec<(u32, u32, bool)> = Vec::with_capacity(18);
            for ds in 0..3u32 {
                for dt in 0..3u32 {
                    // The seed is usually exact: test only its own two triangles first.
                    if step == 0 && (ds != 1 || dt != 1) {
                        continue;
                    }
                    let (Some(s), Some(t)) = ((bs + ds).checked_sub(1), (bt + dt).checked_sub(1))
                    else {
                        continue;
                    };
                    if s >= n || t >= n || s + t >= n {
                        continue;
                    }
                    for lower in [false, true] {
                        if lower && s + t + 2 > n {
                            continue;
                        }
                        if !seen.contains(&(s, t, lower)) {
                            seen.push((s, t, lower));
                            cands.push((s, t, lower));
                        }
                    }
                }
            }
            for &(s, t, lower) in &cands {
                let tri = Self::tri_of(s, t, lower);
                let score = self.cone_test(k, tri, d, stats);
                if best.as_ref().is_none_or(|b| score > b.0) {
                    best = Some((score, tri, s, t));
                    moved = true;
                }
            }
            let Some((score, _, s, t)) = best else {
                break;
            };
            if score >= snap || !moved {
                break;
            }
            (bs, bt) = (s, t);
        }
        best.map(|b| (b.0, b.1))
    }

    /// Locates `d` in the parent mesh surface: the target and its kind (see `resolve_hit`).
    fn locate(&mut self, d: DVec3, stats: &mut MorphStats) -> Option<((DVec3, [f32; 2]), u8, f64)> {
        let n = self.n;
        let mut order: Vec<(f64, u32, f64, f64)> = Vec::with_capacity(self.sectors as usize);
        for k in 0..self.sectors {
            let [c0, c1, c2] = self.cones[k as usize];
            if let Some(h) = project_to_triangle(c0, c1, c2, d) {
                order.push((
                    h.min_weight(),
                    k,
                    f64::from(n) * h.w[1],
                    f64::from(n) * h.w[2],
                ));
            }
        }
        order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        order.retain(|o| o.0 >= -CONE_REJECT);
        let snap = -EDGE_SNAP_M;
        let mut chosen: Option<(u32, [(u32, u32); 3])> = None;
        for (rank, &(_, k, sf, tf)) in order.iter().take(WALK_SECTORS).enumerate() {
            if rank > 0 {
                stats.extra_walks += 1;
            }
            if let Some((score, tri)) = self.walk(k, sf, tf, d, stats)
                && score >= snap
            {
                chosen = Some((k, tri));
                break;
            }
        }
        let (k, tri) = chosen?;
        let v = tri.map(|(s, t)| self.vertex(k, s, t, stats));
        let hit: MorphHit = project_to_triangle(v[0].0, v[1].0, v[2].0, d)?;
        let md = hit.min_edge_dist();
        if md < snap {
            return None;
        }
        let (t, kind) = resolve_hit(v, d, &hit);
        Some((t, kind, md))
    }
}

/// Morph targets of the grid and skirt vertices of hex cell `c` (same order as the mesh).
/// `None` when cancelled.
pub(super) fn hex_morph_targets(
    terrain: &PlanetTerrain,
    c: CellId,
    parents: &[CellId],
    own: &HexOwn<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Option<MorphTargets> {
    let n = own.frame.n();
    let sectors = u32::from(own.frame.sectors());
    let per = sector_vertices(n);
    let pentagon = sectors == 5;
    let total_grid = (sectors * per) as usize;
    let mut stats = MorphStats::default();
    let mut position: Vec<DVec3> = Vec::with_capacity(total_grid);
    let mut uv: Vec<[f32; 2]> = Vec::with_capacity(total_grid);

    if c.level() == 0 || parents.is_empty() {
        position.extend_from_slice(own.pos);
        uv.extend_from_slice(own.uvs);
        stats.vertices = total_grid as u32;
    } else {
        // Canonical lattice point of every own grid vertex, sorted for lookup.
        let mut lookup: Vec<(LatticePoint, u32)> = Vec::with_capacity(total_grid);
        for k in 0..sectors {
            for s in 0..=n {
                for t in 0..=(n - s) {
                    lookup.push((
                        own.frame.sample(k as u8, s, t).canonical(),
                        grid_index(n, k, s, t),
                    ));
                }
            }
        }
        lookup.sort_by_key(|a| a.0);
        let mut grids: Vec<ParentGrid<'_>> = parents
            .iter()
            .map(|p| ParentGrid::new(terrain, *p, n, &lookup, own))
            .collect();
        for k in 0..sectors {
            if cancelled() {
                return None;
            }
            for s in 0..=n {
                for t in 0..=(n - s) {
                    let g = grid_index(n, k, s, t) as usize;
                    stats.vertices += 1;
                    let d = own.dirs[g];
                    let mut found: Option<(DVec3, [f32; 2])> = None;
                    for (pi, grid) in grids.iter_mut().enumerate() {
                        if let Some((t, kind, md)) = grid.locate(d, &mut stats) {
                            match kind {
                                0 => stats.copied += 1,
                                1 => stats.chord += 1,
                                _ if md < 0.0 => stats.extrapolated += 1,
                                _ => stats.triangle += 1,
                            }
                            if pi > 0 {
                                stats.second_parent += 1;
                            }
                            found = Some(t);
                            break;
                        }
                    }
                    let (p, u) = found.unwrap_or_else(|| {
                        stats.identity += 1;
                        (own.pos[g], own.uvs[g])
                    });
                    position.push(p);
                    uv.push(u);
                }
            }
        }
    }

    let hang = |position: &[DVec3], uv: &[[f32; 2]], g: usize| {
        let (p, d) = (position[g], own.dirs[g]);
        (
            DVec3::new(
                p.x - d.x * own.depth,
                p.y - d.y * own.depth,
                p.z - d.z * own.depth,
            ),
            uv[g],
        )
    };
    let mut extra: Vec<(DVec3, [f32; 2])> = Vec::new();
    for k in 0..sectors {
        for j in 0..=n {
            extra.push(hang(&position, &uv, grid_index(n, k, n - j, j) as usize));
        }
    }
    debug_assert_eq!(
        outer_skirt_index(n, sectors, 0, 0) as usize,
        total_grid,
        "outer skirts follow the grid"
    );
    if !pentagon {
        for d in 0..3u32 {
            for j in 0..=(2 * n) {
                let (k, s, t) = diag_grid(n, d, j);
                extra.push(hang(&position, &uv, grid_index(n, k, s, t) as usize));
            }
        }
    }
    for (p, u) in extra {
        position.push(p);
        uv.push(u);
    }
    Some(MorphTargets {
        position,
        uv,
        stats,
    })
}
