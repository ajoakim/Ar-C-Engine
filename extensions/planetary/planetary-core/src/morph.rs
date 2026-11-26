//! Geomorph targets (07.1-22): each vertex of a refined cell carries the point of the PARENT-level
//! mesh surface at the vertex's own direction, so a blend factor of 1 reproduces the parent
//! surface and a refinement handover is continuous.
//!
//! The target is a pure function of (direction, parent-level lattice, terrain), independent of
//! which cell asks. Everything here is exact-op (`+ - * /` and `sqrt` only, ADR-0008). This code
//! is NOT inside a border-critical region: it never feeds the equality of border positions.

use engine_sdk::DVec3;

/// Tolerance on the affine weights of a triangle hit (plan 07.1-22).
pub const INSIDE_EPS: f64 = 1e-12;

/// A hit counts as inside a triangle (or on one of its edges) when it is at most this far (metres)
/// outside every edge line; above the f64 noise of a surface position (about 1e-9 m) and well
/// below the 1e-6 m oracle tolerance.
pub const EDGE_SNAP_M: f64 = 2e-7;

/// One morph target vertex: offsets relative to the cell origin (same convention as
/// `CellMesh.vertices[i].position`) and the parent uv.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MorphVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
}

/// Intersection of a ray with a triangle plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MorphHit {
    /// Intersection point `t * d`.
    pub point: DVec3,
    /// Affine weights of `point` (sum 1) for the triangle vertices `a, b, c`.
    pub w: [f64; 3],
    /// Signed distance (metres, positive inside) from `point` to the line of the edge opposite
    /// each vertex.
    pub edge_dist: [f64; 3],
}

impl MorphHit {
    /// Smallest affine weight.
    #[must_use]
    pub fn min_weight(&self) -> f64 {
        self.w[0].min(self.w[1]).min(self.w[2])
    }

    /// Smallest signed edge distance (metres).
    #[must_use]
    pub fn min_edge_dist(&self) -> f64 {
        self.edge_dist[0]
            .min(self.edge_dist[1])
            .min(self.edge_dist[2])
    }
}

/// Counters describing how the targets of one cell were found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MorphStats {
    /// Vertices whose target is a parent vertex copied bit-for-bit.
    pub copied: u32,
    /// Vertices on a parent edge, computed with the canonical chord function.
    pub chord: u32,
    /// Vertices inside a parent triangle.
    pub triangle: u32,
    /// Vertices whose target is a marginal extrapolation (outside every triangle by up to
    /// [`EDGE_SNAP_M`] but accepted).
    pub extrapolated: u32,
    /// Vertices not located in any parent: `morph_position == position`.
    pub identity: u32,
    /// Vertices located in the second parent (hex edge children).
    pub second_parent: u32,
    /// Triangle tests run by the locator.
    pub cone_tests: u64,
    /// Walks started in a second or third ranked sector (the best ranked one did not contain
    /// the direction).
    pub extra_walks: u32,
    /// Parent vertices that had to be sampled (not already available from the cell itself).
    pub parent_samples: u32,
    /// Vertices considered (grid only; skirts follow their border vertex).
    pub vertices: u32,
}

/// Morph targets of one cell in planet space (f64), parallel to `CellMesh.vertices`.
#[derive(Clone, Debug, PartialEq)]
pub struct MorphTargets {
    pub position: Vec<DVec3>,
    pub uv: Vec<[f32; 2]>,
    pub stats: MorphStats,
}

impl MorphTargets {
    /// f32 offsets relative to `origin`, the same conversion the mesh builders use.
    #[must_use]
    pub fn to_vertices(&self, origin: DVec3) -> Vec<MorphVertex> {
        self.position
            .iter()
            .zip(&self.uv)
            .map(|(p, uv)| {
                let o = vsub(*p, origin);
                MorphVertex {
                    position: [o.x as f32, o.y as f32, o.z as f32],
                    uv: *uv,
                }
            })
            .collect()
    }
}

/// Identity morph: every target is the vertex itself.
#[must_use]
pub fn identity_morph(vertices: &[engine_sdk::VertexPosNormalUv]) -> Vec<MorphVertex> {
    vertices
        .iter()
        .map(|v| MorphVertex {
            position: v.position,
            uv: v.uv,
        })
        .collect()
}

pub(crate) fn vsub(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

pub(crate) fn vdot(a: DVec3, b: DVec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

pub(crate) fn vcross(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn finite(v: DVec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

/// Intersects the ray `t * d` (`t > 0`) with the plane of triangle `(a, b, c)`.
///
/// The plane normal is `n = (b - a) x (c - a)` (small, so no cancellation between the large
/// planet-space vectors); `t = (a . n) / (d . n)`; the weights follow from
/// `(p - a) = alpha (b - a) + beta (c - a)`. The affine weights sum to 1 and `point` lies on the
/// plane and on the ray (the plan names Cramer's rule; this is the same solution without the
/// cancellation of the triple product at deep levels). `None` for a degenerate triangle, a ray
/// parallel to or pointing away from the plane, or non-finite input.
#[must_use]
pub fn project_to_triangle(a: DVec3, b: DVec3, c: DVec3, d: DVec3) -> Option<MorphHit> {
    let u = vsub(b, a);
    let v = vsub(c, a);
    let n = vcross(u, v);
    let nn = vdot(n, n);
    let dn = vdot(d, n);
    if !(nn > 0.0 && nn.is_finite() && dn.is_finite() && finite(a) && finite(d)) {
        return None;
    }
    let nlen = nn.sqrt();
    let dlen = vdot(d, d).sqrt();
    if dn.abs() <= 1e-12 * nlen * dlen {
        return None;
    }
    let t = vdot(a, n) / dn;
    if !(t.is_finite() && t > 0.0) {
        return None;
    }
    let p = DVec3::new(d.x * t, d.y * t, d.z * t);
    let r = vsub(p, a);
    let alpha = vdot(vcross(r, v), n) / nn;
    let beta = vdot(vcross(u, r), n) / nn;
    let w = [1.0 - alpha - beta, alpha, beta];
    let e = [vsub(c, b), vsub(c, a), vsub(b, a)];
    let mut edge_dist = [0.0; 3];
    for k in 0..3 {
        let el = vdot(e[k], e[k]).sqrt();
        edge_dist[k] = if el > 0.0 { w[k] * nlen / el } else { 0.0 };
    }
    if !(finite(p) && w.iter().all(|x| x.is_finite())) {
        return None;
    }
    Some(MorphHit {
        point: p,
        w,
        edge_dist,
    })
}

/// Total order on positions used to orient chords canonically.
fn pos_cmp(a: DVec3, b: DVec3) -> core::cmp::Ordering {
    a.x.total_cmp(&b.x)
        .then(a.y.total_cmp(&b.y))
        .then(a.z.total_cmp(&b.z))
}

/// Point of the chord `a - b` on the ray `t * d` and the uv interpolated along it.
///
/// The endpoints are ordered canonically before anything is computed, so every cell that asks for
/// the same chord and direction gets bit-identical output (the seam guarantee for borders).
/// `None` when the chord is degenerate or the ray does not meet its line.
#[must_use]
pub fn chord_target(
    a: (DVec3, [f32; 2]),
    b: (DVec3, [f32; 2]),
    d: DVec3,
) -> Option<(DVec3, [f32; 2])> {
    let (a, b) = if pos_cmp(a.0, b.0).is_le() {
        (a, b)
    } else {
        (b, a)
    };
    let u = vsub(b.0, a.0);
    let ud = vcross(u, d);
    let den = vdot(ud, ud);
    if !(den > 0.0 && den.is_finite()) {
        return None;
    }
    let alpha = -vdot(vcross(a.0, d), ud) / den;
    if !alpha.is_finite() {
        return None;
    }
    let p = DVec3::new(
        a.0.x + alpha * u.x,
        a.0.y + alpha * u.y,
        a.0.z + alpha * u.z,
    );
    let lerp = |x: f32, y: f32| (f64::from(x) + alpha * (f64::from(y) - f64::from(x))) as f32;
    Some((p, [lerp(a.1[0], b.1[0]), lerp(a.1[1], b.1[1])]))
}

/// Affine blend `pos * (1 - m) + target * m`: the CPU mirror of the vertex shader.
#[must_use]
pub fn blend(pos: [f32; 3], target: [f32; 3], m: f32) -> [f32; 3] {
    let k = 1.0 - m;
    [
        pos[0] * k + target[0] * m,
        pos[1] * k + target[1] * m,
        pos[2] * k + target[2] * m,
    ]
}

/// uv of a triangle hit: the hit weights applied to the vertex uvs.
#[must_use]
pub fn blend_uv(w: [f64; 3], uv: [[f32; 2]; 3]) -> [f32; 2] {
    let f = |k: usize| {
        (w[0] * f64::from(uv[0][k]) + w[1] * f64::from(uv[1][k]) + w[2] * f64::from(uv[2][k]))
            as f32
    };
    [f(0), f(1)]
}

/// Chooses the target of a hit that lies inside the triangle `(pa, pb, pc)` (positions + uv) for
/// direction `d`: the vertex itself when `d` equals a vertex direction bit-for-bit, the canonical
/// chord when the hit is on an edge, else the plane point. Returns the target and whether it was a
/// copy (`0`), a chord (`1`) or a plain hit (`2`).
pub(crate) fn resolve_hit(
    tri: [(DVec3, [f32; 2], DVec3); 3],
    d: DVec3,
    hit: &MorphHit,
) -> ((DVec3, [f32; 2]), u8) {
    for (p, uv, vd) in tri {
        if vd.x.to_bits() == d.x.to_bits()
            && vd.y.to_bits() == d.y.to_bits()
            && vd.z.to_bits() == d.z.to_bits()
        {
            return ((p, uv), 0);
        }
    }
    // Edge `k` is opposite vertex `k`.
    let mut best = 0;
    for k in 1..3 {
        if hit.edge_dist[k].abs() < hit.edge_dist[best].abs() {
            best = k;
        }
    }
    if hit.edge_dist[best].abs() <= EDGE_SNAP_M {
        let (i, j) = match best {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        if let Some(t) = chord_target((tri[i].0, tri[i].1), (tri[j].0, tri[j].1), d) {
            return (t, 1);
        }
    }
    (
        (hit.point, blend_uv(hit.w, [tri[0].1, tri[1].1, tri[2].1])),
        2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_is_on_plane_and_ray() {
        let a = DVec3::new(6.0e6, 0.0, 0.0);
        let b = DVec3::new(6.0e6, 100.0, 0.0);
        let c = DVec3::new(6.0e6, 0.0, 100.0);
        let d = DVec3::new(1.0, 0.5e-5, 0.5e-5);
        let h = project_to_triangle(a, b, c, d).expect("hit");
        assert!((h.w[0] + h.w[1] + h.w[2] - 1.0).abs() < 1e-12);
        assert!(h.min_weight() > 0.0);
        let x = vcross(h.point, d);
        assert!(vdot(x, x).sqrt() < 1e-6);
        assert!((h.point.x - 6.0e6).abs() < 1e-6);
    }

    #[test]
    fn degenerate_and_backwards_return_none() {
        let a = DVec3::new(1.0, 0.0, 0.0);
        assert!(project_to_triangle(a, a, a, a).is_none());
        let b = DVec3::new(1.0, 1.0, 0.0);
        let c = DVec3::new(1.0, 0.0, 1.0);
        assert!(project_to_triangle(a, b, c, DVec3::new(-1.0, 0.0, 0.0)).is_none());
        assert!(project_to_triangle(a, b, c, DVec3::new(0.0, 1.0, 0.0)).is_none());
        assert!(project_to_triangle(a, b, c, DVec3::new(f64::NAN, 0.0, 1.0)).is_none());
    }

    #[test]
    fn chord_is_order_independent() {
        let a = (DVec3::new(6.0e6, 0.0, 0.0), [0.1f32, 0.2]);
        let b = (DVec3::new(6.0e6, 100.0, 5.0), [0.3f32, 0.9]);
        let d = DVec3::new(1.0, 0.4e-5, 0.1e-5);
        let p = chord_target(a, b, d).expect("p");
        let q = chord_target(b, a, d).expect("q");
        assert_eq!(p.0, q.0);
        assert_eq!(p.1, q.1);
    }

    #[test]
    fn blend_endpoints() {
        let p = [1.0, 2.0, 3.0];
        let t = [4.0, 5.0, 6.0];
        assert_eq!(blend(p, t, 0.0), p);
        assert_eq!(blend(p, t, 1.0), t);
    }
}
