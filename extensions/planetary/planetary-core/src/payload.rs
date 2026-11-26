//! Binary patch payload for streaming: little-endian, fully validated before use.

use engine_sdk::{DVec3, VertexPosNormalUv};

use crate::mesh::{PatchMesh, index_count, vertex_count};
use crate::patch_id::PatchId;

/// `b"PTCH"` little-endian.
pub const PAYLOAD_MAGIC: u32 = 0x4843_5450;
pub const PAYLOAD_VERSION: u32 = 1;
/// magic u32, version u32, id u64, vertex_count u32, index_count u32, origin 3*f64,
/// bound_radius f32, skirt_depth f64.
pub const HEADER_LEN: usize = 60;

/// Exact encoded size for `n` cells per edge.
#[must_use]
pub fn payload_len(n: u32) -> usize {
    HEADER_LEN + vertex_count(n) * 32 + index_count(n) * 4
}

#[derive(Clone, Debug, PartialEq)]
pub enum PayloadError {
    Truncated { expected: usize, got: usize },
    BadMagic(u32),
    BadVersion(u32),
    IdMismatch { expected: u64, got: u64 },
    CountMismatch { vertices: u32, indices: u32 },
    NonFinite,
    IndexOutOfRange(u32),
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
            PayloadError::IndexOutOfRange(i) => write!(f, "index {i} out of range"),
        }
    }
}

impl std::error::Error for PayloadError {}

/// Serialises `mesh` (counts are taken from the mesh vectors).
#[must_use]
pub fn encode_payload(mesh: &PatchMesh) -> Vec<u8> {
    let mut out =
        Vec::with_capacity(HEADER_LEN + mesh.vertices.len() * 32 + mesh.indices.len() * 4);
    out.extend_from_slice(&PAYLOAD_MAGIC.to_le_bytes());
    out.extend_from_slice(&PAYLOAD_VERSION.to_le_bytes());
    out.extend_from_slice(&mesh.id.raw().to_le_bytes());
    out.extend_from_slice(&(mesh.vertices.len() as u32).to_le_bytes());
    out.extend_from_slice(&(mesh.indices.len() as u32).to_le_bytes());
    out.extend_from_slice(&mesh.origin.x.to_le_bytes());
    out.extend_from_slice(&mesh.origin.y.to_le_bytes());
    out.extend_from_slice(&mesh.origin.z.to_le_bytes());
    out.extend_from_slice(&mesh.bound_radius.to_le_bytes());
    out.extend_from_slice(&mesh.skirt_depth.to_le_bytes());
    for v in &mesh.vertices {
        for f in v.position.iter().chain(&v.normal).chain(&v.uv) {
            out.extend_from_slice(&f.to_le_bytes());
        }
    }
    for i in &mesh.indices {
        out.extend_from_slice(&i.to_le_bytes());
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

/// Validates then decodes `bytes` for patch `expected` with `n` cells per edge.
pub fn decode_payload(bytes: &[u8], expected: PatchId, n: u32) -> Result<PatchMesh, PayloadError> {
    let want = payload_len(n);
    let truncated = PayloadError::Truncated {
        expected: want,
        got: bytes.len(),
    };
    if bytes.len() < HEADER_LEN {
        return Err(truncated);
    }
    let magic = u32_at(bytes, 0).ok_or_else(|| truncated.clone())?;
    if magic != PAYLOAD_MAGIC {
        return Err(PayloadError::BadMagic(magic));
    }
    let version = u32_at(bytes, 4).ok_or_else(|| truncated.clone())?;
    if version != PAYLOAD_VERSION {
        return Err(PayloadError::BadVersion(version));
    }
    let id = u64_at(bytes, 8).ok_or_else(|| truncated.clone())?;
    if id != expected.raw() {
        return Err(PayloadError::IdMismatch {
            expected: expected.raw(),
            got: id,
        });
    }
    let vc = u32_at(bytes, 16).ok_or_else(|| truncated.clone())?;
    let ic = u32_at(bytes, 20).ok_or_else(|| truncated.clone())?;
    if vc as usize != vertex_count(n) || ic as usize != index_count(n) {
        return Err(PayloadError::CountMismatch {
            vertices: vc,
            indices: ic,
        });
    }
    if bytes.len() != want {
        return Err(truncated);
    }
    let rd = |o: usize| f64_at(bytes, o).ok_or(PayloadError::NonFinite);
    let origin = DVec3::new(rd(24)?, rd(32)?, rd(40)?);
    let bound_radius = f32_at(bytes, 48).ok_or(PayloadError::NonFinite)?;
    let skirt_depth = rd(52)?;
    let header_floats = [
        origin.x,
        origin.y,
        origin.z,
        f64::from(bound_radius),
        skirt_depth,
    ];
    if header_floats.iter().any(|f| !f.is_finite()) {
        return Err(PayloadError::NonFinite);
    }
    let mut vertices = Vec::with_capacity(vc as usize);
    let mut o = HEADER_LEN;
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
    let mut indices = Vec::with_capacity(ic as usize);
    for _ in 0..ic {
        let v = u32_at(bytes, o).ok_or(PayloadError::NonFinite)?;
        if v >= vc {
            return Err(PayloadError::IndexOutOfRange(v));
        }
        indices.push(v);
        o += 4;
    }
    Ok(PatchMesh {
        id: expected,
        origin,
        vertices,
        indices,
        bound_radius,
        skirt_depth,
    })
}
