//! Packed 64-bit patch identifier: face (bits 63..61) | level (60..56) | morton(x, y) (55..0).

use crate::face::Face;
use std::fmt;

/// Deepest supported quadtree level.
pub const MAX_LEVEL: u8 = 24;

const LEVEL_SHIFT: u32 = 56;
const FACE_SHIFT: u32 = 61;
const MORTON_MASK: u64 = (1u64 << LEVEL_SHIFT) - 1;

/// Stable identifier of one cube-sphere quadtree patch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatchId(u64);

/// Rejection reasons for malformed ids / coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchIdError {
    BadFace(u8),
    BadLevel(u8),
    CoordOutOfRange { level: u8, x: u32, y: u32 },
    StrayBits(u64),
}

impl fmt::Display for PatchIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatchIdError::BadFace(v) => write!(f, "invalid face index {v}"),
            PatchIdError::BadLevel(v) => write!(f, "invalid level {v} (max {MAX_LEVEL})"),
            PatchIdError::CoordOutOfRange { level, x, y } => {
                write!(f, "coordinate ({x}, {y}) out of range at level {level}")
            }
            PatchIdError::StrayBits(v) => write!(f, "stray morton bits in raw id {v:#x}"),
        }
    }
}

impl std::error::Error for PatchIdError {}

fn interleave(x: u32, y: u32, level: u8) -> u64 {
    let mut m = 0u64;
    for b in 0..u32::from(level) {
        m |= u64::from((x >> b) & 1) << (2 * b);
        m |= u64::from((y >> b) & 1) << (2 * b + 1);
    }
    m
}

fn deinterleave(m: u64, level: u8) -> (u32, u32) {
    let (mut x, mut y) = (0u32, 0u32);
    for b in 0..u32::from(level) {
        x |= (((m >> (2 * b)) & 1) as u32) << b;
        y |= (((m >> (2 * b + 1)) & 1) as u32) << b;
    }
    (x, y)
}

impl PatchId {
    pub const fn root(face: Face) -> PatchId {
        PatchId((face.index() as u64) << FACE_SHIFT)
    }

    pub fn new(face: Face, level: u8, x: u32, y: u32) -> Result<PatchId, PatchIdError> {
        if level > MAX_LEVEL {
            return Err(PatchIdError::BadLevel(level));
        }
        let k = 1u32 << level;
        if x >= k || y >= k {
            return Err(PatchIdError::CoordOutOfRange { level, x, y });
        }
        Ok(PatchId(
            (u64::from(face.index()) << FACE_SHIFT)
                | (u64::from(level) << LEVEL_SHIFT)
                | interleave(x, y, level),
        ))
    }

    pub fn from_raw(raw: u64) -> Result<PatchId, PatchIdError> {
        let face = (raw >> FACE_SHIFT) as u8;
        if Face::from_index(face).is_none() {
            return Err(PatchIdError::BadFace(face));
        }
        let level = ((raw >> LEVEL_SHIFT) & 0x1f) as u8;
        if level > MAX_LEVEL {
            return Err(PatchIdError::BadLevel(level));
        }
        if (raw & MORTON_MASK) >> (2 * u32::from(level)) != 0 {
            return Err(PatchIdError::StrayBits(raw));
        }
        Ok(PatchId(raw))
    }

    /// Builds an id from coordinates already known to satisfy `level <= MAX_LEVEL` and
    /// `x, y < 2^level` (crate-internal; callers derive them from valid ids).
    pub(crate) fn from_parts(face: Face, level: u8, x: u32, y: u32) -> PatchId {
        PatchId(
            (u64::from(face.index()) << FACE_SHIFT)
                | (u64::from(level) << LEVEL_SHIFT)
                | interleave(x, y, level),
        )
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub fn face(self) -> Face {
        // Face bits are validated on construction, so the fallback is unreachable.
        Face::from_index((self.0 >> FACE_SHIFT) as u8).unwrap_or(Face::PosX)
    }

    pub const fn level(self) -> u8 {
        ((self.0 >> LEVEL_SHIFT) & 0x1f) as u8
    }

    pub fn xy(self) -> (u32, u32) {
        deinterleave(self.0 & MORTON_MASK, self.level())
    }

    pub fn parent(self) -> Option<PatchId> {
        self.ancestor_at(self.level().checked_sub(1)?)
    }

    pub fn children(self) -> Option<[PatchId; 4]> {
        let l = self.level();
        if l >= MAX_LEVEL {
            return None;
        }
        let m = (self.0 & MORTON_MASK) << 2;
        let head = ((self.0 >> FACE_SHIFT) << FACE_SHIFT) | (u64::from(l + 1) << LEVEL_SHIFT);
        Some([0u64, 1, 2, 3].map(|c| PatchId(head | m | c)))
    }

    pub fn ancestor_at(self, level: u8) -> Option<PatchId> {
        let l = self.level();
        if level > l {
            return None;
        }
        let m = (self.0 & MORTON_MASK) >> (2 * u32::from(l - level));
        Some(PatchId(
            ((self.0 >> FACE_SHIFT) << FACE_SHIFT) | (u64::from(level) << LEVEL_SHIFT) | m,
        ))
    }

    /// Strict ancestry.
    pub fn is_ancestor_of(self, other: PatchId) -> bool {
        self.level() < other.level() && other.ancestor_at(self.level()) == Some(self)
    }

    /// All `6 * 4^level` ids at `level`, ascending raw order (empty if `level > MAX_LEVEL`).
    pub fn level_iter(level: u8) -> impl Iterator<Item = PatchId> {
        let count = if level <= MAX_LEVEL {
            1u64 << (2 * u32::from(level))
        } else {
            0
        };
        (0..6u64).flat_map(move |f| {
            (0..count)
                .map(move |m| PatchId((f << FACE_SHIFT) | (u64::from(level) << LEVEL_SHIFT) | m))
        })
    }
}
