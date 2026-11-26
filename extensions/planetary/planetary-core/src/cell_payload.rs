//! Topology-agnostic cell payload (version 3): vertices, vertex class and height range, no
//! indices (those are shared per [`crate::IndexVariant`]). Little-endian, fully validated.

use engine_sdk::{DVec3, VertexPosNormalUv};

use crate::topology::CellMesh;

/// `b"CELL"` little-endian.
pub const CELL_PAYLOAD_MAGIC: u32 = 0x4C4C_4543;
pub const CELL_PAYLOAD_VERSION: u32 = 3;
/// magic u32, version u32, raw u64, vertex_class u8 + 3 zero bytes, vertex_count u32,
/// origin 3*f64, bound_radius f32, skirt_depth f64, height_min f64, height_max f64.
pub const CELL_HEADER_LEN: usize = 76;

#[derive(Clone, Debug, PartialEq)]
pub enum PayloadError {
    Truncated { expected: usize, got: usize },
    BadMagic(u32),
    BadVersion(u32),
    IdMismatch { expected: u64, got: u64 },
    CountMismatch { vertices: u32, indices: u32 },
    NonFinite,
    BadRange,
    BadClass(u8),
    BadPadding,
}

impl core::fmt::Display for PayloadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PayloadError::Truncated { expected, got } => {
                write!(f, "payload length {got}, expected {expected}")
            }
            PayloadError::BadMagic(m) => write!(f, "bad payload magic {m:#x}"),
            PayloadError::BadVersion(v) => write!(f, "unsupported payload version {v}"),
            PayloadError::IdMismatch { expected, got } => {
                write!(f, "payload id {got:#x}, expected {expected:#x}")
            }
            PayloadError::CountMismatch { vertices, indices } => {
                write!(f, "bad counts: {vertices} vertices, {indices} indices")
            }
            PayloadError::NonFinite => write!(f, "non-finite float in payload"),
            PayloadError::BadRange => write!(f, "payload height range min > max"),
            PayloadError::BadClass(c) => write!(f, "unknown vertex class {c}"),
            PayloadError::BadPadding => write!(f, "non-zero reserved payload bytes"),
        }
    }
}

impl std::error::Error for PayloadError {}

/// Serialises `mesh` (the vertex count is taken from the mesh).
#[must_use]
pub fn encode_cell_payload(m: &CellMesh) -> Vec<u8> {
    let mut out = Vec::with_capacity(CELL_HEADER_LEN + m.vertices.len() * 32);
    out.extend_from_slice(&CELL_PAYLOAD_MAGIC.to_le_bytes());
    out.extend_from_slice(&CELL_PAYLOAD_VERSION.to_le_bytes());
    out.extend_from_slice(&m.raw.to_le_bytes());
    out.extend_from_slice(&[m.vertex_class, 0, 0, 0]);
    out.extend_from_slice(&(m.vertices.len() as u32).to_le_bytes());
    out.extend_from_slice(&m.origin.x.to_le_bytes());
    out.extend_from_slice(&m.origin.y.to_le_bytes());
    out.extend_from_slice(&m.origin.z.to_le_bytes());
    out.extend_from_slice(&m.bound_radius.to_le_bytes());
    out.extend_from_slice(&m.skirt_depth.to_le_bytes());
    out.extend_from_slice(&m.height_min.to_le_bytes());
    out.extend_from_slice(&m.height_max.to_le_bytes());
    for v in &m.vertices {
        for f in v.position.iter().chain(&v.normal).chain(&v.uv) {
            out.extend_from_slice(&f.to_le_bytes());
        }
    }
    out
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(o..o + 8)?.try_into().ok()?))
}

fn f32_at(b: &[u8], o: usize) -> Option<f32> {
    Some(f32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

fn f64_at(b: &[u8], o: usize) -> Option<f64> {
    Some(f64::from_le_bytes(b.get(o..o + 8)?.try_into().ok()?))
}

/// Validates then decodes `bytes` for the cell `expected_raw`. `vertex_count_of_class` returns the
/// exact vertex count of a class, or `None` for an unknown class.
pub fn decode_cell_payload(
    bytes: &[u8],
    expected_raw: u64,
    vertex_count_of_class: &dyn Fn(u8) -> Option<u32>,
) -> Result<CellMesh, PayloadError> {
    let truncated = |expected: usize| PayloadError::Truncated {
        expected,
        got: bytes.len(),
    };
    if bytes.len() < CELL_HEADER_LEN {
        return Err(truncated(CELL_HEADER_LEN));
    }
    let magic = u32_at(bytes, 0).ok_or_else(|| truncated(CELL_HEADER_LEN))?;
    if magic != CELL_PAYLOAD_MAGIC {
        return Err(PayloadError::BadMagic(magic));
    }
    let version = u32_at(bytes, 4).ok_or_else(|| truncated(CELL_HEADER_LEN))?;
    if version != CELL_PAYLOAD_VERSION {
        return Err(PayloadError::BadVersion(version));
    }
    let raw = u64_at(bytes, 8).ok_or_else(|| truncated(CELL_HEADER_LEN))?;
    if raw != expected_raw {
        return Err(PayloadError::IdMismatch {
            expected: expected_raw,
            got: raw,
        });
    }
    let class = bytes[16];
    if bytes[17..20].iter().any(|b| *b != 0) {
        return Err(PayloadError::BadPadding);
    }
    let want_vc = vertex_count_of_class(class).ok_or(PayloadError::BadClass(class))?;
    let vc = u32_at(bytes, 20).ok_or_else(|| truncated(CELL_HEADER_LEN))?;
    if vc != want_vc {
        return Err(PayloadError::CountMismatch {
            vertices: vc,
            indices: 0,
        });
    }
    let want = CELL_HEADER_LEN + vc as usize * 32;
    if bytes.len() != want {
        return Err(truncated(want));
    }
    let rd = |o: usize| f64_at(bytes, o).ok_or(PayloadError::NonFinite);
    let origin = DVec3::new(rd(24)?, rd(32)?, rd(40)?);
    let bound_radius = f32_at(bytes, 48).ok_or(PayloadError::NonFinite)?;
    let skirt_depth = rd(52)?;
    let height_min = rd(60)?;
    let height_max = rd(68)?;
    let header_floats = [
        origin.x,
        origin.y,
        origin.z,
        f64::from(bound_radius),
        skirt_depth,
        height_min,
        height_max,
    ];
    if header_floats.iter().any(|f| !f.is_finite()) {
        return Err(PayloadError::NonFinite);
    }
    if height_min > height_max {
        return Err(PayloadError::BadRange);
    }
    let mut vertices = Vec::with_capacity(vc as usize);
    let mut o = CELL_HEADER_LEN;
    for _ in 0..vc {
        let mut f = [0.0f32; 8];
        for slot in &mut f {
            let v = f32_at(bytes, o).ok_or(PayloadError::NonFinite)?;
            if !v.is_finite() {
                return Err(PayloadError::NonFinite);
            }
            *slot = v;
            o += 4;
        }
        vertices.push(VertexPosNormalUv {
            position: [f[0], f[1], f[2]],
            normal: [f[3], f[4], f[5]],
            uv: [f[6], f[7]],
        });
    }
    let morph = crate::morph::identity_morph(&vertices);
    Ok(CellMesh {
        raw,
        vertex_class: class,
        origin,
        vertices,
        morph,
        bound_radius,
        skirt_depth,
        height_min,
        height_max,
    })
}
