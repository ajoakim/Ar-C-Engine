//! Hex cell mesh: vertex counts, the 8 index variants, winding, shared-vertex identity,
//! determinism, golden hash and cancellation (HEX-02).

use std::cell::Cell;
use std::collections::BTreeSet;

use engine_sdk::DVec3;
use engine_sdk::procedural::{Fnv1a64, splitmix64};
use planetary_core::CellMesh;
use planetary_core::hex::adjacency::is_pentagon;
use planetary_core::hex::mesh::{
    build_hex_cell, hex_index_variants, vertex_count_hex, vertex_count_pentagon,
};
use planetary_core::hex::pieces::{VARIANT_FULL_HEX, VARIANT_FULL_PENTAGON, half_variant};
use planetary_core::hex::{CellFrame, CellId};
use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
use planetary_core::{PlanetTerrain, TerrainParams};
use planetary_core::{encode_cell_payload, encode_cell_payload_v3};

const SEED: u64 = 0x0007_5EED_2026_0001;
/// Windows-computed; cross-OS pending hosted CI.
const GOLDEN_HEX_MESH_V3: u64 = 0xa826_ad7e_fa59_59b6;
/// Payload v4 (morph fields included). Record with
/// `cargo test -p planetary-core --test hex_mesh golden_hex_mesh_v4 -- --nocapture`.
const GOLDEN_HEX_MESH_V4: u64 = 0x17a0_7e46_1b54_c31b;

fn terrain(n: u32) -> PlanetTerrain {
    PlanetTerrain::new(SEED, TerrainParams::default(), n).expect("terrain")
}

fn sample_angle0(n: u32) -> f64 {
    (8.0 * core::f64::consts::PI / (10.0 * 3.0f64.sqrt())).sqrt() / (3.0f64.sqrt() * f64::from(n))
}

fn model(t: &PlanetTerrain) -> LodModel {
    LodModel::for_topology(t, DEFAULT_KAPPA, sample_angle0(t.cells_per_edge()))
}

fn pentagons(level: u8) -> Vec<CellId> {
    CellId::level_iter(level)
        .filter(|c| is_pentagon(*c))
        .collect()
}

/// `count` seeded hexagons at levels `1..=16` (level 0 has only pentagons).
fn seeded_hexes(count: usize, seed: u64) -> Vec<CellId> {
    let mut v = Vec::new();
    let mut s = seed;
    while v.len() < count {
        s = splitmix64(s);
        let level = 1 + (v.len() % 16) as u8;
        let k = 1u64 << level;
        let base = 1 + ((s >> 4) % 10) as u8;
        let i = ((s >> 8) % k) as u32;
        let j = ((s >> 36) % k) as u32;
        let c = CellId::new(base, level, i, j).expect("cell");
        if !is_pentagon(c) {
            v.push(c);
        }
    }
    v
}

fn build(c: CellId, t: &PlanetTerrain, m: &LodModel) -> CellMesh {
    build_hex_cell(c, t, m, false, &|| false).expect("mesh")
}

fn world(m: &CellMesh, k: u32) -> DVec3 {
    let p = m.vertices[k as usize].position;
    DVec3::new(
        m.origin.x + f64::from(p[0]),
        m.origin.y + f64::from(p[1]),
        m.origin.z + f64::from(p[2]),
    )
}

fn sub(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn cross(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn dot(a: DVec3, b: DVec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

#[test]
fn vertex_counts() {
    assert_eq!(vertex_count_hex(16), 6 * 17 * 18 / 2 + 6 * 17 + 3 * 33);
    assert_eq!(vertex_count_hex(16), 1119);
    assert_eq!(vertex_count_pentagon(16), 5 * 153 + 5 * 17);
    assert_eq!(vertex_count_pentagon(16), 850);
    let t = terrain(16);
    let m = model(&t);
    for c in seeded_hexes(6, 11) {
        assert_eq!(build(c, &t, &m).vertices.len(), 1119);
        assert_eq!(build(c, &t, &m).vertex_class, 0);
    }
    for c in pentagons(2) {
        let mesh = build(c, &t, &m);
        assert_eq!(mesh.vertices.len(), 850);
        assert_eq!(mesh.vertex_class, 1);
    }
}

#[test]
fn variants_valid() {
    for n in [4u32, 16] {
        let vars = hex_index_variants(n);
        assert_eq!(vars.len(), 8);
        let counts = [vertex_count_hex(n), vertex_count_pentagon(n)];
        let names: Vec<&str> = vars.iter().map(|v| v.name).collect();
        assert_eq!(&names[..2], &["hex-full", "pentagon-full"]);
        assert_eq!(names[2], "hex-half-d0-s0");
        assert_eq!(names[7], "hex-half-d2-s1");
        for (i, v) in vars.iter().enumerate() {
            assert!(!v.indices.is_empty(), "{}", v.name);
            assert_eq!(v.indices.len() % 3, 0, "{}", v.name);
            let class = u8::from(i == 1);
            assert_eq!(v.vertex_class, class, "{}", v.name);
            let max = *v.indices.iter().max().expect("non-empty");
            assert!(max < counts[usize::from(class)], "{}: {max}", v.name);
            let tris = (v.indices.len() / 3) as u32;
            let expect = match i {
                0 => 6 * n * n + 6 * 2 * n,
                1 => 5 * n * n + 5 * 2 * n,
                _ => 3 * n * n + 3 * 2 * n + 2 * (2 * n),
            };
            assert_eq!(tris, expect, "{}", v.name);
        }
        assert_eq!(VARIANT_FULL_HEX, 0);
        assert_eq!(VARIANT_FULL_PENTAGON, 1);
    }
}

fn sector_triangles(indices: &[u32], n: u32, sectors: u32) -> BTreeSet<[u32; 3]> {
    let count = (sectors * n * n) as usize;
    indices[..count * 3]
        .chunks(3)
        .map(|t| {
            let mut a = [t[0], t[1], t[2]];
            a.sort_unstable();
            a
        })
        .collect()
}

#[test]
fn halves_partition_full() {
    let n = 8;
    let vars = hex_index_variants(n);
    let full = sector_triangles(&vars[0].indices, n, 6);
    assert_eq!(full.len(), (6 * n * n) as usize);
    for d in 0..3u8 {
        let a = sector_triangles(&vars[usize::from(half_variant(d, 0))].indices, n, 3);
        let b = sector_triangles(&vars[usize::from(half_variant(d, 1))].indices, n, 3);
        assert_eq!(a.len(), (3 * n * n) as usize);
        assert!(a.is_disjoint(&b), "diagonal {d}");
        let union: BTreeSet<[u32; 3]> = a.union(&b).copied().collect();
        assert_eq!(union, full, "diagonal {d}");
    }
}

#[test]
fn winding_outward() {
    let n = 16;
    let t = terrain(n);
    let m = model(&t);
    let vars = hex_index_variants(n);
    let mut cells = seeded_hexes(50, 0xA11);
    cells.extend(pentagons(2).into_iter().take(3));
    let mut checked = 0u32;
    for c in cells {
        let mesh = build(c, &t, &m);
        let pent = is_pentagon(c);
        let var = if pent { &vars[1] } else { &vars[0] };
        let sectors = if pent { 5 } else { 6 };
        let sector_idx = (sectors * n * n * 3) as usize;
        for tri in var.indices[..sector_idx].chunks(3) {
            let (a, b, cc) = (
                world(&mesh, tri[0]),
                world(&mesh, tri[1]),
                world(&mesh, tri[2]),
            );
            let nrm = cross(sub(b, a), sub(cc, a));
            let radial = DVec3::new(a.x + b.x + cc.x, a.y + b.y + cc.y, a.z + b.z + cc.z);
            assert!(dot(nrm, radial) > 0.0, "{c:?}: inward sector triangle");
            checked += 1;
        }
        // Outer skirts face away from the cell.
        for tri in var.indices[sector_idx..].chunks(3) {
            let (a, b, cc) = (
                world(&mesh, tri[0]),
                world(&mesh, tri[1]),
                world(&mesh, tri[2]),
            );
            let nrm = cross(sub(b, a), sub(cc, a));
            let mid = DVec3::new(
                (a.x + b.x + cc.x) / 3.0,
                (a.y + b.y + cc.y) / 3.0,
                (a.z + b.z + cc.z) / 3.0,
            );
            assert!(
                dot(nrm, sub(mid, mesh.origin)) > 0.0,
                "{c:?}: skirt faces in"
            );
        }
    }
    assert!(checked > 0);
}

#[test]
fn half_diagonal_skirts_face_away_from_their_half() {
    let n = 8;
    let t = terrain(n);
    let m = model(&t);
    let vars = hex_index_variants(n);
    for c in seeded_hexes(8, 0xD1A6) {
        let mesh = build(c, &t, &m);
        for d in 0..3u8 {
            for side in 0..2u8 {
                let idx = &vars[usize::from(half_variant(d, side))].indices;
                let sector_len = (3 * n * n * 3) as usize;
                let outer_len = (3 * 2 * n * 3) as usize;
                // Centroid of the half's sector vertices.
                let mut sum = DVec3::new(0.0, 0.0, 0.0);
                for &i in &idx[..sector_len] {
                    let p = world(&mesh, i);
                    sum = DVec3::new(sum.x + p.x, sum.y + p.y, sum.z + p.z);
                }
                let k = sector_len as f64;
                let centre = DVec3::new(sum.x / k, sum.y / k, sum.z / k);
                for tri in idx[sector_len + outer_len..].chunks(3) {
                    let (a, b, cc) = (
                        world(&mesh, tri[0]),
                        world(&mesh, tri[1]),
                        world(&mesh, tri[2]),
                    );
                    let nrm = cross(sub(b, a), sub(cc, a));
                    let mid = DVec3::new(
                        (a.x + b.x + cc.x) / 3.0,
                        (a.y + b.y + cc.y) / 3.0,
                        (a.z + b.z + cc.z) / 3.0,
                    );
                    assert!(
                        dot(nrm, sub(mid, centre)) > 0.0,
                        "{c:?} d{d} s{side}: diagonal skirt faces its own half"
                    );
                }
            }
        }
    }
}

#[test]
fn duplicate_vertices_identical() {
    let n = 8;
    let t = terrain(n);
    let m = model(&t);
    let per = (n + 1) * (n + 2) / 2;
    let at = |k: u32, s: u32, tt: u32| k * per + s * (n + 1) - s * s.saturating_sub(1) / 2 + tt;
    let mut cells = seeded_hexes(10, 0xD0B);
    cells.extend(pentagons(1).into_iter().take(4));
    let mut pairs = 0u32;
    for c in cells {
        let mesh = build(c, &t, &m);
        let sectors = CellFrame::new(c, n).sectors();
        let sectors = u32::from(sectors);
        for k in 0..sectors {
            let next = (k + 1) % sectors;
            for j in 0..=n {
                // sector k, s = 0, t = j  ==  sector k+1, s = j, t = 0
                let (a, b) = (
                    &mesh.vertices[at(k, 0, j) as usize],
                    &mesh.vertices[at(next, j, 0) as usize],
                );
                assert_eq!(a, b, "{c:?} sector {k} j {j}");
                pairs += 1;
            }
            let (a, b) = (
                &mesh.vertices[at(0, 0, 0) as usize],
                &mesh.vertices[at(k, 0, 0) as usize],
            );
            assert_eq!(a, b, "{c:?} centre");
        }
    }
    assert!(pairs > 0);
}

fn golden_hash(encode: fn(&CellMesh) -> Vec<u8>) -> u64 {
    let t = terrain(16);
    let m = model(&t);
    let mut cells: Vec<CellId> = pentagons(2);
    assert_eq!(cells.len(), 12);
    cells.extend(seeded_hexes(20, 0x601D));
    let mut h = Fnv1a64::new();
    for c in cells {
        let a = encode(&build(c, &t, &m));
        let b = encode(&build(c, &t, &m));
        assert_eq!(a, b, "{c:?} not deterministic");
        h.write(&a);
    }
    h.finish()
}

#[test]
fn golden_hex_mesh_v3() {
    let h = golden_hash(encode_cell_payload_v3);
    eprintln!("GOLDEN_HEX_MESH_V3 = {h:#018x}");
    assert_eq!(h, GOLDEN_HEX_MESH_V3);
}

#[test]
fn golden_hex_mesh_v4() {
    let h = golden_hash(encode_cell_payload);
    eprintln!("GOLDEN_HEX_MESH_V4 = {h:#018x}");
    assert_eq!(h, GOLDEN_HEX_MESH_V4);
}

#[test]
fn cancellation() {
    let t = terrain(8);
    let m = model(&t);
    let c = seeded_hexes(1, 5)[0];
    assert!(build_hex_cell(c, &t, &m, false, &|| true).is_none());
    let calls = Cell::new(0u32);
    let after_some = || {
        calls.set(calls.get() + 1);
        calls.get() > 3
    };
    assert!(build_hex_cell(c, &t, &m, false, &after_some).is_none());
    assert!(calls.get() > 3);
    assert!(build_hex_cell(c, &t, &m, false, &|| false).is_some());
}

#[test]
fn invalid_sector_n_is_rejected() {
    let m_t = terrain(8);
    let m = model(&m_t);
    let c = seeded_hexes(1, 6)[0];
    for n in [1u32, 3, 66] {
        if let Ok(t) = PlanetTerrain::new(SEED, TerrainParams::default(), n) {
            assert!(
                build_hex_cell(c, &t, &m, false, &|| false).is_none(),
                "n={n}"
            );
        }
    }
}

#[test]
fn debug_cells_sets_palette_row() {
    let t = terrain(8);
    let m = model(&t);
    let c = seeded_hexes(1, 7)[0];
    let a = build_hex_cell(c, &t, &m, true, &|| false).expect("mesh");
    let v = a.vertices[0].uv[1];
    assert!(a.vertices.iter().all(|x| x.uv[1] == v));
    assert!((0.0..1.0).contains(&v));
}
