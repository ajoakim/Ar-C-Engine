//! Exact-op equal-angle cube-sphere mapping over a global integer lattice.
//!
//! Every vertex direction is a pure function of `(face, gu, gv, g)` using only `+ - * /`, `sqrt`
//! and literals (ADR-0008: no libm in border-critical paths), so two patches that share a
//! lattice vertex compute a bit-identical unit direction.

use engine_sdk::DVec3;

use crate::face::Face;
use crate::patch_id::{MAX_LEVEL, PatchId};

/// Exact-op `tan(pi/4 * s)` for `s` in `[-1, 1]`: Taylor `sin / cos` ratio in Horner form.
/// `s == +-1` returns exactly `+-1`. Odd by construction (`x * sn` odd, `cs` even).
#[must_use]
pub fn equal_angle_t(s: f64) -> f64 {
    if s == 1.0 {
        return 1.0;
    }
    if s == -1.0 {
        return -1.0;
    }
    let x = core::f64::consts::FRAC_PI_4 * s;
    let x2 = x * x;
    // sin x = x * (1 - x^2/(2*3) * (1 - x^2/(4*5) * (... )))  up to x^19
    let mut sn = 1.0;
    for k in (1..=9u32).rev() {
        let a = f64::from(2 * k);
        let b = f64::from(2 * k + 1);
        sn = 1.0 - (x2 / (a * b)) * sn;
    }
    // cos x = 1 - x^2/(1*2) * (1 - x^2/(3*4) * (... ))  up to x^20
    let mut cs = 1.0;
    for k in (1..=10u32).rev() {
        let a = f64::from(2 * k - 1);
        let b = f64::from(2 * k);
        cs = 1.0 - (x2 / (a * b)) * cs;
    }
    (x * sn) / cs
}

/// Unit direction of lattice point `(gu, gv)` on `face` with lattice size `g` (power of two,
/// `1..=2^30`), `gu, gv` in `0..=g`.
///
/// Invariant: per world axis at most one of the terms `n`, `a * u`, `b * v` is non-zero (the
/// face basis is a signed permutation), so each component is an exact copy of `+-1`, `+-a` or
/// `+-b` regardless of face; this is what makes shared borders bit-identical across faces.
#[must_use]
pub fn lattice_dir(face: Face, gu: u64, gv: u64, g: u64) -> DVec3 {
    let gf = g as f64;
    let s = (2.0 * (gu as f64) - gf) / gf;
    let t = (2.0 * (gv as f64) - gf) / gf;
    let a = equal_angle_t(s);
    let b = equal_angle_t(t);
    let basis = face.basis();
    let px = f64::from(basis.n[0]) + a * f64::from(basis.u[0]) + b * f64::from(basis.v[0]);
    let py = f64::from(basis.n[1]) + a * f64::from(basis.u[1]) + b * f64::from(basis.v[1]);
    let pz = f64::from(basis.n[2]) + a * f64::from(basis.u[2]) + b * f64::from(basis.v[2]);
    // World x, y, z summation order is fixed.
    let len2 = px * px + py * py + pz * pz;
    let len = len2.sqrt();
    DVec3::new(px / len, py / len, pz / len)
}

/// Vertex `(i, j)` (`0..=n`) of patch `id` with `n` cells per edge.
#[must_use]
pub fn vertex_dir(id: PatchId, n: u32, i: u32, j: u32) -> DVec3 {
    let (x, y) = id.xy();
    let nn = u64::from(n);
    let g = nn << id.level();
    lattice_dir(
        id.face(),
        u64::from(x) * nn + u64::from(i),
        u64::from(y) * nn + u64::from(j),
        g,
    )
}

/// Direction of the patch center (lattice `(2x+1, 2y+1)` with `g = 2^(level+1)`).
#[must_use]
pub fn patch_center_dir(id: PatchId) -> DVec3 {
    let (x, y) = id.xy();
    lattice_dir(
        id.face(),
        2 * u64::from(x) + 1,
        2 * u64::from(y) + 1,
        1u64 << (id.level() + 1),
    )
}

/// Corner directions in the order `(x,y), (x+1,y), (x,y+1), (x+1,y+1)`.
#[must_use]
pub fn patch_corner_dirs(id: PatchId) -> [DVec3; 4] {
    let (x, y) = id.xy();
    let (x, y) = (u64::from(x), u64::from(y));
    let g = 1u64 << id.level();
    let f = id.face();
    [
        lattice_dir(f, x, y, g),
        lattice_dir(f, x + 1, y, g),
        lattice_dir(f, x, y + 1, g),
        lattice_dir(f, x + 1, y + 1, g),
    ]
}

/// Face containing `d`: max `|component|`, ties go to the lowest face index.
#[must_use]
pub fn face_of_dir(d: DVec3) -> Face {
    let comps = [d.x, d.y, d.z];
    let mut best_axis = 0;
    let mut best = comps[0].abs();
    for (axis, c) in comps.iter().enumerate().skip(1) {
        if c.abs() > best {
            best = c.abs();
            best_axis = axis;
        }
    }
    let positive = comps[best_axis] >= 0.0;
    match (best_axis, positive) {
        (0, true) => Face::PosX,
        (0, false) => Face::NegX,
        (1, true) => Face::PosY,
        (1, false) => Face::NegY,
        (_, true) => Face::PosZ,
        (_, false) => Face::NegZ,
    }
}

fn dot_basis(d: DVec3, b: [i32; 3]) -> f64 {
    d.x * f64::from(b[0]) + d.y * f64::from(b[1]) + d.z * f64::from(b[2])
}

/// Patch at `level` (clamped to `MAX_LEVEL`) containing direction `d`, found by interval
/// bisection against `equal_angle_t` (no atan). Non-finite or zero `d` gives `root(PosX)`.
#[must_use]
pub fn locate(d: DVec3, level: u8) -> PatchId {
    let fallback = PatchId::root(Face::PosX);
    if !(d.x.is_finite() && d.y.is_finite() && d.z.is_finite())
        || (d.x == 0.0 && d.y == 0.0 && d.z == 0.0)
    {
        return fallback;
    }
    let level = level.min(MAX_LEVEL);
    let face = face_of_dir(d);
    let basis = face.basis();
    let dn = dot_basis(d, basis.n);
    let a = dot_basis(d, basis.u) / dn;
    let b = dot_basis(d, basis.v) / dn;
    let (mut alo, mut ahi) = (-1.0f64, 1.0f64);
    let (mut blo, mut bhi) = (-1.0f64, 1.0f64);
    let (mut x, mut y) = (0u32, 0u32);
    for _ in 0..level {
        let amid = (alo + ahi) * 0.5;
        x <<= 1;
        if a >= equal_angle_t(amid) {
            alo = amid;
            x |= 1;
        } else {
            ahi = amid;
        }
        let bmid = (blo + bhi) * 0.5;
        y <<= 1;
        if b >= equal_angle_t(bmid) {
            blo = bmid;
            y |= 1;
        } else {
            bhi = bmid;
        }
    }
    PatchId::new(face, level, x, y).unwrap_or(fallback)
}

/// True for a power of two in `2..=64`.
#[must_use]
pub fn is_valid_cells_per_edge(n: u32) -> bool {
    n.is_power_of_two() && (2..=64).contains(&n)
}
