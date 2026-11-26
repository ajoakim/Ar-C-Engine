//! Icosahedron and diamond combinatorics for the hex grid (integer only, all `const`).
//!
//! Vertices: 0 = north pole, 1..=5 = upper ring `U_k`, 6..=10 = lower ring `L_k` (`L_k` sits
//! between `U_k` and `U_{k+1}`), 11 = south pole. A diamond is two faces sharing an edge: north
//! diamonds `d = k` (1..=5) are the cap face `(0, U_k, U_{k+1})` plus the middle face
//! `(U_k, U_{k+1}, L_k)`; south diamonds `d = 5 + k` are `(L_k, L_{k+1}, U_{k+1})` plus the cap
//! `(11, L_k, L_{k+1})`.
//!
//! Each diamond has corners `[P00, P10, P01, P11]` with its origin `P00` at an obtuse (120 degree)
//! corner. Lattice point `(i, j)` has barycentrics `(n-i, i-j, j)` on face `(P00, P10, P11)` when
//! `i >= j` and `(n-j, j-i, i)` on face `(P00, P01, P11)` otherwise, `n = 2^level`. The diamond
//! owns `0 <= i, j < n`; the 10 ring vertices are the 10 origins and the poles are separate cells.
//! This layout was validated by the S1 spike (07.1-04).

/// Number of icosahedron vertices (= pentagon cells per level).
pub const VERTEX_COUNT: u8 = 12;
/// Number of icosahedron faces.
pub const FACE_COUNT: u8 = 20;
/// Number of diamonds.
pub const DIAMOND_COUNT: u8 = 10;

const fn up(k: u8) -> u8 {
    1 + (k + 4) % 5
}

const fn lo(k: u8) -> u8 {
    6 + (k + 4) % 5
}

const fn sort3(f: [u8; 3]) -> [u8; 3] {
    let [mut a, mut b, mut c] = f;
    if a > b {
        let t = a;
        a = b;
        b = t;
    }
    if b > c {
        let t = b;
        b = c;
        c = t;
    }
    if a > b {
        let t = a;
        a = b;
        b = t;
    }
    [a, b, c]
}

const fn build_faces() -> [[u8; 3]; 20] {
    let mut out = [[0u8; 3]; 20];
    let mut k = 1u8;
    while k <= 5 {
        out[(k - 1) as usize] = sort3([0, up(k), up(k + 1)]);
        out[(k + 4) as usize] = sort3([up(k), up(k + 1), lo(k)]);
        out[(k + 9) as usize] = sort3([lo(k - 1), lo(k), up(k)]);
        out[(k + 14) as usize] = sort3([11, lo(k), lo(k + 1)]);
        k += 1;
    }
    out
}

/// The 20 faces as ascending vertex triples.
pub const FACES: [[u8; 3]; 20] = build_faces();

const fn face_index(sorted: [u8; 3]) -> u8 {
    let mut f = 0;
    while f < 20 {
        let t = FACES[f];
        if t[0] == sorted[0] && t[1] == sorted[1] && t[2] == sorted[2] {
            return f as u8;
        }
        f += 1;
    }
    panic!("not an icosahedron face")
}

/// Diamond corners `[P00, P10, P01, P11]` for diamond `d` in `1..=10`.
pub const fn corners(d: u8) -> [u8; 4] {
    assert!(d >= 1 && d <= 10);
    if d <= 5 {
        [up(d), 0, lo(d), up(d + 1)]
    } else {
        let k = d - 5;
        [lo(k), up(k + 1), 11, lo(k + 1)]
    }
}

/// One diamond (see the module doc).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Diamond {
    /// `P00`, the obtuse origin corner; the vertex owned by lattice point `(0, 0)`.
    pub origin: u8,
    /// `P10`, end of the `i` axis.
    pub i_vertex: u8,
    /// `P01`, end of the `j` axis.
    pub j_vertex: u8,
    /// `P11`, the corner opposite the origin.
    pub far_vertex: u8,
    /// Indices into [`FACES`]: `(P00, P10, P11)` then `(P00, P01, P11)`.
    pub faces: [u8; 2],
}

const fn build_diamonds() -> [Diamond; 10] {
    let mut out = [Diamond {
        origin: 0,
        i_vertex: 0,
        j_vertex: 0,
        far_vertex: 0,
        faces: [0, 0],
    }; 10];
    let mut d = 1u8;
    while d <= 10 {
        let c = corners(d);
        out[(d - 1) as usize] = Diamond {
            origin: c[0],
            i_vertex: c[1],
            j_vertex: c[2],
            far_vertex: c[3],
            faces: [
                face_index(sort3([c[0], c[1], c[3]])),
                face_index(sort3([c[0], c[2], c[3]])),
            ],
        };
        d += 1;
    }
    out
}

/// The 10 diamonds; diamond `d` (base id `d`) is `DIAMONDS[d - 1]`.
pub const DIAMONDS: [Diamond; 10] = build_diamonds();

/// Indexes `a` with a small integer. Keeps platform-width integers out of the border-critical
/// files that only handle `u8`/`u32`/`u64` lattice values.
///
/// # Panics
/// When `i` is out of bounds.
pub fn at<T: Copy>(a: &[T], i: u8) -> T {
    a[usize::from(i)]
}
