//! Patch mesh builder: per-patch f64 origin, f32 offsets, shared grid + skirt topology.
//!
//! Every vertex position is the single planet-space function `surface_point(vertex_dir(..))`, so
//! shared borders agree bit-for-bit in f64. Border normals use a stencil that depends only on the
//! unit direction and the level, so they agree across cube faces as well. Skirts hang below the
//! border by `SKIRT_MARGIN * eps(L - 1)` (valid because of the 2:1 balance).

use core::f64::consts::FRAC_PI_2;

use engine_sdk::{DVec3, VertexPosNormalUv};

use crate::face::Edge;
use crate::lod::LodModel;
use crate::mapping::{patch_center_dir, vertex_dir};
use crate::patch_id::PatchId;
use crate::terrain::{PlanetTerrain, TerrainSample};

/// Skirt depth as a multiple of the coarser neighbor's geometric error.
pub const SKIRT_MARGIN: f64 = 1.5;

/// A built patch: f64 origin plus f32 offsets relative to it.
#[derive(Clone, Debug, PartialEq)]
pub struct PatchMesh {
    pub id: PatchId,
    pub origin: DVec3,
    pub vertices: Vec<VertexPosNormalUv>,
    pub indices: Vec<u32>,
    pub bound_radius: f32,
    pub skirt_depth: f64,
    /// Measured min/max of the grid sample heights (skirts excluded).
    pub height_min: f64,
    pub height_max: f64,
}

/// `(n + 1)^2` grid vertices plus `4 * (n + 1)` skirt vertices.
#[must_use]
pub fn vertex_count(n: u32) -> usize {
    let w = (n as usize) + 1;
    w * w + 4 * w
}

/// `6 * n * n` grid indices plus `4 * 6 * n` skirt indices.
#[must_use]
pub fn index_count(n: u32) -> usize {
    let n = n as usize;
    6 * n * n + 4 * 6 * n
}

/// Index topology shared by every patch: grid triangles first, then skirts in `Edge::ALL` order.
#[must_use]
pub fn grid_indices(n: u32) -> Vec<u32> {
    let w = n + 1;
    let grid = |i: u32, j: u32| j * w + i;
    let skirt = |e: Edge, t: u32| w * w + (e as u32) * w + t;
    let mut out = Vec::with_capacity(index_count(n));
    for j in 0..n {
        for i in 0..n {
            let (v00, v10, v11, v01) = (
                grid(i, j),
                grid(i + 1, j),
                grid(i + 1, j + 1),
                grid(i, j + 1),
            );
            out.extend_from_slice(&[v00, v10, v11, v00, v11, v01]);
        }
    }
    for e in Edge::ALL {
        // `t` increasing runs counter-clockwise (seen from outside) on NegV and PosU.
        let ccw = matches!(e, Edge::NegV | Edge::PosU);
        for t in 0..n {
            let (i0, j0) = e.vertex(n, t);
            let (i1, j1) = e.vertex(n, t + 1);
            let (b0, b1) = (grid(i0, j0), grid(i1, j1));
            let (s0, s1) = (skirt(e, t), skirt(e, t + 1));
            if ccw {
                out.extend_from_slice(&[b0, s0, s1, b0, s1, b1]);
            } else {
                out.extend_from_slice(&[b0, s1, s0, b0, b1, s1]);
            }
        }
    }
    out
}

// BORDER-CRITICAL BEGIN
/// Skirt depth of a level-`level` patch: `SKIRT_MARGIN * eps(level - 1)`.
#[must_use]
pub fn skirt_depth(model: &LodModel, level: u8) -> f64 {
    SKIRT_MARGIN * model.geometric_error(level.saturating_sub(1))
}

/// Planet-space position of grid vertex `(i, j)` of patch `id`.
#[must_use]
pub fn vertex_position_f64(terrain: &PlanetTerrain, id: PatchId, i: u32, j: u32) -> DVec3 {
    let d = vertex_dir(id, terrain.cells_per_edge(), i, j);
    terrain.surface_point(d)
}

/// The `n + 1` border positions of `edge` in `Edge::vertex` order.
#[must_use]
pub fn border_positions_f64(terrain: &PlanetTerrain, id: PatchId, edge: Edge) -> Vec<DVec3> {
    let n = terrain.cells_per_edge();
    (0..=n)
        .map(|t| {
            let (i, j) = edge.vertex(n, t);
            vertex_position_f64(terrain, id, i, j)
        })
        .collect()
}

fn vnormalize(x: f64, y: f64, z: f64) -> DVec3 {
    let len = (x * x + y * y + z * z).sqrt();
    if len > 0.0 {
        DVec3::new(x / len, y / len, z / len)
    } else {
        DVec3::new(0.0, 0.0, 1.0)
    }
}

fn vcross(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn vsub(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn tap(terrain: &PlanetTerrain, d: DVec3, t: DVec3, s: f64) -> DVec3 {
    let q = vnormalize(d.x + s * t.x, d.y + s * t.y, d.z + s * t.z);
    terrain.surface_point(q)
}

/// Face-independent unit normal at direction `d` for a patch of level `level`: a central
/// difference stencil whose tangent basis and step depend only on `d` and `level`.
#[must_use]
pub fn border_normal_f64(terrain: &PlanetTerrain, level: u8, d: DVec3) -> DVec3 {
    let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
    let e = if ax <= ay && ax <= az {
        DVec3::new(1.0, 0.0, 0.0)
    } else if ay <= az {
        DVec3::new(0.0, 1.0, 0.0)
    } else {
        DVec3::new(0.0, 0.0, 1.0)
    };
    let c = vcross(d, e);
    let t1 = vnormalize(c.x, c.y, c.z);
    let t2 = vcross(d, t1);
    let delta = FRAC_PI_2 / (f64::from(terrain.cells_per_edge()) * f64::from(1u32 << level));
    let a = vsub(tap(terrain, d, t1, delta), tap(terrain, d, t1, -delta));
    let b = vsub(tap(terrain, d, t2, delta), tap(terrain, d, t2, -delta));
    let c = vcross(a, b);
    let nrm = vnormalize(c.x, c.y, c.z);
    if nrm.x * d.x + nrm.y * d.y + nrm.z * d.z < 0.0 {
        DVec3::new(-nrm.x, -nrm.y, -nrm.z)
    } else {
        nrm
    }
}
// BORDER-CRITICAL END

pub(crate) fn uv_of(terrain: &PlanetTerrain, s: TerrainSample) -> [f32; 2] {
    let p = terrain.params();
    let span = p.max_relief() - p.sea_level;
    let h = if span > 0.0 {
        ((s.height - p.sea_level) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    [h as f32, s.mask.clamp(0.0, 1.0) as f32]
}

/// Slope below which a vertex is not rock (`slope = 1 - n . d`, quadratic in the inclination
/// angle). Calibrated on the measured v2 land slopes (p50 about 1.2e-5, p90 about 5e-5, p99 about
/// 1.1e-4 for levels 8..20, see ADR-0015); the plan's 0.05 (about 18 degrees) never occurs on this
/// terrain, so the ramp would have been inert.
pub const ROCK_SLOPE_LO: f64 = 1.0e-5;
/// Slope at and above which a vertex is fully rock.
pub const ROCK_SLOPE_HI: f64 = 1.0e-4;
/// Rows of the v2 palette (the ramp texel rows).
pub const RAMP_ROWS: f64 = 8.0;

/// Surface attribute v2 ramp value `q = 0.4 * (1 - mask) + 0.6 * rock_t` in `[0, 1]`.
///
/// `normal` and `dir` are unit vectors; `slope = 1 - normal . dir` and
/// `rock_t = clamp((slope - ROCK_SLOPE_LO) / (ROCK_SLOPE_HI - ROCK_SLOPE_LO), 0, 1)`.
/// Only `+ - * /` (ADR-0008 exact-op).
#[must_use]
pub fn ramp_value(mask: f64, normal: DVec3, dir: DVec3) -> f64 {
    let slope = 1.0 - (normal.x * dir.x + normal.y * dir.y + normal.z * dir.z);
    let t = (slope - ROCK_SLOPE_LO) / (ROCK_SLOPE_HI - ROCK_SLOPE_LO);
    let rock_t = t.clamp(0.0, 1.0);
    let m = mask.clamp(0.0, 1.0);
    0.4 * (1.0 - m) + 0.6 * rock_t
}

/// Texel-centre row coordinate of ramp value `q`: `(0.5 + 7 q) / 8`.
#[must_use]
pub fn ramp_row(q: f64) -> f64 {
    (0.5 + 7.0 * q) / RAMP_ROWS
}

/// Surface attribute v2: keeps `uv.x` (normalised height) and replaces `uv.y` (the mask in v1)
/// by the dry-to-rock ramp row. `uv` is the v1 attribute of the vertex.
#[must_use]
pub(crate) fn uv_v2(uv: [f32; 2], normal: DVec3, dir: DVec3) -> [f32; 2] {
    let q = ramp_value(f64::from(uv[1]), normal, dir);
    [uv[0], ramp_row(q) as f32]
}

/// Applies [`uv_v2`] to every vertex of a builder when the generator is v2 or newer.
pub(crate) fn apply_surface_v2(
    terrain: &PlanetTerrain,
    uvs: &mut [[f32; 2]],
    normals: &[DVec3],
    dirs: &[DVec3],
) {
    if terrain.params().generator_version < 2 {
        return;
    }
    for ((uv, n), d) in uvs.iter_mut().zip(normals).zip(dirs) {
        *uv = uv_v2(*uv, *n, *d);
    }
}

/// Attribute of a vertex that exists in no built mesh (morph parent samples): v1 attribute, or for
/// generator v2 the ramp from the face-independent stencil normal of a level-`level` patch.
pub(crate) fn uv_sampled(
    terrain: &PlanetTerrain,
    s: TerrainSample,
    level: u8,
    dir: DVec3,
) -> [f32; 2] {
    uv_at_level(terrain, uv_of(terrain, s), level, dir)
}

/// Re-expresses the v1 attribute `uv` of the vertex at `dir` at the slope of a level-`level`
/// patch (identity for generator v1). The v2 ramp depends on the mesh normal and therefore on the
/// level, so a morph target must carry the parent level's ramp, not the child's.
pub(crate) fn uv_at_level(
    terrain: &PlanetTerrain,
    uv: [f32; 2],
    level: u8,
    dir: DVec3,
) -> [f32; 2] {
    if terrain.params().generator_version < 2 {
        return uv;
    }
    uv_v2(uv, border_normal_f64(terrain, level, dir), dir)
}

/// Builds the mesh of `id`; identical to `build_patch_cancellable` that is never cancelled.
#[must_use]
pub fn build_patch(terrain: &PlanetTerrain, model: &LodModel, id: PatchId) -> PatchMesh {
    build_patch_cancellable(terrain, model, id, &|| false).unwrap_or_else(|| PatchMesh {
        id,
        origin: DVec3::new(0.0, 0.0, 0.0),
        vertices: Vec::new(),
        indices: Vec::new(),
        bound_radius: 0.0,
        skirt_depth: 0.0,
        height_min: 0.0,
        height_max: 0.0,
    })
}

/// Like [`build_patch`], checking `cancelled` before each grid row and before normals/skirts.
pub fn build_patch_cancellable(
    terrain: &PlanetTerrain,
    model: &LodModel,
    id: PatchId,
    cancelled: &dyn Fn() -> bool,
) -> Option<PatchMesh> {
    build_patch_data(terrain, model, id, cancelled).map(|d| d.mesh)
}

/// A built patch plus the f64 grid data the morph targets are derived from (row-major
/// `(n + 1)^2`, same order as the first grid vertices of `mesh.vertices`).
pub(crate) struct PatchData {
    pub mesh: PatchMesh,
    pub dirs: Vec<DVec3>,
    pub pos: Vec<DVec3>,
    pub uvs: Vec<[f32; 2]>,
}

pub(crate) fn build_patch_data(
    terrain: &PlanetTerrain,
    model: &LodModel,
    id: PatchId,
    cancelled: &dyn Fn() -> bool,
) -> Option<PatchData> {
    let n = terrain.cells_per_edge();
    let w = n + 1;
    let wu = w as usize;
    let depth = skirt_depth(model, id.level());
    let origin = terrain.surface_point(patch_center_dir(id));

    let mut samples = terrain.patch_samples(id);
    let fill = samples.len() != wu * wu;

    let mut dirs: Vec<DVec3> = Vec::with_capacity(wu * wu);
    let mut pos: Vec<DVec3> = Vec::with_capacity(wu * wu);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(wu * wu);
    let mut height_min = f64::INFINITY;
    let mut height_max = f64::NEG_INFINITY;
    for j in 0..w {
        if cancelled() {
            return None;
        }
        for i in 0..w {
            let d = vertex_dir(id, n, i, j);
            let s = if fill {
                terrain.sample(d)
            } else {
                samples[(j * w + i) as usize]
            };
            if s.height < height_min {
                height_min = s.height;
            }
            if s.height > height_max {
                height_max = s.height;
            }
            let r = terrain.params().radius + s.height;
            dirs.push(d);
            pos.push(DVec3::new(d.x * r, d.y * r, d.z * r));
            uvs.push(uv_of(terrain, s));
        }
    }
    samples.clear();
    if cancelled() {
        return None;
    }

    let at = |i: u32, j: u32| (j * w + i) as usize;
    let mut normals: Vec<DVec3> = Vec::with_capacity(wu * wu);
    for j in 0..w {
        for i in 0..w {
            let nrm = if i == 0 || j == 0 || i == n || j == n {
                border_normal_f64(terrain, id.level(), dirs[at(i, j)])
            } else {
                let a = vsub(pos[at(i + 1, j)], pos[at(i - 1, j)]);
                let b = vsub(pos[at(i, j + 1)], pos[at(i, j - 1)]);
                let c = vcross(a, b);
                vnormalize(c.x, c.y, c.z)
            };
            normals.push(nrm);
        }
        if cancelled() {
            return None;
        }
    }
    apply_surface_v2(terrain, &mut uvs, &normals, &dirs);

    let mut vertices = Vec::with_capacity(vertex_count(n));
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
    for k in 0..wu * wu {
        push(pos[k], normals[k], uvs[k]);
    }
    for e in Edge::ALL {
        for t in 0..w {
            let (i, j) = e.vertex(n, t);
            let k = at(i, j);
            let d = dirs[k];
            let p = DVec3::new(
                pos[k].x - d.x * depth,
                pos[k].y - d.y * depth,
                pos[k].z - d.z * depth,
            );
            push(p, normals[k], uvs[k]);
        }
    }
    let bound_radius = (max_len2.sqrt() * 1.0001) as f32;
    Some(PatchData {
        mesh: PatchMesh {
            id,
            origin,
            vertices,
            indices: grid_indices(n),
            bound_radius,
            skirt_depth: depth,
            height_min,
            height_max,
        },
        dirs,
        pos,
        uvs,
    })
}
