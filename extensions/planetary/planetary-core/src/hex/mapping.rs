//! Exact-op gnomonic projection and sample lattice of the hex topology (promoted from spike S2).
//!
//! Every sample direction is `normalize(a*VA + b*VB + c*VC)` over an integer barycentric lattice
//! point of an icosahedron face, summed in ascending global vertex-index order with only
//! `+ - * /` and `sqrt` (ADR-0008: no libm in border-critical paths). Lattice points are
//! canonicalised first (common powers of two removed; points on an icosahedron edge or vertex
//! move to the lowest-index face containing them), so a point shared by two cells, two faces or
//! five faces yields a bit-identical `f64` direction whichever of them computes it, and a coarse
//! border sample equals the even fine sample of the same border at the next level.
//!
//! Orientation: the canonical icosahedron `(0, +-1, +-phi)` and cyclic permutations is kept
//! unrotated (a rotation would not be exact-op). Vertices 0 and 11 are the antipodal pair
//! `(0, 1, phi)` / `(0, -1, -phi)`.
//!
//! The file is scanned as a whole for platform-width integers (`tests/banned_ops.rs`); slice
//! indexing by `u8` goes through [`super::icosa::at`].

use std::sync::OnceLock;

use engine_sdk::DVec3;

use super::adjacency::{Bary, bary_of, neighbors, owner};
use super::cell_id::{CellId, HEX_MAX_LEVEL};
use super::icosa::{FACES, at};
use crate::lod::Footprint;

/// Largest supported sector side `n`.
pub const MAX_SECTOR_N: u32 = 64;

/// Margin subtracted from the footprint cosine (same as the cube-sphere footprint).
const CONE_MARGIN: f64 = 1e-12;

fn phi() -> f64 {
    (1.0 + 5.0f64.sqrt()) / 2.0
}

fn adjacent(a: &[f64; 3], b: &[f64; 3]) -> bool {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) - 4.0).abs() < 1e-9
}

/// The 12 icosahedron vertices in the global numbering of [`super::icosa`]: 0 and 11 are the
/// poles, `1..=5` the upper ring in cyclic order, `6 + k` is adjacent to `1 + k` and `2 + k`.
/// Adjacency is only used to NAME the vertices; every sampled direction is exact-op.
pub fn vertices() -> &'static [[f64; 3]; 12] {
    static V: OnceLock<[[f64; 3]; 12]> = OnceLock::new();
    V.get_or_init(|| {
        let p = phi();
        let mut canon: Vec<[f64; 3]> = Vec::new();
        for s1 in [1.0, -1.0] {
            for s2 in [1.0, -1.0] {
                canon.push([0.0, s1, s2 * p]);
                canon.push([s1, s2 * p, 0.0]);
                canon.push([s2 * p, 0.0, s1]);
            }
        }
        let find = |t: [f64; 3]| canon.iter().position(|c| *c == t).unwrap_or(0);
        let north = find([0.0, 1.0, p]);
        let south = find([0.0, -1.0, -p]);
        let nbrs: Vec<_> = (0..12)
            .filter(|k| adjacent(&canon[*k], &canon[north]))
            .collect();
        let mut ring = vec![nbrs[0]];
        while ring.len() < 5 {
            let last = ring[ring.len() - 1];
            let next = nbrs
                .iter()
                .copied()
                .find(|k| !ring.contains(k) && adjacent(&canon[*k], &canon[last]))
                .unwrap_or(nbrs[0]);
            ring.push(next);
        }
        let mut out = [[0.0; 3]; 12];
        out[0] = canon[north];
        out[11] = canon[south];
        for k in 0..5 {
            out[1 + k] = canon[ring[k]];
            let (a, b) = (ring[k], ring[(k + 1) % 5]);
            let low = (0..12)
                .find(|x| {
                    *x != north
                        && !ring.contains(x)
                        && adjacent(&canon[*x], &canon[a])
                        && adjacent(&canon[*x], &canon[b])
                })
                .unwrap_or(0);
            out[6 + k] = canon[low];
        }
        out
    })
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn face_verts(face: u8) -> [[f64; 3]; 3] {
    let v = vertices();
    at(&FACES, face.min(19)).map(|x| at(v, x))
}

/// `+1` when the ascending vertex order of the face is counter-clockwise seen from outside.
fn handedness(face: u8) -> i32 {
    let [a, b, c] = face_verts(face);
    if dot3(a, cross3(b, c)) > 0.0 { 1 } else { -1 }
}

/// Integer barycentric lattice point `(a, b, c) / d` of an icosahedron face; the weights follow
/// the ascending vertex order of the face and `a + b + c == d`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LatticePoint {
    pub face: u8,
    pub a: u64,
    pub b: u64,
    pub c: u64,
    pub d: u64,
}

impl LatticePoint {
    /// Canonical representative: common powers of two removed; a point on a face edge or at a
    /// vertex is re-expressed on the lowest-index face containing it.
    pub fn canonical(self) -> LatticePoint {
        let (mut a, mut b, mut c, mut d) = (self.a, self.b, self.c, self.d);
        while d > 1
            && a.is_multiple_of(2)
            && b.is_multiple_of(2)
            && c.is_multiple_of(2)
            && d.is_multiple_of(2)
        {
            a /= 2;
            b /= 2;
            c /= 2;
            d /= 2;
        }
        let face = self.face.min(19);
        let fv = at(&FACES, face);
        let w = [a, b, c];
        let support: Vec<(u8, u64)> = fv.iter().copied().zip(w).filter(|(_, x)| *x > 0).collect();
        if support.is_empty() || support.len() == 3 {
            return LatticePoint { face, a, b, c, d };
        }
        let Some(lowest) = (0u8..20).find(|f| {
            let t = at(&FACES, *f);
            support.iter().all(|(v, _)| t.contains(v))
        }) else {
            return LatticePoint { face, a, b, c, d };
        };
        let mut nw = [0u64; 3];
        for (slot, vertex) in nw.iter_mut().zip(at(&FACES, lowest)) {
            for (v, x) in &support {
                if *v == vertex {
                    *slot = *x;
                }
            }
        }
        LatticePoint {
            face: lowest,
            a: nw[0],
            b: nw[1],
            c: nw[2],
            d,
        }
    }
}

/// Exact-op unit direction of a lattice point (see the module doc). Zero weights contribute no
/// term, so a point on an edge sums the same terms in the same order from either face.
pub fn sample_dir(p: LatticePoint) -> DVec3 {
    let p = p.canonical();
    let v = vertices();
    let fv = at(&FACES, p.face.min(19));
    let mut acc = [0.0f64; 3];
    let mut first = true;
    for (vertex, w) in fv.iter().zip([p.a, p.b, p.c]) {
        if w == 0 {
            continue;
        }
        let wf = w as f64;
        let vv = at(v, *vertex);
        if first {
            acc = [wf * vv[0], wf * vv[1], wf * vv[2]];
            first = false;
        } else {
            acc[0] += wf * vv[0];
            acc[1] += wf * vv[1];
            acc[2] += wf * vv[2];
        }
    }
    let len = (acc[0] * acc[0] + acc[1] * acc[1] + acc[2] * acc[2]).sqrt();
    DVec3::new(acc[0] / len, acc[1] / len, acc[2] / len)
}

// ---------------------------------------------------------------------------------------------
// Cell frame (sector sample lattice)

type FaceWeights = (u8, [[i64; 3]; 3]);

/// Face containing all three cells (unique for a non-degenerate lattice triangle) and their
/// integer barycentrics on it (sum `2^level` each).
fn find_face(cells: [CellId; 3]) -> Option<FaceWeights> {
    let barys: [Bary; 3] = cells.map(bary_of);
    for f in 0u8..20 {
        let tri = at(&FACES, f);
        if barys.iter().all(|b| b.supported_by(tri)) {
            let w = barys.map(|b| tri.map(|vertex| b.weight(vertex)));
            return Some((f, w));
        }
    }
    None
}

fn det3(w: &[[i64; 3]; 3]) -> i128 {
    let [[a, b, c], [d, e, f], [g, h, i]] = w.map(|r| r.map(i128::from));
    a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)
}

/// Sign of the orientation of `(c, a, b)` seen from outside (positive: counter-clockwise).
fn orientation(c: CellId, a: CellId, b: CellId) -> i32 {
    let Some((f, w)) = find_face([c, a, b]) else {
        return 0;
    };
    let det = det3(&w);
    let sign = if det > 0 {
        1
    } else if det < 0 {
        -1
    } else {
        0
    };
    sign * handedness(f)
}

/// The lattice neighbours of `c` (6, or 5 for a pentagon) in counter-clockwise order seen from
/// outside, starting at the lowest raw id; consecutive entries (cyclically) are adjacent.
pub fn ring(c: CellId) -> Vec<CellId> {
    let mut nb = Vec::with_capacity(6);
    neighbors(c, &mut nb);
    nb.sort();
    let mut order: Vec<CellId> = Vec::with_capacity(nb.len());
    if let Some(first) = nb.first().copied() {
        order.push(first);
    }
    let mut scratch = Vec::with_capacity(6);
    while order.len() < nb.len() {
        let Some(last) = order.last().copied() else {
            break;
        };
        scratch.clear();
        neighbors(last, &mut scratch);
        let Some(next) = nb
            .iter()
            .copied()
            .find(|y| !order.contains(y) && scratch.contains(y))
        else {
            break;
        };
        order.push(next);
    }
    if order.len() != nb.len() {
        // Defensive: not a cycle (cannot happen on a valid lattice); fall back to raw order.
        return nb;
    }
    if let (Some(a0), Some(a1)) = (order.first().copied(), order.get(1).copied())
        && orientation(c, a0, a1) < 0
    {
        order.reverse();
        order.rotate_right(1);
    }
    order
}

/// Precomputed sector triangles of one cell for a sector side `n` (even, `2..=64`).
///
/// Sector `k` is the triangle `(centre, corner_k, corner_{k+1})`, where `corner_k` is the
/// centroid of the lattice triangle `(centre, ring[k-1], ring[k])`; the sector spans the two
/// lattice triangles on either side of the lattice direction `ring[k]`. Sample `(s, t)` is
/// `centre + s*(corner_k - centre)/n + t*(corner_{k+1} - centre)/n`, evaluated in integers.
#[derive(Clone, Debug)]
pub struct CellFrame {
    centre: CellId,
    n: u32,
    level: u8,
    /// Per sector: triangle `(centre, prev, cur)` and triangle `(centre, cur, next)`.
    tris: Vec<[FaceWeights; 2]>,
}

impl CellFrame {
    /// Builds the frame. `n` must be even and in `2..=64` (clamped otherwise).
    pub fn new(centre: CellId, n: u32) -> CellFrame {
        debug_assert!(
            n.is_multiple_of(2) && (2..=MAX_SECTOR_N).contains(&n),
            "bad sector n"
        );
        let n = (n & !1).clamp(2, MAX_SECTOR_N);
        let r = ring(centre);
        let m = r.len() as u8;
        let mut tris = Vec::with_capacity(r.len());
        for k in 0..m {
            let prev = at(&r, (k + m - 1) % m);
            let cur = at(&r, k);
            let next = at(&r, (k + 1) % m);
            let fallback = || find_face([centre, centre, centre]).unwrap_or((0, [[0; 3]; 3]));
            let t1 = find_face([centre, prev, cur]).unwrap_or_else(fallback);
            let t2 = find_face([centre, cur, next]).unwrap_or_else(fallback);
            tris.push([t1, t2]);
        }
        CellFrame {
            centre,
            n,
            level: centre.level(),
            tris,
        }
    }

    /// Sector count: 6, or 5 for a pentagon.
    pub fn sectors(&self) -> u8 {
        self.tris.len() as u8
    }

    pub fn n(&self) -> u32 {
        self.n
    }

    pub fn centre(&self) -> CellId {
        self.centre
    }

    fn denominator(&self) -> u64 {
        3 * u64::from(self.n) * (1u64 << self.level)
    }

    /// The cell centre as a lattice point of the sample lattice `d = 3 * n * 2^level`.
    pub fn center_point(&self) -> LatticePoint {
        self.sample(0, 0, 0)
    }

    /// Sample `(s, t)` (each `0..=n`) of `sector`; `s + t <= n` stays inside the cell.
    pub fn sample(&self, sector: u8, s: u32, t: u32) -> LatticePoint {
        let n = i64::from(self.n);
        let (s, t) = (i64::from(s.min(self.n)), i64::from(t.min(self.n)));
        let m = self.sectors().max(1);
        let pair = at(&self.tris, sector % m);
        let (face, w, coef) = if s >= t {
            let (f, w) = pair[0];
            (f, w, [3 * n - 2 * s - t, s - t, s + 2 * t])
        } else {
            let (f, w) = pair[1];
            (f, w, [3 * n - s - 2 * t, 2 * s + t, t - s])
        };
        let mut out = [0u64; 3];
        for (q, slot) in out.iter_mut().enumerate() {
            let v = coef[0] * w[0][q] + coef[1] * w[1][q] + coef[2] * w[2][q];
            *slot = v.max(0) as u64;
        }
        LatticePoint {
            face,
            a: out[0],
            b: out[1],
            c: out[2],
            d: self.denominator(),
        }
    }

    /// Corner `k` (centroid of the lattice triangle `(centre, ring[k-1], ring[k])`).
    pub fn corner_point(&self, k: u8) -> LatticePoint {
        self.sample(k, self.n, 0)
    }
}

/// Cell centre as a lattice point of the sample lattice `d = 3 * n * 2^level`.
pub fn cell_center_point(c: CellId, n: u32) -> LatticePoint {
    CellFrame::new(c, n).center_point()
}

/// Appends the 6 (or 5) corners in direction order (see [`CellFrame`]).
pub fn cell_corner_points(c: CellId, n: u32, out: &mut Vec<LatticePoint>) {
    let f = CellFrame::new(c, n);
    for k in 0..f.sectors() {
        out.push(f.corner_point(k));
    }
}

/// One sample of one sector (builds the frame; use [`CellFrame`] to amortise).
pub fn cell_sample_point(c: CellId, n: u32, sector: u8, s: u32, t: u32) -> LatticePoint {
    CellFrame::new(c, n).sample(sector, s, t)
}

/// Cone containing the cell: centre direction and the cosine/sine of the largest angle to the
/// corners and to border samples at quarter spacing, lowered by a tiny margin.
pub fn footprint(c: CellId) -> Footprint {
    let f = CellFrame::new(c, 4);
    let centre = sample_dir(f.center_point());
    let mut cos_delta = 1.0f64;
    for k in 0..f.sectors() {
        for s in 0..=4u32 {
            let d = sample_dir(f.sample(k, s, 4 - s));
            let v = d.x * centre.x + d.y * centre.y + d.z * centre.z;
            if v < cos_delta {
                cos_delta = v;
            }
        }
    }
    cos_delta -= CONE_MARGIN;
    let s2 = 1.0 - cos_delta * cos_delta;
    let sin_delta = if s2 > 0.0 { s2.sqrt() } else { 0.0 };
    Footprint {
        center: centre,
        cos_delta,
        sin_delta,
    }
}

// ---------------------------------------------------------------------------------------------
// locate

fn face_centroid(face: u8) -> [f64; 3] {
    let [a, b, c] = face_verts(face);
    [
        (a[0] + b[0]) + c[0],
        (a[1] + b[1]) + c[1],
        (a[2] + b[2]) + c[2],
    ]
}

/// The cell of `level` whose region contains `dir`: the nearest lattice point in the gnomonic
/// face-lattice metric (cube-coordinate rounding of the gnomonic barycentrics). This is the
/// design's cell definition; against the spherical nearest centre it differs only near borders
/// (spike S2: about 1% of random directions at level 5, worst cosine gap 3.6e-5).
///
/// Non-finite or zero directions return the north-pole cell of the level (documented, T-07.1-31).
pub fn locate(dir: DVec3, level: u8) -> CellId {
    let level = level.min(HEX_MAX_LEVEL);
    let north = CellId::north_pole(level);
    let d = [dir.x, dir.y, dir.z];
    if !(d[0].is_finite() && d[1].is_finite() && d[2].is_finite()) || dot3(d, d) <= 0.0 {
        return north;
    }
    let mut face = 0u8;
    let mut best = dot3(d, face_centroid(0));
    for f in 1u8..20 {
        let s = dot3(d, face_centroid(f));
        if s > best {
            best = s;
            face = f;
        }
    }
    let [a, b, c] = face_verts(face);
    let wa = dot3(d, cross3(b, c));
    let wb = dot3(a, cross3(d, c));
    let wc = dot3(a, cross3(b, d));
    let sum = wa + wb + wc;
    let n = 1i64 << level;
    let nf = n as f64;
    let x = [wa / sum * nf, wb / sum * nf, wc / sum * nf];
    if !(x[0].is_finite() && x[1].is_finite() && x[2].is_finite()) {
        return north;
    }
    let mut r = x.map(|t| (t + 0.5).floor() as i64);
    let total = r[0] + r[1] + r[2];
    if total != n {
        let mut idx = 0;
        let mut worst = -1.0;
        for (k, (rk, xk)) in r.iter().zip(x).enumerate() {
            let err = (*rk as f64 - xk).abs();
            if err > worst {
                worst = err;
                idx = k;
            }
        }
        r[idx] += n - total;
    }
    for v in &mut r {
        *v = (*v).clamp(0, n);
    }
    owner(Bary {
        v: at(&FACES, face),
        w: r,
        n,
    })
    .unwrap_or(north)
}

/// Direction of the cell centre (convenience).
pub fn cell_center_dir(c: CellId, n: u32) -> DVec3 {
    sample_dir(cell_center_point(c, n))
}
