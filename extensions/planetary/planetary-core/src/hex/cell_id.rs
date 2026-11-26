//! Packed 64-bit hex cell identifier:
//! base (63..60: 0 north pole, 1..=10 diamond, 11 south pole) | level (59..55) |
//! morton(i, j) (54..7) | 7 zero bits.

use std::fmt;

/// Deepest supported hex level.
pub const HEX_MAX_LEVEL: u8 = 24;

const BASE_SHIFT: u32 = 60;
const LEVEL_SHIFT: u32 = 55;
const MORTON_SHIFT: u32 = 7;
const MORTON_BITS: u32 = 48;
const STRAY_MASK: u64 = (1 << MORTON_SHIFT) - 1;

/// Stable identifier of one hex cell (a pentagon at the 12 icosahedron vertices).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellId(u64);

/// Rejection reasons for malformed ids / coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellIdError {
    /// Low 7 bits are not zero.
    StrayBits,
    /// Base outside `0..=11`.
    BadBase(u8),
    /// Level above [`HEX_MAX_LEVEL`].
    BadLevel(u8),
    /// `i` or `j` is `>= 2^level`.
    CoordOutOfRange,
    /// A pole carries non-zero lattice coordinates.
    PoleCoord,
}

impl fmt::Display for CellIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellIdError::StrayBits => write!(f, "stray low bits in cell id"),
            CellIdError::BadBase(b) => write!(f, "invalid base {b} (max 11)"),
            CellIdError::BadLevel(l) => write!(f, "invalid level {l} (max {HEX_MAX_LEVEL})"),
            CellIdError::CoordOutOfRange => write!(f, "lattice coordinate out of range"),
            CellIdError::PoleCoord => write!(f, "pole cell with non-zero coordinates"),
        }
    }
}

impl std::error::Error for CellIdError {}

const fn spread(x: u32) -> u64 {
    let mut v = x as u64;
    v = (v | (v << 16)) & 0x0000_FFFF_0000_FFFF;
    v = (v | (v << 8)) & 0x00FF_00FF_00FF_00FF;
    v = (v | (v << 4)) & 0x0F0F_0F0F_0F0F_0F0F;
    v = (v | (v << 2)) & 0x3333_3333_3333_3333;
    v = (v | (v << 1)) & 0x5555_5555_5555_5555;
    v
}

const fn compact(mut v: u64) -> u32 {
    v &= 0x5555_5555_5555_5555;
    v = (v | (v >> 1)) & 0x3333_3333_3333_3333;
    v = (v | (v >> 2)) & 0x0F0F_0F0F_0F0F_0F0F;
    v = (v | (v >> 4)) & 0x00FF_00FF_00FF_00FF;
    v = (v | (v >> 8)) & 0x0000_FFFF_0000_FFFF;
    v = (v | (v >> 16)) & 0x0000_0000_FFFF_FFFF;
    v as u32
}

const fn pack(base: u8, level: u8, i: u32, j: u32) -> u64 {
    ((base as u64) << BASE_SHIFT)
        | ((level as u64) << LEVEL_SHIFT)
        | ((spread(i) | (spread(j) << 1)) << MORTON_SHIFT)
}

impl CellId {
    /// The north pole cell at `level` (caller keeps `level <= HEX_MAX_LEVEL`).
    pub const fn north_pole(level: u8) -> CellId {
        CellId(pack(0, level, 0, 0))
    }

    /// The south pole cell at `level` (caller keeps `level <= HEX_MAX_LEVEL`).
    pub const fn south_pole(level: u8) -> CellId {
        CellId(pack(11, level, 0, 0))
    }

    /// Validated constructor.
    pub fn new(base: u8, level: u8, i: u32, j: u32) -> Result<CellId, CellIdError> {
        if base > 11 {
            return Err(CellIdError::BadBase(base));
        }
        if level > HEX_MAX_LEVEL {
            return Err(CellIdError::BadLevel(level));
        }
        let n = 1u64 << level;
        if u64::from(i) >= n || u64::from(j) >= n {
            return Err(CellIdError::CoordOutOfRange);
        }
        if (base == 0 || base == 11) && (i != 0 || j != 0) {
            return Err(CellIdError::PoleCoord);
        }
        Ok(CellId(pack(base, level, i, j)))
    }

    /// Validates every field of a raw id (stream keys and payload headers are untrusted).
    pub fn from_raw(raw: u64) -> Result<CellId, CellIdError> {
        if raw & STRAY_MASK != 0 {
            return Err(CellIdError::StrayBits);
        }
        let base = (raw >> BASE_SHIFT) as u8;
        let level = ((raw >> LEVEL_SHIFT) & 0x1F) as u8;
        let m = (raw >> MORTON_SHIFT) & ((1u64 << MORTON_BITS) - 1);
        let (i, j) = (compact(m), compact(m >> 1));
        CellId::new(base, level, i, j)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub const fn base(self) -> u8 {
        (self.0 >> BASE_SHIFT) as u8
    }

    pub const fn level(self) -> u8 {
        ((self.0 >> LEVEL_SHIFT) & 0x1F) as u8
    }

    /// Diamond-plane lattice coordinates `(i, j)` (both 0 for poles).
    pub const fn ij(self) -> (u32, u32) {
        let m = (self.0 >> MORTON_SHIFT) & ((1u64 << MORTON_BITS) - 1);
        (compact(m), compact(m >> 1))
    }

    pub const fn is_pole(self) -> bool {
        let b = self.base();
        b == 0 || b == 11
    }

    /// All `10 * 4^level + 2` cells of `level` in ascending raw order.
    pub fn level_iter(level: u8) -> impl Iterator<Item = CellId> {
        let count = 1u64 << (2 * u32::from(level.min(HEX_MAX_LEVEL)));
        let level = level.min(HEX_MAX_LEVEL);
        let diamonds = (1..=10u8).flat_map(move |base| {
            (0..count).map(move |m| {
                CellId(
                    (u64::from(base) << BASE_SHIFT)
                        | (u64::from(level) << LEVEL_SHIFT)
                        | (m << MORTON_SHIFT),
                )
            })
        });
        std::iter::once(CellId::north_pole(level))
            .chain(diamonds)
            .chain(std::iter::once(CellId::south_pole(level)))
    }
}
