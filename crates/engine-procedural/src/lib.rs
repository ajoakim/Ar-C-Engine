//! Deterministic, seeded procedural generation: integer hashing, exact-arithmetic
//! noise and independently seeded generator layers (ADR-0008).
//!
//! Tier 1 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`. Pure and GPU-free: no threads,
//! clocks, I/O or global state; no planetary concepts.

mod error;
mod generator;
mod hash;
mod noise;
mod seed;

pub use error::ProcError;
pub use generator::{Generator, LayerOutput, LayerStack, Params, hash_values};

pub use noise::{fbm_3d, value_noise_2d, value_noise_3d};

pub use hash::{
    FNV_OFFSET_BASIS, FNV_PRIME, Fnv1a64, canonical_bits, fnv1a64, hash2, hash3, splitmix64,
    unit_f64,
};
pub use seed::{StableId, layer_seed};

/// Version of the hashing and noise specification (ADR-0008). Changing any
/// hashed or noise output is a deliberate break: bump this and re-pin goldens.
pub const HASH_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    #[test]
    fn hash_version_is_one() {
        assert_eq!(super::HASH_VERSION, 1);
    }
}
