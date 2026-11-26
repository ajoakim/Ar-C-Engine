//! Hex cell mesh: 6 triangular sector grids of side `n` (5 for a pentagon) on the shared sample
//! lattice, outer-edge skirts, diagonal skirts and the 8 shared index variants (HEX-02).
//!
//! Every vertex position is the single planet-space function
//! `terrain.surface_point(sample_dir(lattice point))`, so shared borders agree bit-for-bit in
//! f64 (same level) and even fine border vertices equal the coarse ones (L / L+1). Border normals
//! use the face-independent stencil of the cube mesh ([`crate::mesh::border_normal_f64`]).
//! Skirts hang below the border by `SKIRT_MARGIN * eps(L - 1)` (ADR-0010).
//!
//! Vertex layout of one cell (all counts in `u32`):
//! 1. sectors `k = 0..sectors`, each a triangular grid, rows `s = 0..=n`, `t = 0..=n-s`;
//! 2. outer skirts: for sector `k` the edge `corner_k -> corner_{k+1}`, `n + 1` vertices;
//! 3. (hexagons only) diagonal skirts `d = 0..3`: `corner_d -> centre -> corner_{d+3}`,
//!    `2n + 1` vertices.
//!
//! Index layout of one variant: the sector triangles of its sectors (in variant order), then
//! the outer skirts of those sectors, then (halves only) the diagonal skirt.

use engine_sdk::{DVec3, VertexPosNormalUv};

use super::adjacency::parents;
use super::cell_id::CellId;
use super::mapping::{CellFrame, MAX_SECTOR_N, cell_sample_point, sample_dir};
use super::morph::{HexOwn, hex_morph_targets};
use super::pieces::{VARIANT_FULL_HEX, VARIANT_FULL_PENTAGON, half_variant, variant_sectors};
use crate::lod::LodModel;
use crate::mesh::{SKIRT_MARGIN, border_normal_f64, uv_of};
use crate::morph::identity_morph;
use crate::terrain::PlanetTerrain;
use crate::topology::{CellMesh, IndexVariant};

/// Vertices in one sector grid of side `n`.
pub(super) const fn sector_vertices(n: u32) -> u32 {
    (n + 1) * (n + 2) / 2
}

/// Vertex count of a hexagon: 6 sector grids, 6 outer skirts, 3 diagonal skirts.
#[must_use]
pub fn vertex_count_hex(n: u32) -> u32 {
    6 * sector_vertices(n) + 6 * (n + 1) + 3 * (2 * n + 1)
}

/// Vertex count of a pentagon: 5 sector grids, 5 outer skirts, no diagonals.
#[must_use]
pub fn vertex_count_pentagon(n: u32) -> u32 {
    5 * sector_vertices(n) + 5 * (n + 1)
}

/// `n` must be even and in `2..=MAX_SECTOR_N` (T-07.1-38).
#[must_use]
pub fn valid_sector_n(n: u32) -> bool {
    n.is_multiple_of(2) && (2..=MAX_SECTOR_N).contains(&n)
}

// BORDER-CRITICAL BEGIN
/// Skirt depth of a level-`level` cell: `SKIRT_MARGIN * eps(level - 1)`.
#[must_use]
pub fn hex_skirt_depth(model: &LodModel, level: u8) -> f64 {
    SKIRT_MARGIN * model.geometric_error(level.saturating_sub(1))
}

/// Planet-space position of sample `(s, t)` of `sector` of cell `c` (sector side `n`): the same
/// function the mesh builder uses.
#[must_use]
pub fn hex_vertex_position_f64(
    terrain: &PlanetTerrain,
    c: CellId,
    n: u32,
    sector: u8,
    s: u32,
    t: u32,
) -> DVec3 {
    terrain.surface_point(sample_dir(cell_sample_point(c, n, sector, s, t)))
}

/// Unit direction, terrain sample and surface position of sample `(s, t)` of `sector`.
fn frame_vertex(
    terrain: &PlanetTerrain,
    frame: &CellFrame,
    sector: u8,
    s: u32,
    t: u32,
) -> (DVec3, crate::terrain::TerrainSample, DVec3) {
    let d = sample_dir(frame.sample(sector, s, t));
    let smp = terrain.sample(d);
    let r = terrain.params().radius + smp.height;
    (d, smp, DVec3::new(d.x * r, d.y * r, d.z * r))
}

/// Normal of a vertex on the border of a sector grid (face- and cell-independent).
fn border_normal(terrain: &PlanetTerrain, level: u8, d: DVec3) -> DVec3 {
    border_normal_f64(terrain, level, d)
}

/// Position of a skirt vertex hanging `depth` below the surface position `p` along `d`.
fn skirt_position(p: DVec3, d: DVec3, depth: f64) -> DVec3 {
    DVec3::new(p.x - d.x * depth, p.y - d.y * depth, p.z - d.z * depth)
}

/// True when `(s, t)` lies on the border of a sector grid of side `n`.
fn on_sector_border(n: u32, s: u32, t: u32) -> bool {
    s == 0 || t == 0 || s + t == n
}
// BORDER-CRITICAL END

fn vsub(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn vcross(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn vnormalize(v: DVec3) -> DVec3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len > 0.0 {
        DVec3::new(v.x / len, v.y / len, v.z / len)
    } else {
        DVec3::new(0.0, 0.0, 1.0)
    }
}

/// Index of grid vertex `(k, s, t)`.
pub(super) const fn grid_index(n: u32, k: u32, s: u32, t: u32) -> u32 {
    k * sector_vertices(n) + s * (n + 1) - s * s.saturating_sub(1) / 2 + t
}

/// Index of outer skirt vertex `j` of sector `k` (`j = 0` is `corner_k`).
pub(super) const fn outer_skirt_index(n: u32, sectors: u32, k: u32, j: u32) -> u32 {
    sectors * sector_vertices(n) + k * (n + 1) + j
}

/// Index of diagonal skirt vertex `j` of diagonal `d` (`j = 0` is `corner_d`, `j = n` the centre).
const fn diag_skirt_index(n: u32, sectors: u32, d: u32, j: u32) -> u32 {
    sectors * (sector_vertices(n) + n + 1) + d * (2 * n + 1) + j
}

/// Grid `(sector, s, t)` carrying diagonal-`d` vertex `j` (`0..=2n`).
pub(super) const fn diag_grid(n: u32, d: u32, j: u32) -> (u32, u32, u32) {
    if j <= n {
        (d, n - j, 0)
    } else {
        (d + 3, j - n, 0)
    }
}

/// Builds the mesh of hex cell `c` (sector side `n = terrain.cells_per_edge()`, even, `2..=64`);
/// `None` when cancelled or `n` is invalid. Carries the geomorph targets (07.1-22).
pub fn build_hex_cell(
    c: CellId,
    terrain: &PlanetTerrain,
    model: &LodModel,
    debug_cells: bool,
    cancelled: &dyn Fn() -> bool,
) -> Option<CellMesh> {
    let parts = build_hex_parts(c, terrain, model, debug_cells, cancelled)?;
    let mut ps = Vec::with_capacity(2);
    parents(c, &mut ps);
    let own = HexOwn {
        frame: &parts.frame,
        dirs: &parts.dirs,
        pos: &parts.pos,
        uvs: &parts.uvs,
        depth: parts.mesh.skirt_depth,
    };
    let targets = hex_morph_targets(terrain, c, &ps, &own, cancelled)?;
    let mut mesh = parts.mesh;
    mesh.morph = if c.level() == 0 {
        identity_morph(&mesh.vertices)
    } else {
        targets.to_vertices(mesh.origin)
    };
    if debug_cells {
        let uv1 = mesh.vertices.first().map_or(0.0, |v| v.uv[1]);
        for mv in &mut mesh.morph {
            mv.uv[1] = uv1;
        }
    }
    Some(mesh)
}

/// Planet-space (f64) morph targets of hex cell `c`, parallel to the vertices of its mesh (grid,
/// outer skirts, diagonal skirts). For seam and oracle tests.
#[doc(hidden)]
#[must_use]
pub fn hex_morph_targets_f64(
    c: CellId,
    terrain: &PlanetTerrain,
    model: &LodModel,
) -> Option<crate::morph::MorphTargets> {
    let mut ps = Vec::with_capacity(2);
    parents(c, &mut ps);
    hex_morph_targets_with_parents_f64(c, terrain, model, &ps)
}

/// Like [`hex_morph_targets_f64`] with an explicit parent search order (tests: the order, and a
/// wrong parent as a negative control).
#[doc(hidden)]
#[must_use]
pub fn hex_morph_targets_with_parents_f64(
    c: CellId,
    terrain: &PlanetTerrain,
    model: &LodModel,
    search: &[CellId],
) -> Option<crate::morph::MorphTargets> {
    let parts = build_hex_parts(c, terrain, model, false, &|| false)?;
    let own = HexOwn {
        frame: &parts.frame,
        dirs: &parts.dirs,
        pos: &parts.pos,
        uvs: &parts.uvs,
        depth: parts.mesh.skirt_depth,
    };
    hex_morph_targets(terrain, c, search, &own, &|| false)
}

/// The mesh without morph targets (identity morph): the pre-07.1-22 build, kept for timing and
/// vertex-equality tests.
#[doc(hidden)]
pub fn build_hex_cell_identity_morph(
    c: CellId,
    terrain: &PlanetTerrain,
    model: &LodModel,
    debug_cells: bool,
    cancelled: &dyn Fn() -> bool,
) -> Option<CellMesh> {
    build_hex_parts(c, terrain, model, debug_cells, cancelled).map(|p| p.mesh)
}

struct HexParts {
    mesh: CellMesh,
    frame: CellFrame,
    dirs: Vec<DVec3>,
    pos: Vec<DVec3>,
    uvs: Vec<[f32; 2]>,
}

fn build_hex_parts(
    c: CellId,
    terrain: &PlanetTerrain,
    model: &LodModel,
    debug_cells: bool,
    cancelled: &dyn Fn() -> bool,
) -> Option<HexParts> {
    let n = terrain.cells_per_edge();
    if !valid_sector_n(n) {
        return None;
    }
    let frame = CellFrame::new(c, n);
    let sectors = u32::from(frame.sectors());
    let pentagon = sectors == 5;
    let level = c.level();
    let depth = hex_skirt_depth(model, level);
    let origin = terrain.surface_point(sample_dir(frame.center_point()));
    let per = sector_vertices(n) as usize;

    let mut dirs: Vec<DVec3> = Vec::with_capacity(per * sectors as usize);
    let mut pos: Vec<DVec3> = Vec::with_capacity(per * sectors as usize);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(per * sectors as usize);
    let mut height_min = f64::INFINITY;
    let mut height_max = f64::NEG_INFINITY;
    for k in 0..sectors {
        if cancelled() {
            return None;
        }
        for s in 0..=n {
            for t in 0..=(n - s) {
                let (d, smp, p) = frame_vertex(terrain, &frame, k as u8, s, t);
                height_min = height_min.min(smp.height);
                height_max = height_max.max(smp.height);
                dirs.push(d);
                pos.push(p);
                uvs.push(uv_of(terrain, smp));
            }
        }
    }
    if cancelled() {
        return None;
    }

    let at = |k: u32, s: u32, t: u32| grid_index(n, k, s, t) as usize;
    let mut normals: Vec<DVec3> = Vec::with_capacity(pos.len());
    for k in 0..sectors {
        for s in 0..=n {
            for t in 0..=(n - s) {
                let nrm = if on_sector_border(n, s, t) {
                    border_normal(terrain, level, dirs[at(k, s, t)])
                } else {
                    let a = vsub(pos[at(k, s + 1, t)], pos[at(k, s - 1, t)]);
                    let b = vsub(pos[at(k, s, t + 1)], pos[at(k, s, t - 1)]);
                    vnormalize(vcross(a, b))
                };
                normals.push(nrm);
            }
        }
        if cancelled() {
            return None;
        }
    }

    let total = if pentagon {
        vertex_count_pentagon(n)
    } else {
        vertex_count_hex(n)
    };
    let mut vertices: Vec<VertexPosNormalUv> = Vec::with_capacity(total as usize);
    let mut max_len2 = 0.0f64;
    let mut push = |p: DVec3, nrm: DVec3, uv: [f32; 2]| {
        let o = vsub(p, origin);
        max_len2 = max_len2.max(o.x * o.x + o.y * o.y + o.z * o.z);
        vertices.push(VertexPosNormalUv {
            position: [o.x as f32, o.y as f32, o.z as f32],
            normal: [nrm.x as f32, nrm.y as f32, nrm.z as f32],
            uv,
        });
    };
    for k in 0..pos.len() {
        push(pos[k], normals[k], uvs[k]);
    }
    for k in 0..sectors {
        for j in 0..=n {
            let g = at(k, n - j, j);
            push(skirt_position(pos[g], dirs[g], depth), normals[g], uvs[g]);
        }
    }
    if cancelled() {
        return None;
    }
    if !pentagon {
        for d in 0..3u32 {
            for j in 0..=(2 * n) {
                let (k, s, t) = diag_grid(n, d, j);
                let g = at(k, s, t);
                push(skirt_position(pos[g], dirs[g], depth), normals[g], uvs[g]);
            }
        }
    }
    let bound_radius = (max_len2.sqrt() * 1.0001) as f32;

    if debug_cells {
        let row = ((u32::from(level) * 7).wrapping_add((c.raw() >> 7) as u32) % 16) as f32;
        let v = (row + 0.5) / 16.0;
        for vert in &mut vertices {
            vert.uv[1] = v;
        }
    }
    let morph = identity_morph(&vertices);
    Some(HexParts {
        mesh: CellMesh {
            raw: c.raw(),
            vertex_class: u8::from(pentagon),
            origin,
            vertices,
            morph,
            bound_radius,
            skirt_depth: depth,
            height_min,
            height_max,
        },
        frame,
        dirs,
        pos,
        uvs,
    })
}

/// Appends the two triangles per grid cell of sector `k` (counter-clockwise seen from outside).
fn push_sector(n: u32, k: u32, out: &mut Vec<u32>) {
    for s in 0..n {
        for t in 0..(n - s) {
            let (a, b, c) = (
                grid_index(n, k, s, t),
                grid_index(n, k, s + 1, t),
                grid_index(n, k, s, t + 1),
            );
            out.extend_from_slice(&[a, b, c]);
            if t + 1 < n - s {
                let d = grid_index(n, k, s + 1, t + 1);
                out.extend_from_slice(&[b, d, c]);
            }
        }
    }
}

/// Appends the skirt strip between border vertices `base(j)` and skirt vertices `skirt(j)`,
/// `j = 0..count`. `along_ccw`: increasing `j` runs counter-clockwise seen from outside around the
/// region that owns the strip (the strip then faces away from that region).
fn push_strip(
    count: u32,
    base: impl Fn(u32) -> u32,
    skirt: impl Fn(u32) -> u32,
    along_ccw: bool,
    out: &mut Vec<u32>,
) {
    for j in 0..count {
        let (b0, b1) = (base(j), base(j + 1));
        let (s0, s1) = (skirt(j), skirt(j + 1));
        if along_ccw {
            out.extend_from_slice(&[b0, s0, s1, b0, s1, b1]);
        } else {
            out.extend_from_slice(&[b0, s1, s0, b0, b1, s1]);
        }
    }
}

fn variant_indices(n: u32, variant: u16) -> Vec<u32> {
    let sectors_total: u32 = if variant == VARIANT_FULL_PENTAGON {
        5
    } else {
        6
    };
    let list = variant_sectors(variant);
    let mut out = Vec::new();
    for k in list {
        push_sector(n, u32::from(*k), &mut out);
    }
    for k in list {
        let k = u32::from(*k);
        push_strip(
            n,
            |j| grid_index(n, k, n - j, j),
            |j| outer_skirt_index(n, sectors_total, k, j),
            true,
            &mut out,
        );
    }
    if (2..8).contains(&variant) {
        let d = u32::from((variant - 2) / 2);
        let side = (variant - 2) % 2;
        // Sectors d..d+2 (side 0) lie on the right of corner_d -> centre -> corner_{d+3}, so the
        // strip faces left (reverse winding); the other half lies on the left.
        push_strip(
            2 * n,
            |j| {
                let (k, s, t) = diag_grid(n, d, j);
                grid_index(n, k, s, t)
            },
            |j| diag_skirt_index(n, sectors_total, d, j),
            side == 1,
            &mut out,
        );
    }
    out
}

/// The 8 shared index variants, in the order of [`variant_sectors`]: 0 full hexagon, 1 full
/// pentagon, then `half_variant(d, side)` for `d` in `0..3`, `side` in `0..2`.
#[must_use]
pub fn hex_index_variants(n: u32) -> Vec<IndexVariant> {
    const HALF_NAMES: [&str; 6] = [
        "hex-half-d0-s0",
        "hex-half-d0-s1",
        "hex-half-d1-s0",
        "hex-half-d1-s1",
        "hex-half-d2-s0",
        "hex-half-d2-s1",
    ];
    let mut out = vec![
        IndexVariant {
            name: "hex-full",
            vertex_class: 0,
            indices: variant_indices(n, VARIANT_FULL_HEX),
        },
        IndexVariant {
            name: "pentagon-full",
            vertex_class: 1,
            indices: variant_indices(n, VARIANT_FULL_PENTAGON),
        },
    ];
    for d in 0..3u8 {
        for side in 0..2u8 {
            let v = half_variant(d, side);
            out.push(IndexVariant {
                name: HALF_NAMES[usize::from(v - 2)],
                vertex_class: 0,
                indices: variant_indices(n, v),
            });
        }
    }
    out
}

/// Sector triangle count of a sector grid of side `n` (`n * n`).
#[must_use]
pub const fn sector_triangles(n: u32) -> u32 {
    n * n
}
