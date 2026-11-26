//! Patch mesh topology, winding, border normals, determinism, cancellation and payload codec.

use engine_sdk::DVec3;
use engine_sdk::procedural::{Fnv1a64, splitmix64};
use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
use planetary_core::mesh::{PatchMesh, grid_indices, index_count, skirt_depth, vertex_count};
use planetary_core::payload::{
    HEADER_LEN, PAYLOAD_MAGIC, PayloadError, decode_payload, encode_payload, payload_len,
};
use planetary_core::{
    Edge, Face, PatchId, PlanetTerrain, TerrainParams, build_patch, build_patch_cancellable,
    neighbor, patch_center_dir, vertex_dir,
};

const SEED: u64 = 0x0007_5EED_2026_0001;
const GOLDEN_MESH: u64 = 0x51f4_dccc_04db_d5bd; // Windows-computed; cross-platform CI pending

fn terrain(n: u32) -> PlanetTerrain {
    PlanetTerrain::new(SEED, TerrainParams::default(), n).expect("terrain")
}

fn model(t: &PlanetTerrain) -> LodModel {
    LodModel::from_terrain(t, DEFAULT_KAPPA)
}

fn pid(face: Face, level: u8, x: u32, y: u32) -> PatchId {
    PatchId::new(face, level, x, y).expect("patch id")
}

fn sample_patches() -> Vec<PatchId> {
    let mut v = vec![
        PatchId::root(Face::PosX),
        PatchId::root(Face::NegZ),
        pid(Face::PosY, 3, 7, 3),
        pid(Face::PosZ, 2, 0, 0),
        pid(Face::NegX, 4, 15, 15),
        pid(Face::PosX, 19, 262_144, 262_144),
    ];
    let mut s = 0x1234u64;
    while v.len() < 20 {
        s = splitmix64(s);
        let face = Face::ALL[(s % 6) as usize];
        let level = ((s >> 8) % 12) as u8;
        let k = 1u64 << level;
        let x = ((s >> 16) % k) as u32;
        let y = ((s >> 40) % k) as u32;
        v.push(pid(face, level, x, y));
    }
    v
}

fn world(m: &PatchMesh, k: usize) -> DVec3 {
    let p = m.vertices[k].position;
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
fn counts_and_topology() {
    for n in [8u32, 16, 32] {
        let t = terrain(n);
        let m = model(&t);
        let mesh = build_patch(&t, &m, pid(Face::PosX, 2, 1, 2));
        assert_eq!(mesh.vertices.len(), vertex_count(n));
        assert_eq!(mesh.indices, grid_indices(n));
        assert_eq!(mesh.indices.len(), index_count(n));
        assert!(mesh.indices.iter().all(|&i| (i as usize) < vertex_count(n)));
        assert_eq!(vertex_count(n), ((n + 1) * (n + 1) + 4 * (n + 1)) as usize);
        assert_eq!(index_count(n), (6 * n * n + 24 * n) as usize);
    }
}

#[test]
fn winding_is_ccw_outside_for_grid_and_skirts() {
    let n = 8;
    let t = terrain(n);
    let m = model(&t);
    let grid_tris = (n * n * 2) as usize;
    for id in sample_patches() {
        let mesh = build_patch(&t, &m, id);
        let center = patch_center_dir(id);
        for (ti, tri) in mesh.indices.chunks(3).enumerate() {
            let p: Vec<DVec3> = tri.iter().map(|&i| world(&mesh, i as usize)).collect();
            let nrm = cross(sub(p[1], p[0]), sub(p[2], p[0]));
            if ti < grid_tris {
                let c = DVec3::new(
                    (p[0].x + p[1].x + p[2].x) / 3.0,
                    (p[0].y + p[1].y + p[2].y) / 3.0,
                    (p[0].z + p[1].z + p[2].z) / 3.0,
                );
                assert!(dot(nrm, c) > 0.0, "grid tri {ti} of {id:?} faces inward");
            } else {
                let e = Edge::ALL[(ti - grid_tris) / (2 * n as usize)];
                let (i, j) = e.vertex(n, n / 2);
                let d = vertex_dir(id, n, i, j);
                let tangent = sub(d, center);
                let rr = dot(tangent, center);
                let tangent = sub(
                    tangent,
                    DVec3::new(center.x * rr, center.y * rr, center.z * rr),
                );
                assert!(
                    dot(nrm, tangent) >= 0.0,
                    "skirt tri {ti} edge {e:?} of {id:?} faces the patch interior"
                );
            }
        }
    }
}

#[test]
fn skirt_vertices_hang_below_border() {
    let n = 8;
    let t = terrain(n);
    let m = model(&t);
    for id in sample_patches() {
        let mesh = build_patch(&t, &m, id);
        let depth = skirt_depth(&m, id.level());
        assert_eq!(mesh.skirt_depth, depth);
        let tol = 8.0 * f64::from(mesh.bound_radius) * 2f64.powi(-23) + 1e-9 * depth;
        let w = (n + 1) as usize;
        for e in Edge::ALL {
            for k in 0..=n {
                let (i, j) = e.vertex(n, k);
                let b = world(&mesh, j as usize * w + i as usize);
                let s = world(&mesh, w * w + e as usize * w + k as usize);
                let d = vertex_dir(id, n, i, j);
                let want = DVec3::new(b.x - d.x * depth, b.y - d.y * depth, b.z - d.z * depth);
                let err = sub(s, want);
                assert!(
                    dot(err, err).sqrt() <= tol,
                    "skirt vertex off by {} (tol {tol})",
                    dot(err, err).sqrt()
                );
            }
        }
    }
}

#[test]
fn normals_unit_outward_and_uv_in_range() {
    let n = 8;
    let t = terrain(n);
    let m = model(&t);
    for id in sample_patches() {
        let mesh = build_patch(&t, &m, id);
        let w = (n + 1) as usize;
        for (k, v) in mesh.vertices.iter().enumerate() {
            let nn = DVec3::new(
                f64::from(v.normal[0]),
                f64::from(v.normal[1]),
                f64::from(v.normal[2]),
            );
            assert!((dot(nn, nn).sqrt() - 1.0).abs() <= 1e-5, "normal length");
            let kk = if k < w * w { k } else { 0 };
            let (i, j) = ((kk % w) as u32, (kk / w) as u32);
            let d = vertex_dir(id, n, i, j);
            if k < w * w {
                assert!(dot(nn, d) > 0.0, "normal points inward at {k} of {id:?}");
            }
            assert!(v.uv.iter().all(|c| (0.0..=1.0).contains(c)), "uv range");
        }
    }
}

#[test]
fn border_normals_identical_across_same_level_neighbors() {
    let n = 8u32;
    let t = terrain(n);
    let m = model(&t);
    let w = (n + 1) as usize;
    let mut checked = 0u32;
    for level in 0..=3u8 {
        for id in PatchId::level_iter(level) {
            let a = build_patch(&t, &m, id);
            for e in Edge::ALL {
                let nb = neighbor(id, e);
                let b = build_patch(&t, &m, nb.patch);
                for k in 0..=n {
                    let (i, j) = e.vertex(n, k);
                    let kb = if nb.flipped { n - k } else { k };
                    let (ib, jb) = nb.edge.vertex(n, kb);
                    let na = a.vertices[j as usize * w + i as usize].normal;
                    let nbn = b.vertices[jb as usize * w + ib as usize].normal;
                    for c in 0..3 {
                        assert_eq!(na[c].to_bits(), nbn[c].to_bits(), "{id:?} {e:?} t={k}");
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 0);
}

#[test]
fn deterministic_and_golden() {
    let n = 8;
    let t = terrain(n);
    let m = model(&t);
    let mut h = Fnv1a64::new();
    for id in [PatchId::root(Face::PosX), pid(Face::PosZ, 2, 0, 0)] {
        let a = encode_payload(&build_patch(&t, &m, id));
        let b = encode_payload(&build_patch(&t, &m, id));
        assert_eq!(a, b);
        h.write(&a);
    }
    eprintln!("GOLDEN_MESH = {:#018x}", h.finish());
    assert_eq!(h.finish(), GOLDEN_MESH);
}

#[test]
fn cancellation() {
    let t = terrain(8);
    let m = model(&t);
    let id = pid(Face::NegY, 3, 1, 1);
    assert!(build_patch_cancellable(&t, &m, id, &|| true).is_none());
    assert_eq!(
        build_patch_cancellable(&t, &m, id, &|| false),
        Some(build_patch(&t, &m, id))
    );
}

fn valid_payload() -> (Vec<u8>, PatchId, u32, PatchMesh) {
    let n = 8;
    let t = terrain(n);
    let m = model(&t);
    let id = pid(Face::PosY, 3, 7, 3);
    let mesh = build_patch(&t, &m, id);
    (encode_payload(&mesh), id, n, mesh)
}

#[test]
fn payload_round_trip() {
    let (bytes, id, n, mesh) = valid_payload();
    assert_eq!(bytes.len(), payload_len(n));
    assert_eq!(decode_payload(&bytes, id, n), Ok(mesh));
}

#[test]
fn payload_rejects_malformed() {
    let (bytes, id, n, _) = valid_payload();
    let other = pid(Face::PosY, 3, 7, 2);
    assert!(matches!(
        decode_payload(&bytes[..bytes.len() - 1], id, n),
        Err(PayloadError::Truncated { .. })
    ));
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(matches!(
        decode_payload(&extra, id, n),
        Err(PayloadError::Truncated { .. })
    ));
    let mut b = bytes.clone();
    b[0] ^= 0xff;
    assert!(matches!(
        decode_payload(&b, id, n),
        Err(PayloadError::BadMagic(_))
    ));
    assert_eq!(PAYLOAD_MAGIC.to_le_bytes(), *b"PTCH");
    let mut b = bytes.clone();
    b[4..8].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(decode_payload(&b, id, n), Err(PayloadError::BadVersion(2)));
    assert!(matches!(
        decode_payload(&bytes, other, n),
        Err(PayloadError::IdMismatch { .. })
    ));
    let mut b = bytes.clone();
    b[16..20].copy_from_slice(&5u32.to_le_bytes());
    assert!(matches!(
        decode_payload(&b, id, n),
        Err(PayloadError::CountMismatch { .. })
    ));
    let mut b = bytes.clone();
    b[20..24].copy_from_slice(&5u32.to_le_bytes());
    assert!(matches!(
        decode_payload(&b, id, n),
        Err(PayloadError::CountMismatch { .. })
    ));
    let mut b = bytes.clone();
    b[HEADER_LEN..HEADER_LEN + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    assert_eq!(decode_payload(&b, id, n), Err(PayloadError::NonFinite));
    let mut b = bytes.clone();
    let off = HEADER_LEN + vertex_count(n) * 32;
    b[off..off + 4].copy_from_slice(&(vertex_count(n) as u32).to_le_bytes());
    assert_eq!(
        decode_payload(&b, id, n),
        Err(PayloadError::IndexOutOfRange(vertex_count(n) as u32))
    );
}

#[test]
fn payload_fuzz_never_panics() {
    let (bytes, id, n, _) = valid_payload();
    let mut s = 0xFEED_u64;
    for _ in 0..2000 {
        s = splitmix64(s);
        let mut b = bytes.clone();
        let pos = (s % b.len() as u64) as usize;
        b[pos] ^= ((s >> 32) as u8) | 1;
        let _ = decode_payload(&b, id, n);
    }
    for _ in 0..200 {
        s = splitmix64(s);
        let cut = (s % bytes.len() as u64) as usize;
        assert!(decode_payload(&bytes[..cut], id, n).is_err());
    }
}
