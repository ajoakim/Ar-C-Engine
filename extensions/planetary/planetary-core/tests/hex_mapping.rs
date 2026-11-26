//! Hex projection and sample lattice (07.1-12): bit identity on every shared border, locate,
//! footprints, area distortion and goldens.

use std::collections::BTreeMap;

use engine_sdk::DVec3;
use planetary_core::hex::mapping::{CellFrame, LatticePoint};
use planetary_core::hex::{
    CellId, FACES, adjacency, cell_center_dir, cell_center_point, cell_corner_points, footprint,
    locate, ring, sample_dir, vertices,
};

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

    fn direction(&mut self) -> DVec3 {
        loop {
            let p = DVec3::new(
                self.unit() * 2.0 - 1.0,
                self.unit() * 2.0 - 1.0,
                self.unit() * 2.0 - 1.0,
            );
            let l2 = p.x * p.x + p.y * p.y + p.z * p.z;
            if l2 > 1e-6 && l2 <= 1.0 {
                let l = l2.sqrt();
                return DVec3::new(p.x / l, p.y / l, p.z / l);
            }
        }
    }

    fn cell(&mut self, level: u8) -> CellId {
        let base = (self.next() % 12) as u8;
        let n = 1u64 << level;
        let (i, j) = if base == 0 || base == 11 {
            (0, 0)
        } else {
            ((self.next() % n) as u32, (self.next() % n) as u32)
        };
        CellId::new(base, level, i, j).expect("valid cell")
    }
}

fn bits(v: DVec3) -> [u64; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

fn dot(a: DVec3, b: DVec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn fnv1a(h: &mut u64, v: u64) {
    for byte in v.to_le_bytes() {
        *h ^= u64::from(byte);
        *h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
}

#[test]
fn edges_and_vertices_bit_identical() {
    let mut edges: BTreeMap<(u8, u8), Vec<u8>> = BTreeMap::new();
    for (f, tri) in (0u8..).zip(FACES) {
        for (x, u) in tri.iter().enumerate() {
            for w in &tri[x + 1..] {
                edges.entry((*u, *w)).or_default().push(f);
            }
        }
    }
    assert_eq!(edges.len(), 30);
    let mut total = 0u64;
    for level in 0..=6u32 {
        let d = 3 * 16 * (1u64 << level);
        for ((u, w), faces) in &edges {
            assert_eq!(faces.len(), 2);
            for a in 0..=d {
                let make = |f: u8| {
                    let fv = FACES[usize::from(f)];
                    let wt = fv.map(|v| {
                        if v == *u {
                            a
                        } else if v == *w {
                            d - a
                        } else {
                            0
                        }
                    });
                    LatticePoint {
                        face: f,
                        a: wt[0],
                        b: wt[1],
                        c: wt[2],
                        d,
                    }
                };
                let (p, q) = (make(faces[0]), make(faces[1]));
                assert_eq!(p.canonical(), q.canonical());
                assert_eq!(bits(sample_dir(p)), bits(sample_dir(q)));
                total += 1;
            }
        }
        for vertex in 0..12u8 {
            let mut reference = None;
            let mut count = 0;
            for (f, tri) in (0u8..).zip(FACES) {
                if !tri.contains(&vertex) {
                    continue;
                }
                let wt = tri.map(|v| if v == vertex { d } else { 0 });
                let p = LatticePoint {
                    face: f,
                    a: wt[0],
                    b: wt[1],
                    c: wt[2],
                    d,
                };
                let got = (p.canonical(), bits(sample_dir(p)));
                match reference {
                    None => reference = Some(got),
                    Some(r) => assert_eq!(r, got),
                }
                count += 1;
                total += 1;
            }
            assert_eq!(count, 5);
        }
    }
    println!("edge+vertex samples compared: {total}");
}

#[test]
fn shared_cell_borders_bit_identical() {
    let n = 16u32;
    let mut checked = 0u64;
    for level in 0..=4u8 {
        let frames: BTreeMap<CellId, (CellFrame, Vec<CellId>)> = CellId::level_iter(level)
            .map(|c| (c, (CellFrame::new(c, n), ring(c))))
            .collect();
        for (c, (fc, rc)) in &frames {
            for (k, b) in (0u8..).zip(rc) {
                let (fb, rb) = &frames[b];
                let j = rb.iter().position(|x| x == c).expect("reciprocal") as u8;
                let side = |f: &CellFrame, sector: u8| -> BTreeMap<LatticePoint, [u64; 3]> {
                    (0..=n)
                        .map(|s| {
                            let p = f.sample(sector, s, n - s);
                            (p.canonical(), bits(sample_dir(p)))
                        })
                        .collect()
                };
                let (x, y) = (side(fc, k), side(fb, j));
                assert_eq!(x.len(), n as usize + 1, "{c:?} border has distinct samples");
                assert_eq!(x, y, "border {c:?} - {b:?} at level {level}");
                checked += x.len() as u64;
            }
        }
    }
    println!("border samples compared: {checked}");
}

#[test]
fn coarse_equals_even_fine() {
    let n = 16u32;
    let mut rng = SplitMix(0xC0A5_E001);
    let mut checked = 0u64;
    for _ in 0..2_000 {
        let level = (rng.next() % 11) as u8;
        let c = rng.cell(level);
        let fc = CellFrame::new(c, n);
        let mut kids = Vec::new();
        adjacency::children(c, &mut kids);
        let fine: Vec<CellFrame> = kids.iter().map(|k| CellFrame::new(*k, n)).collect();
        for (k, _) in (0u8..).zip(ring(c)) {
            let mid = fc.sample(k, n / 2, n / 2).canonical();
            let f = fine
                .iter()
                .find(|f| f.center_point().canonical() == mid)
                .expect("edge child at the border midpoint");
            let corner_sector = |p: LatticePoint| {
                (0..f.sectors())
                    .find(|j| f.corner_point(*j).canonical() == p.canonical())
                    .expect("fine corner at coarse corner")
            };
            let j1 = corner_sector(fc.corner_point(k));
            let j2 = corner_sector(fc.sample(k, 0, n));
            for i in 0..=n {
                let coarse = fc.sample(k, n - i, i);
                let (j, sf) = if i <= n / 2 {
                    (j1, n - 2 * i)
                } else {
                    (j2, 2 * (i - n / 2))
                };
                let fp = f.sample(j, sf, 0);
                assert_eq!(coarse.canonical(), fp.canonical());
                assert_eq!(bits(sample_dir(coarse)), bits(sample_dir(fp)));
                checked += 1;
            }
        }
    }
    println!("coarse/fine border samples compared: {checked}");
}

fn centroid(f: usize) -> [f64; 3] {
    let v = vertices();
    let t = FACES[f];
    let (a, b, c) = (
        v[usize::from(t[0])],
        v[usize::from(t[1])],
        v[usize::from(t[2])],
    );
    [
        (a[0] + b[0]) + c[0],
        (a[1] + b[1]) + c[1],
        (a[2] + b[2]) + c[2],
    ]
}

#[test]
fn locate_round_trip() {
    let mut cells = 0u64;
    for level in 0..=5u8 {
        for c in CellId::level_iter(level) {
            assert_eq!(locate(cell_center_dir(c, 2), level), c, "{c:?}");
            cells += 1;
        }
    }
    let mut rng = SplitMix(0x10CA_7E02);
    for level in [16u8, 24] {
        for _ in 0..5_000 {
            let c = rng.cell(level);
            assert_eq!(locate(cell_center_dir(c, 2), level), c, "{c:?}");
            cells += 1;
        }
    }
    println!("round trips: {cells}");

    // Degenerate input returns the north pole cell instead of panicking.
    for bad in [
        DVec3::new(f64::NAN, 0.0, 1.0),
        DVec3::new(f64::INFINITY, 0.0, 0.0),
        DVec3::new(0.0, 0.0, 0.0),
    ] {
        assert_eq!(locate(bad, 7), CellId::north_pole(7));
    }
}

#[test]
fn locate_matches_face_lattice_brute_force() {
    let level = 5u8;
    let n = 1i64 << level;
    // Cell by canonical centre lattice point.
    let by_point: BTreeMap<LatticePoint, CellId> = CellId::level_iter(level)
        .map(|c| (cell_center_point(c, 2).canonical(), c))
        .collect();
    let v = vertices();
    let mut rng = SplitMix(0x10CA_7E01);
    let (samples, mut mismatch, mut angular) = (10_000u32, 0u32, 0u32);
    let centres: Vec<(CellId, DVec3)> = CellId::level_iter(level)
        .map(|c| (c, cell_center_dir(c, 2)))
        .collect();
    for _ in 0..samples {
        let d = rng.direction();
        let got = locate(d, level);
        let dv = [d.x, d.y, d.z];
        let mut face = 0usize;
        let mut best = f64::MIN;
        for f in 0..20 {
            let c = centroid(f);
            let s = dv[0] * c[0] + dv[1] * c[1] + dv[2] * c[2];
            if s > best {
                best = s;
                face = f;
            }
        }
        let t = FACES[face];
        let [a, b, c] = t.map(|x| v[usize::from(x)]);
        let cr = |p: [f64; 3], q: [f64; 3]| {
            [
                p[1] * q[2] - p[2] * q[1],
                p[2] * q[0] - p[0] * q[2],
                p[0] * q[1] - p[1] * q[0],
            ]
        };
        let dt = |p: [f64; 3], q: [f64; 3]| p[0] * q[0] + p[1] * q[1] + p[2] * q[2];
        let (wa, wb, wc) = (dt(dv, cr(b, c)), dt(a, cr(dv, c)), dt(a, cr(b, dv)));
        let s = wa + wb + wc;
        let x = [wa / s * n as f64, wb / s * n as f64, wc / s * n as f64];
        let mut best_d = f64::MAX;
        let mut best_w = [0i64; 3];
        for i in 0..=n {
            for j in 0..=(n - i) {
                let w = [i, j, n - i - j];
                let e: f64 = (0..3).map(|q| (x[q] - w[q] as f64).powi(2)).sum();
                if e < best_d {
                    best_d = e;
                    best_w = w;
                }
            }
        }
        let p = LatticePoint {
            face: face as u8,
            a: best_w[0] as u64 * 6,
            b: best_w[1] as u64 * 6,
            c: best_w[2] as u64 * 6,
            d: 6 * n as u64,
        };
        let want = by_point[&p.canonical()];
        if want != got {
            mismatch += 1;
        }
        let near = centres
            .iter()
            .max_by(|p, q| dot(d, p.1).total_cmp(&dot(d, q.1)))
            .expect("cells")
            .0;
        if near != got {
            angular += 1;
        }
    }
    println!(
        "locate L{level}: {samples} samples, mismatches vs face-lattice brute force {mismatch}; \
         differs from spherical nearest centre {angular}"
    );
    assert_eq!(mismatch, 0);
}

#[test]
fn footprint_contains_samples() {
    let n = 16u32;
    let mut rng = SplitMix(0xF007_0001);
    let mut cells: Vec<CellId> = CellId::level_iter(3)
        .filter(|c| adjacency::is_pentagon(*c))
        .collect();
    assert_eq!(cells.len(), 12);
    for _ in 0..500 {
        let level = (rng.next() % 13) as u8;
        cells.push(rng.cell(level));
    }
    let mut samples = 0u64;
    for c in cells {
        let fp = footprint(c);
        let f = CellFrame::new(c, n);
        for k in 0..f.sectors() {
            for s in 0..=n {
                for t in 0..=(n - s) {
                    let d = sample_dir(f.sample(k, s, t));
                    assert!(
                        dot(d, fp.center) >= fp.cos_delta,
                        "{c:?} sector {k} ({s},{t})"
                    );
                    samples += 1;
                }
            }
        }
        let mut corners = Vec::new();
        cell_corner_points(c, 4, &mut corners);
        assert_eq!(corners.len(), if adjacency::is_pentagon(c) { 5 } else { 6 });
    }
    println!("footprint samples checked: {samples}");
}

fn tri_area(a: DVec3, b: DVec3, c: DVec3) -> f64 {
    let ab = DVec3::new(b.x - a.x, b.y - a.y, b.z - a.z);
    let ac = DVec3::new(c.x - a.x, c.y - a.y, c.z - a.z);
    let x = DVec3::new(
        ab.y * ac.z - ab.z * ac.y,
        ab.z * ac.x - ab.x * ac.z,
        ab.x * ac.y - ab.y * ac.x,
    );
    dot(x, x).sqrt() / 2.0
}

fn area_ratio(level: u8) -> f64 {
    let (mut min, mut max) = (f64::MAX, 0.0f64);
    for c in CellId::level_iter(level) {
        if adjacency::is_pentagon(c) {
            continue;
        }
        let f = CellFrame::new(c, 2);
        let p = sample_dir(f.center_point());
        let m = f.sectors();
        let mut area = 0.0;
        for k in 0..m {
            let c0 = sample_dir(f.sample(k, 2, 0));
            let c1 = sample_dir(f.sample(k, 0, 2));
            let mid = sample_dir(f.sample(k, 1, 1));
            area += tri_area(p, c0, mid) + tri_area(p, mid, c1);
        }
        min = min.min(area);
        max = max.max(area);
    }
    max / min
}

#[test]
fn hex_area_ratio() {
    let r5 = area_ratio(5);
    let r7 = area_ratio(7);
    println!("hex area max/min: L5 {r5:.4}, L7 {r7:.4}");
    assert!(r5 <= 2.0, "L5 ratio {r5}");
    assert!(r7 <= 2.0, "L7 ratio {r7}");
}

/// Windows-computed (toolchain 1.99.0); Linux/macOS confirmation pending hosted CI (GitHub
/// billing), same status as the ADR-0008/0010 goldens.
const GOLDEN_HEX_DIRS: u64 = 0x6f3b_d3fa_d334_4b5f;

#[test]
fn golden_hex_dirs() {
    let n = 16u32;
    let mut cells: Vec<CellId> = CellId::level_iter(3)
        .filter(|c| adjacency::is_pentagon(*c))
        .collect();
    let mut rng = SplitMix(0x601D_0001);
    for i in 0..50u64 {
        cells.push(rng.cell((i % 17) as u8));
    }
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    let mut samples = 0u64;
    for c in cells {
        let f = CellFrame::new(c, n);
        for k in 0..f.sectors() {
            for s in 0..=n {
                for t in 0..=(n - s) {
                    for b in bits(sample_dir(f.sample(k, s, t))) {
                        fnv1a(&mut h, b);
                    }
                    samples += 1;
                }
            }
        }
    }
    println!("golden samples: {samples}, hash: {h:#018x}");
    assert_eq!(h, GOLDEN_HEX_DIRS);
}

#[test]
fn ring_is_a_ccw_cycle() {
    let mut count = 0u64;
    for level in 0..=4u8 {
        for c in CellId::level_iter(level) {
            let r = ring(c);
            assert_eq!(r.len(), if adjacency::is_pentagon(c) { 5 } else { 6 });
            let centre = cell_center_dir(c, 2);
            for (a, b) in r.iter().zip(r.iter().cycle().skip(1)) {
                let mut nb = Vec::new();
                adjacency::neighbors(*a, &mut nb);
                assert!(nb.contains(b));
                if level >= 1 {
                    // Counter-clockwise seen from outside: positive triple product.
                    let (da, db) = (cell_center_dir(*a, 2), cell_center_dir(*b, 2));
                    let cr = DVec3::new(
                        da.y * db.z - da.z * db.y,
                        da.z * db.x - da.x * db.z,
                        da.x * db.y - da.y * db.x,
                    );
                    assert!(dot(centre, cr) > 0.0, "{c:?} ring orientation");
                }
            }
            count += 1;
        }
    }
    println!("rings checked: {count}");
}
