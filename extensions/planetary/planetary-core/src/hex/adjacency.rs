//! Neighbour / child / parent derivation for hex cells. Integer math only.
//!
//! Cells are mapped to integer barycentrics on an icosahedron face (`n = 2^level`). Steps that
//! leave the diamond rhombus are unfolded across the shared icosahedron edge by reflection
//! (`(a, b, c < 0) -> (a + c, b + c, -c)` on the adjacent face) and mapped back to the single
//! owning cell. The unfolded plane is not a plane around an icosahedron vertex, so the 12
//! pentagon cells enumerate their 5 neighbours per incident edge instead (S1 caveat).

use super::cell_id::{CellId, HEX_MAX_LEVEL};
use super::icosa::{DIAMONDS, FACES};

/// Diamond-plane neighbour steps `(di, dj)` for hexagon cells. The triangle diagonal is the
/// `(+1, +1)` direction (validated by S1).
pub const DIRS: [(i64, i64); 6] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)];

/// Integer barycentrics `w` on the face with vertices `v`; `sum(w) == n`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Bary {
    pub(super) v: [u8; 3],
    pub(super) w: [i64; 3],
    pub(super) n: i64,
}

impl Bary {
    pub(super) fn weight(&self, vertex: u8) -> i64 {
        let mut total = 0;
        for k in 0..3 {
            if self.v[k] == vertex {
                total += self.w[k];
            }
        }
        total
    }

    pub(super) fn supported_by(&self, tri: [u8; 3]) -> bool {
        (0..3).all(|k| self.w[k] <= 0 || tri.contains(&self.v[k]))
    }
}

fn vertex_bary(vertex: u8, n: i64) -> Bary {
    for f in &FACES {
        if f.contains(&vertex) {
            let mut others = f.iter().copied().filter(|x| *x != vertex);
            let a = others.next().unwrap_or(0);
            let b = others.next().unwrap_or(0);
            return Bary {
                v: [vertex, a, b],
                w: [n, 0, 0],
                n,
            };
        }
    }
    Bary {
        v: [vertex, vertex, vertex],
        w: [n, 0, 0],
        n,
    }
}

/// Barycentrics of diamond-plane coordinates, possibly outside the rhombus (negative weights).
fn diamond_bary(d: u8, n: i64, i: i64, j: i64) -> Bary {
    let dia = DIAMONDS[usize::from(d - 1)];
    if i >= j {
        Bary {
            v: [dia.origin, dia.i_vertex, dia.far_vertex],
            w: [n - i, i - j, j],
            n,
        }
    } else {
        Bary {
            v: [dia.origin, dia.j_vertex, dia.far_vertex],
            w: [n - j, j - i, i],
            n,
        }
    }
}

pub(super) fn bary_of(c: CellId) -> Bary {
    let n = 1i64 << c.level();
    match c.base() {
        0 => vertex_bary(0, n),
        11 => vertex_bary(11, n),
        d => {
            let (i, j) = c.ij();
            diamond_bary(d, n, i64::from(i), i64::from(j))
        }
    }
}

/// Unfolds a point at most one lattice step outside its face across edges until all weights are
/// non-negative. `None` when it does not settle (around an icosahedron vertex).
fn reflect(mut p: Bary) -> Option<Bary> {
    for _ in 0..4 {
        let Some(k) = (0..3).find(|k| p.w[*k] < 0) else {
            return Some(p);
        };
        let neg = p.w[k];
        let (ia, ib) = match k {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let (va, vb, x) = (p.v[ia], p.v[ib], p.v[k]);
        let x2 = FACES
            .iter()
            .find(|f| f.contains(&va) && f.contains(&vb) && !f.contains(&x))
            .and_then(|f| f.iter().copied().find(|t| *t != va && *t != vb))?;
        p = Bary {
            v: [va, vb, x2],
            w: [p.w[ia] + neg, p.w[ib] + neg, -neg],
            n: p.n,
        };
    }
    p.w.iter().all(|x| *x >= 0).then_some(p)
}

/// The unique cell owning the lattice point `p` (non-negative weights).
pub(super) fn owner(p: Bary) -> Option<CellId> {
    let level = p.n.trailing_zeros() as u8;
    let mut support = 0;
    let mut last = 0;
    for k in 0..3 {
        if p.w[k] > 0 {
            support += 1;
            last = p.v[k];
        }
    }
    if support == 1 {
        if last == 0 {
            return Some(CellId::north_pole(level));
        }
        if last == 11 {
            return Some(CellId::south_pole(level));
        }
    }
    for d in 1..=10u8 {
        let dia = DIAMONDS[usize::from(d - 1)];
        let tris = [
            ([dia.origin, dia.i_vertex, dia.far_vertex], false),
            ([dia.origin, dia.j_vertex, dia.far_vertex], true),
        ];
        for (tri, swapped) in tris {
            if !p.supported_by(tri) {
                continue;
            }
            let off = p.n - p.weight(tri[0]);
            let far = p.weight(tri[2]);
            let (i, j) = if swapped { (far, off) } else { (off, far) };
            if (0..p.n).contains(&i)
                && (0..p.n).contains(&j)
                && let Ok(id) = CellId::new(d, level, i as u32, j as u32)
            {
                return Some(id);
            }
        }
    }
    None
}

/// Owning cell of diamond-plane coordinates `(i, j)` in `base`, valid within one step of the
/// closed rhombus (`-1..=2^level`) away from the 12 icosahedron vertices. Out-of-contract input
/// never panics in release builds: it is clamped into the diamond.
pub fn canonical(base: u8, level: u8, i: i64, j: i64) -> CellId {
    let n = 1i64 << level;
    if base == 0 {
        return CellId::north_pole(level);
    }
    if base == 11 {
        return CellId::south_pole(level);
    }
    debug_assert!((1..=10).contains(&base) && (-1..=n).contains(&i) && (-1..=n).contains(&j));
    if (0..n).contains(&i) && (0..n).contains(&j) {
        return clamped(base, level, i, j);
    }
    reflect(diamond_bary(base.clamp(1, 10), n, i, j))
        .and_then(owner)
        .unwrap_or_else(|| clamped(base, level, i, j))
}

fn clamped(base: u8, level: u8, i: i64, j: i64) -> CellId {
    let n = 1i64 << level;
    let (ci, cj) = (i.clamp(0, n - 1) as u32, j.clamp(0, n - 1) as u32);
    CellId::new(base.clamp(1, 10), level, ci, cj).unwrap_or(CellId::north_pole(level))
}

/// Whether `c` is one of the 12 pentagon cells (an icosahedron vertex).
pub fn is_pentagon(c: CellId) -> bool {
    c.is_pole() || c.ij() == (0, 0)
}

/// Appends the lattice neighbours of `c` (6, or 5 for a pentagon) in deterministic order:
/// [`DIRS`] order for hexagons, ascending adjacent icosahedron vertex for pentagons.
pub fn neighbors(c: CellId, out: &mut Vec<CellId>) {
    let level = c.level();
    let n = 1i64 << level;
    let first = out.len();
    let mut push = |x: CellId| {
        if x != c && !out[first..].contains(&x) {
            out.push(x);
        }
    };
    if !is_pentagon(c) {
        let (i, j) = c.ij();
        for (di, dj) in DIRS {
            push(canonical(
                c.base(),
                level,
                i64::from(i) + di,
                i64::from(j) + dj,
            ));
        }
        return;
    }
    let vertex = match c.base() {
        0 => 0,
        11 => 11,
        d => DIAMONDS[usize::from(d - 1)].origin,
    };
    let mut adjacent = [false; 12];
    for f in &FACES {
        if f.contains(&vertex) {
            for t in f.iter().filter(|t| **t != vertex) {
                adjacent[usize::from(*t)] = true;
            }
        }
    }
    for (w, is_adj) in (0u8..).zip(adjacent) {
        if !is_adj {
            continue;
        }
        if let Some(x) = FACES
            .iter()
            .find(|f| f.contains(&vertex) && f.contains(&w))
            .and_then(|f| f.iter().copied().find(|t| *t != vertex && *t != w))
            .and_then(|x| {
                owner(Bary {
                    v: [vertex, w, x],
                    w: [n - 1, 1, 0],
                    n,
                })
            })
        {
            push(x);
        }
    }
}

/// Appends the children of `c`: the centre child `(2i, 2j)` first, then the edge children (the
/// level+1 neighbours of the centre child). 7 children for hexagons, 6 for pentagons. Nothing
/// is appended at [`HEX_MAX_LEVEL`].
pub fn children(c: CellId, out: &mut Vec<CellId>) {
    let level = c.level();
    if level >= HEX_MAX_LEVEL {
        return;
    }
    let centre = match c.base() {
        0 => CellId::north_pole(level + 1),
        11 => CellId::south_pole(level + 1),
        d => {
            let (i, j) = c.ij();
            CellId::new(d, level + 1, 2 * i, 2 * j).unwrap_or(CellId::north_pole(level + 1))
        }
    };
    out.push(centre);
    neighbors(centre, out);
}

/// Appends the parents of `c` in ascending raw order: none at level 0, one for a centre child
/// (all barycentrics even), otherwise the two ends of the coarse lattice edge.
pub fn parents(c: CellId, out: &mut Vec<CellId>) {
    if c.level() == 0 {
        return;
    }
    let p = bary_of(c);
    let half = |w: [i64; 3]| Bary {
        v: p.v,
        w: [w[0] / 2, w[1] / 2, w[2] / 2],
        n: p.n / 2,
    };
    if p.w.iter().all(|x| x % 2 == 0) {
        out.extend(owner(half(p.w)));
        return;
    }
    let mut odd = (0..3).filter(|k| p.w[*k] % 2 != 0);
    let (Some(a), Some(b)) = (odd.next(), odd.next()) else {
        return;
    };
    debug_assert!(odd.next().is_none(), "exactly two odd barycentrics");
    let mut lo = p.w;
    lo[a] += 1;
    lo[b] -= 1;
    let mut hi = p.w;
    hi[a] -= 1;
    hi[b] += 1;
    let (x, y) = (owner(half(lo)), owner(half(hi)));
    match (x, y) {
        (Some(x), Some(y)) if x <= y => out.extend([x, y]),
        (Some(x), Some(y)) => out.extend([y, x]),
        (x, y) => out.extend(x.into_iter().chain(y)),
    }
}

/// The lowest-raw parent, if any.
pub fn primary_parent(c: CellId) -> Option<CellId> {
    let mut v = Vec::with_capacity(2);
    parents(c, &mut v);
    v.first().copied()
}
