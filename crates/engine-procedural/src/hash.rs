//! Fixed, versioned integer hashing (ADR-0008 decisions 7-8). Non-cryptographic.

/// FNV-1a 64-bit offset basis.
pub const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a 64-bit prime.
pub const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// SplitMix64 finaliser (published reference constants).
#[must_use]
pub const fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FNV-1a 64-bit hash of a byte string.
#[must_use]
pub const fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET_BASIS;
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    h
}

/// Streaming FNV-1a 64-bit hasher; integers are written little-endian.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fnv1a64 {
    state: u64,
}

impl Fnv1a64 {
    /// New hasher at the FNV offset basis.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: FNV_OFFSET_BASIS,
        }
    }

    /// Absorb bytes.
    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.state ^= u64::from(b);
            self.state = self.state.wrapping_mul(FNV_PRIME);
        }
    }

    /// Absorb a `u32` little-endian.
    pub fn write_u32(&mut self, v: u32) {
        self.write(&v.to_le_bytes());
    }

    /// Absorb a `u64` little-endian.
    pub fn write_u64(&mut self, v: u64) {
        self.write(&v.to_le_bytes());
    }

    /// Current hash value.
    #[must_use]
    pub const fn finish(&self) -> u64 {
        self.state
    }
}

impl Default for Fnv1a64 {
    fn default() -> Self {
        Self::new()
    }
}

/// Lattice hash of a 2D integer coordinate.
#[must_use]
pub const fn hash2(seed: u64, ix: i64, iy: i64) -> u64 {
    splitmix64(seed ^ splitmix64((ix as u64) ^ splitmix64(iy as u64)))
}

/// Lattice hash of a 3D integer coordinate.
#[must_use]
pub const fn hash3(seed: u64, ix: i64, iy: i64, iz: i64) -> u64 {
    splitmix64(seed ^ splitmix64((ix as u64) ^ splitmix64((iy as u64) ^ splitmix64(iz as u64))))
}

/// Top 53 bits scaled by 2^-53: exact, in [0, 1).
#[must_use]
pub fn unit_f64(h: u64) -> f64 {
    ((h >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
}

/// Canonical bit pattern for hashing: -0.0 becomes 0.0; `None` for NaN or infinite.
#[must_use]
pub fn canonical_bits(v: f64) -> Option<u64> {
    if !v.is_finite() {
        return None;
    }
    if v == 0.0 {
        return Some(0.0f64.to_bits());
    }
    Some(v.to_bits())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_splitmix64_known_answers() {
        assert_eq!(splitmix64(0), 0xE220_A839_7B1D_CDAF);
        assert_eq!(splitmix64(0x9E37_79B9_7F4A_7C15), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(splitmix64(1), 0x910A_2DEC_8902_5CC1);
    }

    #[test]
    fn hash_fnv1a64_known_answers() {
        assert_eq!(fnv1a64(b""), 0xCBF2_9CE4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xAF63_DC4C_8601_EC8C);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_F739_67E8);
        let mut h = Fnv1a64::new();
        h.write(b"foo");
        h.write(b"bar");
        assert_eq!(h.finish(), fnv1a64(b"foobar"));
        assert_eq!(Fnv1a64::default(), Fnv1a64::new());
    }

    #[test]
    fn hash_write_u64_is_little_endian() {
        let mut h = Fnv1a64::new();
        h.write_u64(0x0102_0304_0506_0708);
        assert_eq!(h.finish(), fnv1a64(&[8, 7, 6, 5, 4, 3, 2, 1]));
        let mut g = Fnv1a64::new();
        g.write_u32(0x0102_0304);
        assert_eq!(g.finish(), fnv1a64(&[4, 3, 2, 1]));
    }

    #[test]
    fn hash_lattice_signed_coords() {
        let s = 42;
        assert_ne!(hash3(s, -1, 0, 0), hash3(s, 1, 0, 0));
        assert_eq!(hash2(s, -1, 0), hash2(s, -1, 0));
        let base = hash3(s, 1, 2, 3);
        for v in [
            hash3(s, 2, 2, 3),
            hash3(s, 1, 3, 3),
            hash3(s, 1, 2, 4),
            hash3(s + 1, 1, 2, 3),
            hash3(s, 3, 2, 1),
            hash3(s, -1, -2, -3),
        ] {
            assert_ne!(base, v);
        }
    }

    #[test]
    fn hash_unit_f64_range() {
        assert_eq!(unit_f64(0), 0.0);
        let max = unit_f64(u64::MAX);
        assert!(max < 1.0);
        assert_eq!(max, (((1u64 << 53) - 1) as f64) / 9_007_199_254_740_992.0);
    }

    #[test]
    fn hash_canonical_bits() {
        assert_eq!(canonical_bits(-0.0), Some(0.0f64.to_bits()));
        assert_eq!(canonical_bits(1.5), Some(1.5f64.to_bits()));
        assert_eq!(canonical_bits(f64::NAN), None);
        assert_eq!(canonical_bits(f64::INFINITY), None);
    }
}
