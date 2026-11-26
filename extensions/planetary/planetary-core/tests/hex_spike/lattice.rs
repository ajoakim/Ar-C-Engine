//! S1: diamond (Q2DI-style) lattice on the icosahedron. Integer math only; ordered maps only.
//!
//! Layout. Vertices: 0 = north pole, 1..=5 = upper ring `U_k`, 6..=10 = lower ring `L_k`
//! (`L_k` sits between `U_k` and `U_{k+1}`), 11 = south pole. Diamond `d` is two faces sharing an
//! edge: north diamonds `d = k` (1..=5) are the cap face `(0, U_k, U_{k+1})` plus the middle face
//! `(U_k, U_{k+1}, L_k)`; south diamonds `d = 5 + k` are `(L_k, L_{k+1}, U_{k+1})` plus the cap
//! `(11, L_k, L_{k+1})`.
//!
//! Each diamond has its origin at an OBTUSE (120 degree) corner, with corners
//! `[P00, P10, P01, P11]`; lattice point `(i, j)` has barycentrics `(n-i, i-j, j)` on face
//! `(P00, P10, P11)` when `i >= j` and `(n-j, j-i, i)` on face `(P00, P01, P11)` otherwise, with
//! `n = 2^level`. The two faces meet along the short diagonal `i == j`, so the triangle diagonal
//! is the `(+1, +1)` direction (the plan allowed choosing the diagonal; with 120 degree axes it
//! is `(1, 1)` and not `(1, -1)`). The diamond owns `0 <= i, j < n` (half-open); the origin sides
//! `i = 0`, `j = 0` are "near" sides. Every one of the 20 diamond boundary edges is a near side of
//! exactly one diamond, the 10 ring vertices are the 10 origins and the two poles are separate
//! cells, which gives exactly `10 * 4^L + 2` cells.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, OnceLock};

pub const MAX_LEVEL: u8 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cell {
    /// 0 = north pole, 1..=10 = diamond, 11 = south pole.
    pub base: u8,
    pub level: u8,
    pub i: u32,
    pub j: u32,
}

// ---------------------------------------------------------------- icosahedron combinatorics

fn up(k: i32) -> u8 {
    1 + (k - 1).rem_euclid(5) as u8
}

fn lo(k: i32) -> u8 {
    6 + (k - 1).rem_euclid(5) as u8
}

/// The 20 faces, each with ascending vertex ids.
pub fn faces() -> &'static [[u8; 3]; 20] {
    static F: OnceLock<[[u8; 3]; 20]> = OnceLock::new();
    F.get_or_init(|| {
        let mut out = Vec::new();
        for k in 1..=5 {
            out.push([0, up(k), up(k + 1)]);
        }
        for k in 1..=5 {
            out.push([up(k), up(k + 1), lo(k)]);
        }
        for k in 1..=5 {
            out.push([lo(k - 1), lo(k), up(k)]);
        }
        for k in 1..=5 {
            out.push([11, lo(k), lo(k + 1)]);
        }
        let mut arr = [[0u8; 3]; 20];
        for (slot, mut f) in arr.iter_mut().zip(out) {
            f.sort_unstable();
            *slot = f;
        }
        arr
    })
}

pub fn face_index(sorted: [u8; 3]) -> u8 {
    faces()
        .iter()
        .position(|f| *f == sorted)
        .expect("not an icosahedron face") as u8
}

fn face_containing(vs: &[u8]) -> u8 {
    faces()
        .iter()
        .position(|f| vs.iter().all(|v| f.contains(v)))
        .expect("no face contains the vertices") as u8
}

/// The 30 edges as sorted vertex pairs.
pub fn edges() -> Vec<[u8; 2]> {
    let mut set = BTreeSet::new();
    for f in faces() {
        set.insert([f[0], f[1]]);
        set.insert([f[0], f[2]]);
        set.insert([f[1], f[2]]);
    }
    set.into_iter().collect()
}

pub fn vertex_adjacent(v: u8) -> Vec<u8> {
    let mut set = BTreeSet::new();
    for f in faces() {
        if f.contains(&v) {
            set.extend(f.iter().copied().filter(|x| *x != v));
        }
    }
    set.into_iter().collect()
}

/// Diamond corners `[P00, P10, P01, P11]` (see the module doc).
pub fn corners(d: u8) -> [u8; 4] {
    assert!((1..=10).contains(&d));
    if d <= 5 {
        let k = i32::from(d);
        [up(k), 0, lo(k), up(k + 1)]
    } else {
        let k = i32::from(d) - 5;
        [lo(k), up(k + 1), 11, lo(k + 1)]
    }
}

// ---------------------------------------------------------------- points on faces

/// A lattice point as integer barycentrics `w` (aligned with `faces()[face]`), `sum(w) == n`.
#[derive(Clone, Copy, Debug)]
pub struct FacePoint {
    pub face: u8,
    pub n: i64,
    pub w: [i64; 3],
}

pub type Key = [(u8, i64); 3];

impl FacePoint {
    pub fn verts(&self) -> [u8; 3] {
        faces()[usize::from(self.face)]
    }

    /// Face independent key: positive-weight vertices with their weights, ascending by vertex.
    pub fn key(&self) -> Key {
        let mut key = [(255u8, 0i64); 3];
        let mut m = 0;
        for (v, w) in self.verts().into_iter().zip(self.w) {
            if w > 0 {
                key[m] = (v, w);
                m += 1;
            }
        }
        key
    }

    pub fn level(&self) -> u8 {
        self.n.trailing_zeros() as u8
    }
}

fn point_on_face(face: u8, n: i64, assign: &[(u8, i64)]) -> FacePoint {
    let verts = faces()[usize::from(face)];
    let mut w = [0i64; 3];
    for (slot, v) in w.iter_mut().zip(verts) {
        if let Some((_, x)) = assign.iter().find(|(a, _)| *a == v) {
            *slot = *x;
        }
    }
    FacePoint { face, n, w }
}

fn vertex_point(v: u8, n: i64) -> FacePoint {
    let face = face_containing(&[v]);
    point_on_face(face, n, &[(v, n)])
}

/// Diamond-plane lattice coordinates (possibly outside the rhombus) as face barycentrics; some
/// weights may be negative outside the diamond.
fn diamond_point(d: u8, n: i64, i: i64, j: i64) -> FacePoint {
    let [p00, p10, p01, p11] = corners(d);
    let (tri, ws) = if i >= j {
        ([p00, p10, p11], [n - i, i - j, j])
    } else {
        ([p00, p01, p11], [n - j, j - i, i])
    };
    let mut sorted = tri;
    sorted.sort_unstable();
    let face = face_index(sorted);
    let assign: Vec<(u8, i64)> = tri.into_iter().zip(ws).collect();
    point_on_face(face, n, &assign)
}

/// Unfold across edges until all weights are non-negative (valid for points at most one lattice
/// step outside a face that are not an icosahedron vertex). Returns `None` when it does not settle.
fn reflect(mut fp: FacePoint) -> Option<FacePoint> {
    for _ in 0..4 {
        let Some(k) = fp.w.iter().position(|x| *x < 0) else {
            return Some(fp);
        };
        let neg = fp.w[k];
        let verts = fp.verts();
        let others: Vec<usize> = (0..3).filter(|x| *x != k).collect();
        let (a, b) = (verts[others[0]], verts[others[1]]);
        let f2 = faces()
            .iter()
            .enumerate()
            .find(|(idx, f)| *idx != usize::from(fp.face) && f.contains(&a) && f.contains(&b))
            .map(|(idx, _)| idx as u8)?;
        let x2 = faces()[usize::from(f2)]
            .into_iter()
            .find(|v| *v != a && *v != b)?;
        // C = A + B - C' in the unfolded plane, so p = (a+c) A + (b+c) B - c C'.
        let assign = [
            (a, fp.w[others[0]] + neg),
            (b, fp.w[others[1]] + neg),
            (x2, -neg),
        ];
        fp = point_on_face(f2, fp.n, &assign);
    }
    fp.w.iter().all(|x| *x >= 0).then_some(fp)
}

// ---------------------------------------------------------------- level tables

pub struct LevelTable {
    pub map: BTreeMap<Key, Cell>,
    pub duplicate_keys: usize,
}

pub fn table(level: u8) -> Arc<LevelTable> {
    static CACHE: OnceLock<Mutex<BTreeMap<u8, Arc<LevelTable>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = cache.lock().expect("table cache");
    if let Some(t) = guard.get(&level) {
        return Arc::clone(t);
    }
    let mut map = BTreeMap::new();
    let mut duplicate_keys = 0;
    for c in all_cells(level) {
        if map.insert(point_of(c).key(), c).is_some() {
            duplicate_keys += 1;
        }
    }
    let t = Arc::new(LevelTable {
        map,
        duplicate_keys,
    });
    guard.insert(level, Arc::clone(&t));
    t
}

pub fn all_cells(level: u8) -> Vec<Cell> {
    let n = 1u32 << level;
    let mut out = vec![
        Cell {
            base: 0,
            level,
            i: 0,
            j: 0,
        },
        Cell {
            base: 11,
            level,
            i: 0,
            j: 0,
        },
    ];
    for base in 1..=10 {
        for i in 0..n {
            for j in 0..n {
                out.push(Cell { base, level, i, j });
            }
        }
    }
    out
}

pub fn point_of(c: Cell) -> FacePoint {
    let n = 1i64 << c.level;
    match c.base {
        0 => vertex_point(0, n),
        11 => vertex_point(11, n),
        d => diamond_point(d, n, i64::from(c.i), i64::from(c.j)),
    }
}

pub fn cell_of(fp: FacePoint) -> Cell {
    *table(fp.level())
        .map
        .get(&fp.key())
        .unwrap_or_else(|| panic!("no cell for point {fp:?}"))
}

/// The icosahedron vertex a cell sits on (the 12 pentagons), if any.
pub fn vertex_of(c: Cell) -> Option<u8> {
    match c.base {
        0 => Some(0),
        11 => Some(11),
        d if c.i == 0 && c.j == 0 => Some(corners(d)[0]),
        _ => None,
    }
}

pub const DIAMOND_DIRS: [(i64, i64); 6] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)];
const BARY_DIRS: [[i64; 3]; 6] = [
    [1, -1, 0],
    [-1, 1, 0],
    [1, 0, -1],
    [-1, 0, 1],
    [0, 1, -1],
    [0, -1, 1],
];

/// Owning cell of any diamond-plane coordinate within one step of the closed rhombus (the wrap
/// table, computed by unfolding across the shared icosahedron edge). `None` when the coordinate is
/// ambiguous (around an icosahedron vertex the unfolded plane is not a plane).
pub fn canonical(d: u8, level: u8, i: i64, j: i64) -> Option<Cell> {
    let n = 1i64 << level;
    if (0..n).contains(&i) && (0..n).contains(&j) {
        return Some(Cell {
            base: d,
            level,
            i: i as u32,
            j: j as u32,
        });
    }
    let fp = reflect(diamond_point(d, n, i, j))?;
    Some(cell_of(fp))
}

/// All lattice neighbours (6, or 5 for a pentagon), sorted, NOT deduplicated (tests check that).
pub fn neighbors(c: Cell) -> Vec<Cell> {
    let n = 1i64 << c.level;
    let mut out = Vec::new();
    if let Some(v) = vertex_of(c) {
        for w in vertex_adjacent(v) {
            let face = face_containing(&[v, w]);
            out.push(cell_of(point_on_face(face, n, &[(v, n - 1), (w, 1)])));
        }
    } else {
        let fp = point_of(c);
        for dir in BARY_DIRS {
            let w = [fp.w[0] + dir[0], fp.w[1] + dir[1], fp.w[2] + dir[2]];
            let moved = reflect(FacePoint { w, ..fp }).expect("neighbour unfold");
            out.push(cell_of(moved));
        }
    }
    out.sort_unstable();
    out
}

/// Centre child plus the edge children (the lattice neighbours of the centre child one level down).
pub fn children(c: Cell) -> Vec<Cell> {
    assert!(c.level < MAX_LEVEL);
    let fp = point_of(c);
    let fine = FacePoint {
        face: fp.face,
        n: fp.n * 2,
        w: fp.w.map(|x| x * 2),
    };
    let centre = cell_of(fine);
    let mut out = vec![centre];
    out.extend(neighbors(centre));
    out
}

/// One parent (centre child) or the two ends of the coarse lattice edge, sorted by packed id.
pub fn parents(c: Cell) -> Vec<Cell> {
    if c.level == 0 {
        return Vec::new();
    }
    let fp = point_of(c);
    let half = |w: [i64; 3]| FacePoint {
        face: fp.face,
        n: fp.n / 2,
        w: w.map(|x| x / 2),
    };
    let mut out: Vec<Cell> = if fp.w.iter().all(|x| x % 2 == 0) {
        vec![cell_of(half(fp.w))]
    } else {
        let odd: Vec<usize> = (0..3).filter(|k| fp.w[*k] % 2 != 0).collect();
        assert_eq!(odd.len(), 2, "exactly two odd barycentrics");
        let mut a = fp.w;
        a[odd[0]] += 1;
        a[odd[1]] -= 1;
        let mut b = fp.w;
        b[odd[0]] -= 1;
        b[odd[1]] += 1;
        vec![cell_of(half(a)), cell_of(half(b))]
    };
    out.sort_unstable_by_key(|p| pack(*p));
    out
}

pub fn primary_parent(c: Cell) -> Option<Cell> {
    parents(c).into_iter().min_by_key(|p| pack(*p))
}

// ---------------------------------------------------------------- id packing

fn spread(x: u32) -> u64 {
    let mut v = u64::from(x);
    v = (v | (v << 16)) & 0x0000_FFFF_0000_FFFF;
    v = (v | (v << 8)) & 0x00FF_00FF_00FF_00FF;
    v = (v | (v << 4)) & 0x0F0F_0F0F_0F0F_0F0F;
    v = (v | (v << 2)) & 0x3333_3333_3333_3333;
    v = (v | (v << 1)) & 0x5555_5555_5555_5555;
    v
}

fn compact(mut v: u64) -> u32 {
    v &= 0x5555_5555_5555_5555;
    v = (v | (v >> 1)) & 0x3333_3333_3333_3333;
    v = (v | (v >> 2)) & 0x0F0F_0F0F_0F0F_0F0F;
    v = (v | (v >> 4)) & 0x00FF_00FF_00FF_00FF;
    v = (v | (v >> 8)) & 0x0000_FFFF_0000_FFFF;
    v = (v | (v >> 16)) & 0x0000_0000_FFFF_FFFF;
    v as u32
}

/// `base << 60 | level << 55 | morton(i, j) << 7`.
pub fn pack(c: Cell) -> u64 {
    (u64::from(c.base) << 60)
        | (u64::from(c.level) << 55)
        | ((spread(c.i) | (spread(c.j) << 1)) << 7)
}

pub fn unpack(x: u64) -> Option<Cell> {
    if x & 0x7F != 0 {
        return None;
    }
    let base = (x >> 60) as u8;
    let level = ((x >> 55) & 0x1F) as u8;
    if base > 11 || level > MAX_LEVEL {
        return None;
    }
    let m = (x >> 7) & ((1u64 << 48) - 1);
    let (i, j) = (compact(m), compact(m >> 1));
    let lim = 1u64 << level;
    if u64::from(i) >= lim || u64::from(j) >= lim {
        return None;
    }
    if (base == 0 || base == 11) && (i != 0 || j != 0) {
        return None;
    }
    Some(Cell { base, level, i, j })
}

// ---------------------------------------------------------------- S1 tests

fn count_for(level: u8) -> usize {
    10 * 4usize.pow(u32::from(level)) + 2
}

#[test]
fn s1_cell_counts_and_pentagons() {
    for level in 0..=6u8 {
        let cells = all_cells(level);
        let expected = count_for(level);
        assert_eq!(cells.len(), expected, "enumerated count at L{level}");
        let t = table(level);
        assert_eq!(t.duplicate_keys, 0, "two cells share one point at L{level}");
        assert_eq!(t.map.len(), expected, "distinct points owned at L{level}");

        // Independent geometric enumeration of every lattice point of the subdivided sphere.
        let n = 1i64 << level;
        let mut geom = BTreeSet::new();
        for f in 0..20u8 {
            for a in 0..=n {
                for b in 0..=(n - a) {
                    let fp = FacePoint {
                        face: f,
                        n,
                        w: [a, b, n - a - b],
                    };
                    geom.insert(fp.key());
                }
            }
        }
        assert_eq!(geom.len(), expected, "geometric lattice points at L{level}");
        let owned: BTreeSet<Key> = t.map.keys().copied().collect();
        assert_eq!(owned, geom, "ownership is a bijection onto lattice points");

        let mut pentagons = Vec::new();
        let mut hist = BTreeMap::new();
        for c in &cells {
            let deg = neighbors(*c).len();
            *hist.entry(deg).or_insert(0usize) += 1;
            if deg == 5 {
                pentagons.push(*c);
            }
        }
        assert_eq!(pentagons.len(), 12, "pentagon count at L{level}");
        assert!(
            pentagons.iter().all(|c| vertex_of(*c).is_some()),
            "pentagons are exactly the 12 icosahedron vertices"
        );
        assert_eq!(
            hist.keys().copied().collect::<Vec<_>>(),
            if level == 0 { vec![5] } else { vec![5, 6] },
            "only degrees 5 and 6"
        );
        println!(
            "S1 counts L{level}: cells={} (10*4^L+2={expected}) pentagons={} degree_hist={hist:?}",
            cells.len(),
            pentagons.len()
        );
    }
}

#[test]
fn s1_neighbors_reciprocal() {
    let mut total = 0usize;
    let mut canon_checked = 0usize;
    for level in 0..=6u8 {
        for c in all_cells(level) {
            let ns = neighbors(c);
            let set: BTreeSet<Cell> = ns.iter().copied().collect();
            assert_eq!(set.len(), ns.len(), "duplicate neighbour of {c:?}");
            assert!(!set.contains(&c), "self neighbour {c:?}");
            for nb in &ns {
                total += 1;
                assert!(
                    neighbors(*nb).contains(&c),
                    "not reciprocal: {c:?} -> {nb:?}"
                );
            }
            // The diamond-plane wrap (canonical) must reproduce the face-unfold neighbours.
            if vertex_of(c).is_none() {
                let via_diamond: BTreeSet<Cell> = DIAMOND_DIRS
                    .iter()
                    .map(|(di, dj)| {
                        canonical(c.base, level, i64::from(c.i) + di, i64::from(c.j) + dj)
                            .unwrap_or_else(|| panic!("canonical failed for {c:?} {di},{dj}"))
                    })
                    .collect();
                assert_eq!(via_diamond, set, "wrap table disagrees for {c:?}");
                canon_checked += 1;
            }
        }
        println!("S1 neighbours L{level}: reciprocal ok, running directed edges={total}");
    }
    println!("S1 wrap table cross-checked on {canon_checked} non-vertex cells");
}

#[test]
fn s1_parent_child_round_trip() {
    let mut edge_children = 0usize;
    let mut centre_children = 0usize;
    for level in 0..=5u8 {
        for c in all_cells(level) {
            let pent = vertex_of(c).is_some();
            let ch = children(c);
            let set: BTreeSet<Cell> = ch.iter().copied().collect();
            assert_eq!(set.len(), ch.len(), "duplicate children of {c:?}");
            assert_eq!(ch.len(), if pent { 6 } else { 7 }, "child count of {c:?}");
            assert!(ch.iter().all(|x| x.level == level + 1));
            let centre = ch[0];
            assert_eq!(parents(centre), vec![c], "centre child of {c:?}");
            centre_children += 1;
            for e in &ch[1..] {
                let ps = parents(*e);
                assert_eq!(ps.len(), 2, "edge child {e:?} of {c:?}");
                assert!(ps.contains(&c), "{c:?} not a parent of {e:?}");
                assert!(
                    neighbors(ps[0]).contains(&ps[1]),
                    "parents of {e:?} are not neighbours"
                );
                edge_children += 1;
            }
            for x in &ch {
                assert!(parents(*x).contains(&c));
            }
        }
    }
    // And upward: every child listed by a parent lists that parent back.
    for level in 1..=6u8 {
        for c in all_cells(level) {
            let ps = parents(c);
            assert!(matches!(ps.len(), 1 | 2));
            for p in &ps {
                assert!(children(*p).contains(&c));
            }
            assert_eq!(primary_parent(c), Some(ps[0]));
        }
    }
    println!(
        "S1 parent/child: centre links={centre_children} edge links={edge_children}, all round trip"
    );
}

#[test]
fn s1_children_cover_next_level() {
    for level in 0..=5u8 {
        let mut covered = BTreeSet::new();
        let mut max_parents = 0;
        let mut two_parent = 0usize;
        for c in all_cells(level) {
            covered.extend(children(c));
        }
        let next = all_cells(level + 1);
        for x in &next {
            let np = parents(*x).len();
            max_parents = max_parents.max(np);
            if np == 2 {
                two_parent += 1;
            }
        }
        assert_eq!(covered.len(), next.len(), "coverage at L{}", level + 1);
        assert!(next.iter().all(|x| covered.contains(x)));
        // Closed form: L+1 cells with two parents = edge midpoints of the level-L lattice.
        let edges_l = 3 * count_for(level) - 6; // E = 3V - 6 for a triangulated sphere
        assert_eq!(two_parent, edges_l, "two-parent cells = lattice edges");
        println!(
            "S1 cover L{level}->L{}: cells={} covered={} two_parent={two_parent} (= level-L lattice edges {edges_l}) max_parents={max_parents}",
            level + 1,
            next.len(),
            covered.len()
        );
    }
}

#[test]
fn s1_pack_unpack_and_rejects() {
    let mut n = 0usize;
    let mut ids = BTreeSet::new();
    for level in 0..=6u8 {
        for c in all_cells(level) {
            let p = pack(c);
            assert_eq!(unpack(p), Some(c), "round trip {c:?}");
            assert!(ids.insert(p), "id collision {c:?}");
            n += 1;
        }
    }
    let ok = pack(Cell {
        base: 3,
        level: 5,
        i: 7,
        j: 9,
    });
    assert!(unpack(ok).is_some());
    for bit in 0..7 {
        assert_eq!(unpack(ok | (1 << bit)), None, "stray low bit {bit}");
    }
    // level above 24 (5 bit field allows up to 31)
    assert_eq!(unpack((3u64 << 60) | (25u64 << 55)), None);
    assert!(unpack((3u64 << 60) | (24u64 << 55)).is_some());
    // base 12..=15
    for base in 12u64..=15 {
        assert_eq!(unpack(base << 60), None, "base {base}");
    }
    // Morton out of range: i = 2^level at level 3
    let oob = (1u64 << 60) | (3u64 << 55) | (spread(8) << 7);
    assert_eq!(unpack(oob), None);
    let oob_j = (1u64 << 60) | (3u64 << 55) | ((spread(8) << 1) << 7);
    assert_eq!(unpack(oob_j), None);
    // Poles carry no lattice coordinates.
    assert_eq!(
        unpack((11u64 << 60) | (2u64 << 55) | (spread(1) << 7)),
        None
    );
    assert!(unpack((11u64 << 60) | (2u64 << 55)).is_some());
    // Morton bits at the top of the 48-bit field are accepted at level 24
    let hi = pack(Cell {
        base: 1,
        level: 24,
        i: (1 << 24) - 1,
        j: (1 << 24) - 1,
    });
    assert!(unpack(hi).is_some());
    println!("S1 ids: {n} cells packed, unique, round trip; rejects verified");
}
