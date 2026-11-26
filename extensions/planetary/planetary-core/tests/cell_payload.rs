//! Cell payload v4: round trip and robustness against hostile bytes.

use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
use planetary_core::{
    CELL_HEADER_LEN, CELL_PAYLOAD_MAGIC, CELL_PAYLOAD_VERSION, CELL_VERTEX_BYTES, CubeSphere,
    PatchId, PayloadError, PlanetTerrain, TerrainParams, Topology, decode_cell_payload,
    encode_cell_payload, encode_cell_payload_v3, vertex_count,
};

const SEED: u64 = 0x0007_5EED_2026_0001;
const N: u32 = 4;

fn sample() -> (planetary_core::CellMesh, u64) {
    let terrain = PlanetTerrain::new(SEED, TerrainParams::default(), N).expect("terrain");
    let model = LodModel::from_terrain(&terrain, DEFAULT_KAPPA);
    let cube = CubeSphere::new(N);
    let id = PatchId::root(planetary_core::Face::PosY)
        .children()
        .expect("children")[1];
    let m = cube
        .build_mesh(id, &terrain, &model, false, &|| false)
        .expect("mesh");
    (m, id.raw())
}

fn count(class: u8) -> Option<u32> {
    (class == 0).then(|| vertex_count(N) as u32)
}

#[test]
fn cell_payload_round_trip() {
    let (m, raw) = sample();
    let bytes = encode_cell_payload(&m);
    assert_eq!(CELL_VERTEX_BYTES, 52);
    assert_eq!(
        bytes.len(),
        CELL_HEADER_LEN + vertex_count(N) * CELL_VERTEX_BYTES
    );
    let back = decode_cell_payload(&bytes, raw, &count).expect("decode");
    assert_eq!(m.morph.len(), m.vertices.len());
    assert_eq!(back, m);
}

#[test]
fn cell_payload_v3_is_rejected() {
    let (m, raw) = sample();
    let v3 = encode_cell_payload_v3(&m);
    assert_eq!(v3.len(), CELL_HEADER_LEN + vertex_count(N) * 32);
    assert_eq!(
        decode_cell_payload(&v3, raw, &count),
        Err(PayloadError::BadVersion(3))
    );
    // A v4 payload with the version field patched to 3 is rejected too.
    let mut b = encode_cell_payload(&m);
    b[4..8].copy_from_slice(&3u32.to_le_bytes());
    assert_eq!(
        decode_cell_payload(&b, raw, &count),
        Err(PayloadError::BadVersion(3))
    );
}

#[test]
fn v3_encoder_bytes_exclude_morph_fields() {
    let (m, _) = sample();
    let mut other = m.clone();
    for t in &mut other.morph {
        t.position = [9.0, 9.0, 9.0];
        t.uv = [7.0, 7.0];
    }
    assert_eq!(encode_cell_payload_v3(&m), encode_cell_payload_v3(&other));
    assert_ne!(encode_cell_payload(&m), encode_cell_payload(&other));
}

#[test]
fn cell_payload_rejects_every_truncation() {
    let (m, raw) = sample();
    let bytes = encode_cell_payload(&m);
    for len in 0..bytes.len() {
        assert!(
            decode_cell_payload(&bytes[..len], raw, &count).is_err(),
            "len {len}"
        );
    }
    let mut longer = bytes.clone();
    longer.push(0);
    assert!(decode_cell_payload(&longer, raw, &count).is_err());
}

#[test]
fn cell_payload_rejects_bad_header_fields() {
    let (m, raw) = sample();
    let bytes = encode_cell_payload(&m);
    let dec = |b: &[u8]| decode_cell_payload(b, raw, &count);

    let mut b = bytes.clone();
    b[0] ^= 1;
    assert!(matches!(dec(&b), Err(PayloadError::BadMagic(_))));
    assert_eq!(
        u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
        CELL_PAYLOAD_MAGIC
    );

    let mut b = bytes.clone();
    b[4..8].copy_from_slice(&(CELL_PAYLOAD_VERSION - 1).to_le_bytes());
    assert!(matches!(dec(&b), Err(PayloadError::BadVersion(_))));
    b[4..8].copy_from_slice(&2u32.to_le_bytes());
    assert!(matches!(dec(&b), Err(PayloadError::BadVersion(2))));

    assert!(matches!(
        decode_cell_payload(&bytes, raw ^ 1, &count),
        Err(PayloadError::IdMismatch { .. })
    ));

    let mut b = bytes.clone();
    b[16] = 9;
    assert!(matches!(dec(&b), Err(PayloadError::BadClass(9))));

    let mut b = bytes.clone();
    b[18] = 1;
    assert!(matches!(dec(&b), Err(PayloadError::BadPadding)));

    let mut b = bytes.clone();
    b[20..24].copy_from_slice(&1u32.to_le_bytes());
    assert!(matches!(dec(&b), Err(PayloadError::CountMismatch { .. })));
}

#[test]
fn cell_payload_rejects_non_finite_and_bad_range() {
    let (m, raw) = sample();
    let bytes = encode_cell_payload(&m);
    let dec = |b: &[u8]| decode_cell_payload(b, raw, &count);
    // Every float in the header region and the first vertex.
    for off in [24usize, 32, 40, 52, 60, 68] {
        let mut b = bytes.clone();
        b[off..off + 8].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(matches!(dec(&b), Err(PayloadError::NonFinite)), "off {off}");
    }
    let mut b = bytes.clone();
    b[48..52].copy_from_slice(&f32::INFINITY.to_le_bytes());
    assert!(matches!(dec(&b), Err(PayloadError::NonFinite)));
    // All 13 floats of the first vertex and of the last vertex (morph region included).
    let last = CELL_HEADER_LEN + (vertex_count(N) - 1) * CELL_VERTEX_BYTES;
    for off in (CELL_HEADER_LEN..CELL_HEADER_LEN + CELL_VERTEX_BYTES)
        .step_by(4)
        .chain((last..last + CELL_VERTEX_BYTES).step_by(4))
    {
        let mut b = bytes.clone();
        b[off..off + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(matches!(dec(&b), Err(PayloadError::NonFinite)), "off {off}");
    }
    let mut b = bytes.clone();
    b[60..68].copy_from_slice(&1.0e9f64.to_le_bytes());
    b[68..76].copy_from_slice(&(-1.0e9f64).to_le_bytes());
    assert!(matches!(dec(&b), Err(PayloadError::BadRange)));
}

#[test]
fn cell_payload_fuzz_never_panics() {
    let (m, raw) = sample();
    let bytes = encode_cell_payload(&m);
    let mut s = 0xFEED_u64;
    let step = |s: &mut u64| {
        *s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    for _ in 0..2000 {
        let r = step(&mut s);
        let mut b = bytes.clone();
        let pos = (r % b.len() as u64) as usize;
        b[pos] ^= ((r >> 32) as u8) | 1;
        let _ = decode_cell_payload(&b, raw, &count);
    }
    for _ in 0..200 {
        let cut = (step(&mut s) % bytes.len() as u64) as usize;
        assert!(decode_cell_payload(&bytes[..cut], raw, &count).is_err());
    }
}
