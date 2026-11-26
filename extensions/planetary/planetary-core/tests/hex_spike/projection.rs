//! S2: exact-op barycentric icosahedron mapping. Only `+ - * /`, `sqrt`, comparisons, `floor`
//! and integer ops (ADR-0008 rules); no transcendental functions anywhere.
//!
//! Orientation choice: the canonical icosahedron `(0, +-1, +-phi)` and cyclic permutations is
//! kept as is (no rotation, which would not be exact-op). The "poles" (vertices 0 and 11 of the
//! lattice numbering) are the antipodal pair `(0, 1, phi)` / `(0, -1, -phi)`; the rings follow
//! from adjacency. Topology discovery below uses a tolerance, but only to NAME vertices; every
//! sampled direction afterwards is exact-op.

use super::lattice::*;
use std::collections::BTreeSet;
use std::sync::OnceLock;

/// Sector side `N` of the design (samples per sector edge).
const SECTOR_N: u64 = 16;

fn phi() -> f64 {
    (1.0 + 5.0f64.sqrt()) / 2.0
}

/// Vertex coordinates indexed by the lattice vertex numbering of `lattice.rs`.
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
        assert_eq!(canon.len(), 12);
        let adjacent = |a: &[f64; 3], b: &[f64; 3]| {
            let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) - 4.0).abs() < 1e-9
        };
        let find = |t: [f64; 3]| canon.iter().position(|c| *c == t).expect("vertex");
        let north = find([0.0, 1.0, p]);
        let south = find([0.0, -1.0, -p]);
        let nbrs: Vec<usize> = (0..12)
            .filter(|k| adjacent(&canon[*k], &canon[north]))
            .collect();
        assert_eq!(nbrs.len(), 5);
        // Upper ring in cyclic order.
        let mut ring = vec![nbrs[0]];
        while ring.len() < 5 {
            let last = *ring.last().expect("ring");
            let next = nbrs
                .iter()
                .copied()
                .find(|k| !ring.contains(k) && adjacent(&canon[*k], &canon[last]))
                .expect("ring successor");
            ring.push(next);
        }
        let mut out = [[0.0; 3]; 12];
        out[0] = canon[north];
        out[11] = canon[south];
        for k in 0..5 {
            out[1 + k] = canon[ring[k]];
            // L_k is adjacent to U_k and U_{k+1} and is neither pole nor on the upper ring.
            let (a, b) = (ring[k], ring[(k + 1) % 5]);
            let low = (0..12)
                .find(|x| {
                    *x != north
                        && !ring.contains(x)
                        && adjacent(&canon[*x], &canon[a])
                        && adjacent(&canon[*x], &canon[b])
                })
                .expect("lower ring vertex");
            out[6 + k] = canon[low];
        }
        out
    })
}

/// Exact-op direction of integer barycentrics `(a, b, c)` over `d` on `face` (weights follow the
/// ascending vertex order of the face). Zero weights contribute no term, so a point on an
/// icosahedron edge sums the same two terms in the same (ascending global vertex) order from
/// either adjacent face, and a vertex is a single term.
pub fn dir(face: u8, a: u64, b: u64, c: u64, d: u64) -> [f64; 3] {
    let (mut a, mut b, mut c, mut d) = (a, b, c, d);
    while d > 1 && a % 2 == 0 && b % 2 == 0 && c % 2 == 0 && d % 2 == 0 {
        a /= 2;
        b /= 2;
        c /= 2;
        d /= 2;
    }
    dir_raw(face, [a, b, c])
}

/// Same as [`dir`] without canonicalising the scale (scale invariance is exact for powers of two).
pub fn dir_raw(face: u8, w: [u64; 3]) -> [f64; 3] {
    let v = vertices();
    let fv = faces()[usize::from(face)];
    let mut p = [0.0f64; 3];
    let mut first = true;
    for k in 0..3 {
        if w[k] == 0 {
            continue;
        }
        let wf = w[k] as f64;
        let vv = v[usize::from(fv[k])];
        if first {
            p = [wf * vv[0], wf * vv[1], wf * vv[2]];
            first = false;
        } else {
            p[0] += wf * vv[0];
            p[1] += wf * vv[1];
            p[2] += wf * vv[2];
        }
    }
    assert!(!first, "all-zero weights");
    let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    [p[0] / len, p[1] / len, p[2] / len]
}

/// Control: sums all three terms (zero terms included) starting from zero. Used only to show what
/// the "skip zero weight" rule protects against.
fn dir_naive(face: u8, w: [u64; 3]) -> [f64; 3] {
    let v = vertices();
    let fv = faces()[usize::from(face)];
    let mut p = [0.0f64; 3];
    for k in 0..3 {
        let wf = w[k] as f64;
        let vv = v[usize::from(fv[k])];
        p[0] += wf * vv[0];
        p[1] += wf * vv[1];
        p[2] += wf * vv[2];
    }
    let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    [p[0] / len, p[1] / len, p[2] / len]
}

fn bits(v: [f64; 3]) -> [u64; 3] {
    v.map(f64::to_bits)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(v: [f64; 3]) -> [f64; 3] {
    let l = dot(v, v).sqrt();
    [v[0] / l, v[1] / l, v[2] / l]
}

/// Direction of a cell centre on the sample lattice `D = 3 * N * 2^L`.
pub fn cell_dir(c: Cell) -> [f64; 3] {
    let fp = point_of(c);
    let k = 3 * SECTOR_N;
    let n = fp.n as u64;
    dir(
        fp.face,
        fp.w[0] as u64 * k,
        fp.w[1] as u64 * k,
        fp.w[2] as u64 * k,
        k * n,
    )
}

/// Unnormalised surface point of a lattice cell (linear in the face plane, measurement only).
fn lin(c: Cell) -> [f64; 3] {
    let fp = point_of(c);
    let v = vertices();
    let fv = fp.verts();
    let mut p = [0.0f64; 3];
    for k in 0..3 {
        let vv = v[usize::from(fv[k])];
        let wf = fp.w[k] as f64;
        for (pc, vc) in p.iter_mut().zip(vv) {
            *pc += wf * vc;
        }
    }
    let n = fp.n as f64;
    [p[0] / n, p[1] / n, p[2] / n]
}

fn add3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn tri_area(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let x = cross(ab, ac);
    dot(x, x).sqrt() / 2.0
}

fn face_centroids() -> [[f64; 3]; 20] {
    let v = vertices();
    let mut out = [[0.0; 3]; 20];
    for (slot, f) in out.iter_mut().zip(faces()) {
        let s = add3(
            add3(v[usize::from(f[0])], v[usize::from(f[1])]),
            v[usize::from(f[2])],
        );
        *slot = s;
    }
    out
}

/// Face by largest dot with the face normals (ties go to the lower face index; all centroid
/// sums have the same length, so the unnormalised sum is enough), then gnomonic barycentrics by
/// Cramer's rule, then cube-coordinate rounding with `floor`.
pub fn locate(d: [f64; 3], level: u8) -> Cell {
    let cents = face_centroids();
    let mut face = 0usize;
    let mut best = dot(d, cents[0]);
    for (k, c) in cents.iter().enumerate().skip(1) {
        let s = dot(d, *c);
        if s > best {
            best = s;
            face = k;
        }
    }
    let x = gnomonic(face as u8, d, level);
    let r = round_cube(x, 1i64 << level);
    let fp = FacePoint {
        face: face as u8,
        n: 1i64 << level,
        w: r,
    };
    cell_of(reflect(fp).expect("rounded point unfolds"))
}

/// Fractional barycentrics scaled to `n = 2^level` of direction `d` on `face`.
fn gnomonic(face: u8, d: [f64; 3], level: u8) -> [f64; 3] {
    let v = vertices();
    let f = faces()[usize::from(face)];
    let (a, b, c) = (
        v[usize::from(f[0])],
        v[usize::from(f[1])],
        v[usize::from(f[2])],
    );
    let wa = dot(d, cross(b, c));
    let wb = dot(a, cross(d, c));
    let wc = dot(a, cross(b, d));
    let s = wa + wb + wc;
    let n = (1u64 << level) as f64;
    [wa / s * n, wb / s * n, wc / s * n]
}

fn round_cube(x: [f64; 3], n: i64) -> [i64; 3] {
    let mut r = x.map(|t| (t + 0.5).floor() as i64);
    let sum = r[0] + r[1] + r[2];
    if sum != n {
        let mut idx = 0;
        let mut worst = -1.0;
        for k in 0..3 {
            let err = (r[k] as f64 - x[k]).abs();
            if err > worst {
                worst = err;
                idx = k;
            }
        }
        r[idx] += n - sum;
    }
    r
}

struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform direction by rejection from the cube (no trig).
    fn direction(&mut self) -> [f64; 3] {
        loop {
            let p = [
                self.unit() * 2.0 - 1.0,
                self.unit() * 2.0 - 1.0,
                self.unit() * 2.0 - 1.0,
            ];
            let l2 = dot(p, p);
            if l2 > 1e-6 && l2 <= 1.0 {
                return norm(p);
            }
        }
    }
}

// ---------------------------------------------------------------- S2 tests

#[test]
fn s2_edges_and_vertices_bit_identical() {
    // The vertex table must agree with the lattice combinatorics: 30 edges of length 2.
    let v = vertices();
    for [a, b] in edges() {
        let (p, q) = (v[usize::from(a)], v[usize::from(b)]);
        let d = [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
        assert!(
            (dot(d, d) - 4.0).abs() < 1e-9,
            "lattice edge {a}-{b} is not a geometric icosahedron edge"
        );
    }
    let mut total_mismatch = 0usize;
    for level in 0..=6u32 {
        let d = 3 * SECTOR_N * (1u64 << level);
        let mut samples = 0usize;
        let mut mismatches = 0usize;
        let mut naive_mismatches = 0usize;
        for [u, w] in edges() {
            let fs: Vec<u8> = (0..20u8)
                .filter(|f| {
                    faces()[usize::from(*f)].contains(&u) && faces()[usize::from(*f)].contains(&w)
                })
                .collect();
            assert_eq!(fs.len(), 2, "edge {u}-{w} must have two faces");
            for a in 0..=d {
                let weights = |f: u8| {
                    let fv = faces()[usize::from(f)];
                    [0, 1, 2].map(|k| {
                        if fv[k] == u {
                            a
                        } else if fv[k] == w {
                            d - a
                        } else {
                            0
                        }
                    })
                };
                let (w0, w1) = (weights(fs[0]), weights(fs[1]));
                let d0 = dir(fs[0], w0[0], w0[1], w0[2], d);
                let d1 = dir(fs[1], w1[0], w1[1], w1[2], d);
                samples += 1;
                if bits(d0) != bits(d1) {
                    mismatches += 1;
                }
                if bits(dir_naive(fs[0], w0)) != bits(dir_naive(fs[1], w1)) {
                    naive_mismatches += 1;
                }
            }
        }
        let mut vsamples = 0usize;
        let mut vmismatch = 0usize;
        for vert in 0..12u8 {
            let fs: Vec<u8> = (0..20u8)
                .filter(|f| faces()[usize::from(*f)].contains(&vert))
                .collect();
            assert_eq!(fs.len(), 5, "vertex {vert} has five faces");
            let mut reference = None;
            for f in fs {
                let fv = faces()[usize::from(f)];
                let w = [0, 1, 2].map(|k| if fv[k] == vert { d } else { 0 });
                let got = bits(dir(f, w[0], w[1], w[2], d));
                vsamples += 1;
                match reference {
                    None => reference = Some(got),
                    Some(r) if r != got => vmismatch += 1,
                    Some(_) => {}
                }
            }
        }
        println!(
            "S2 L{level}: D={d} edge_samples={samples} edge_mismatches={mismatches} vertex_samples={vsamples} vertex_mismatches={vmismatch} (control naive-sum edge mismatches={naive_mismatches})"
        );
        total_mismatch += mismatches + vmismatch;
    }
    assert_eq!(total_mismatch, 0, "edge/vertex bit identity");
}

#[test]
fn s2_level_scaling_identical() {
    // (1) A cell centre and its centre child (same point, doubled lattice) give identical bits.
    let mut checked = 0usize;
    let mut bad = 0usize;
    for level in 0..=5u8 {
        for c in all_cells(level) {
            let child = children(c)[0];
            if bits(cell_dir(c)) != bits(cell_dir(child)) {
                bad += 1;
            }
            checked += 1;
        }
    }
    // Sanity guard: the mapping is injective on cell centres at L<=5.
    for level in 0..=5u8 {
        let set: BTreeSet<[u64; 3]> = all_cells(level)
            .into_iter()
            .map(|c| bits(cell_dir(c)))
            .collect();
        assert_eq!(set.len(), 10 * 4usize.pow(u32::from(level)) + 2);
    }
    // (2) Raw scaling by 2, 4, 8 of random barycentrics on random faces (no canonicalisation).
    let mut rng = SplitMix(0x5EED_0002);
    let mut raw_checked = 0usize;
    let mut raw_bad = 0usize;
    for _ in 0..20_000 {
        let face = (rng.next() % 20) as u8;
        let d = 3 * SECTOR_N * 64;
        let a = rng.next() % (d + 1);
        let b = rng.next() % (d - a + 1);
        let w = [a, b, d - a - b];
        let base = bits(dir_raw(face, w));
        for s in [2u64, 4, 8] {
            raw_checked += 1;
            if bits(dir_raw(face, w.map(|x| x * s))) != base {
                raw_bad += 1;
            }
        }
        if bits(dir(face, w[0] * 2, w[1] * 2, w[2] * 2, d * 2))
            != bits(dir(face, w[0], w[1], w[2], d))
        {
            raw_bad += 1;
        }
    }
    println!(
        "S2 scaling: centre-child checks={checked} mismatches={bad}; raw x2/x4/x8 checks={raw_checked} mismatches={raw_bad}"
    );
    assert_eq!(bad, 0);
    assert_eq!(raw_bad, 0);
}

fn area_stats(level: u8) -> (f64, f64, f64, f64) {
    let (mut hex_min, mut hex_max) = (f64::MAX, 0.0f64);
    let (mut all_min, mut all_max) = (f64::MAX, 0.0f64);
    for c in all_cells(level) {
        let ns = neighbors(c);
        let mut tris = Vec::new();
        for (x, a) in ns.iter().enumerate() {
            for b in &ns[x + 1..] {
                if neighbors(*a).contains(b) {
                    tris.push((*a, *b));
                }
            }
        }
        assert_eq!(tris.len(), ns.len(), "incident lattice triangles of {c:?}");
        let lc = lin(c);
        let p = norm(lc);
        let mut area = 0.0;
        for (a, b) in tris {
            let (la, lb) = (lin(a), lin(b));
            let g = norm(add3(add3(lc, la), lb));
            let mca = norm(add3(lc, la));
            let mcb = norm(add3(lc, lb));
            area += tri_area(p, mca, g) + tri_area(p, g, mcb);
        }
        all_min = all_min.min(area);
        all_max = all_max.max(area);
        if ns.len() == 6 {
            hex_min = hex_min.min(area);
            hex_max = hex_max.max(area);
        }
    }
    (hex_max / hex_min, all_max / all_min, hex_min, hex_max)
}

#[test]
fn s2_hex_area_ratio() {
    let mut ratio5 = 0.0;
    for level in [3u8, 5, 6] {
        let (hex, all, mn, mx) = area_stats(level);
        println!(
            "S2 area L{level}: hex max/min={hex:.4} (incl. pentagons {all:.4}); hex area min={mn:.6e} max={mx:.6e} (unit sphere, flat-triangle measure)"
        );
        if level == 5 {
            ratio5 = hex;
        }
    }
    assert!(ratio5 <= 2.0, "hex area ratio at L5 is {ratio5}");
}

#[test]
fn s2_locate_matches_brute_force() {
    let level = 5u8;
    let n = 1i64 << level;
    let mut rng = SplitMix(0x10CA_7E01);
    let cells = all_cells(level);
    let centres: Vec<[f64; 3]> = cells.iter().map(|c| cell_dir(*c)).collect();
    let samples = 10_000usize;
    let mut lattice_mismatch = 0usize;
    let mut angular_mismatch = 0usize;
    let mut worst_angular_cos_gap = 0.0f64;
    for _ in 0..samples {
        let d = rng.direction();
        let got = locate(d, level);

        // (a) brute force over every lattice point of the located face in the face-lattice
        // metric (the cell definition: hex Voronoi region in the face lattice).
        let cents = face_centroids();
        let mut face = 0usize;
        let mut best = dot(d, cents[0]);
        for (k, c) in cents.iter().enumerate().skip(1) {
            let s = dot(d, *c);
            if s > best {
                best = s;
                face = k;
            }
        }
        let x = gnomonic(face as u8, d, level);
        let mut best_d = f64::MAX;
        let mut best_w = [0i64; 3];
        for a in 0..=n {
            for b in 0..=(n - a) {
                let w = [a, b, n - a - b];
                let (e0, e1, e2) = (x[0] - w[0] as f64, x[1] - w[1] as f64, x[2] - w[2] as f64);
                let e = e0 * e0 + e1 * e1 + e2 * e2;
                if e < best_d {
                    best_d = e;
                    best_w = w;
                }
            }
        }
        let want = cell_of(FacePoint {
            face: face as u8,
            n,
            w: best_w,
        });
        if want != got {
            lattice_mismatch += 1;
        }

        // (b) informational: nearest centre on the sphere (spherical Voronoi) over all cells.
        let mut bi = 0usize;
        let mut bd = f64::MIN;
        for (k, c) in centres.iter().enumerate() {
            let s = dot(d, *c);
            if s > bd {
                bd = s;
                bi = k;
            }
        }
        if cells[bi] != got {
            angular_mismatch += 1;
            let gi = cells.iter().position(|c| *c == got).expect("cell");
            worst_angular_cos_gap = worst_angular_cos_gap.max(bd - dot(d, centres[gi]));
        }
    }
    println!(
        "S2 locate L{level}: samples={samples} mismatches_vs_face_lattice_bruteforce={lattice_mismatch} ; informational vs spherical-nearest-centre: {angular_mismatch} ({:.2}%), worst cos gap={worst_angular_cos_gap:.3e}",
        100.0 * angular_mismatch as f64 / samples as f64
    );
    assert_eq!(lattice_mismatch, 0);
}
