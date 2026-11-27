//! Shared deterministic setup for the ECS decision spike (ADR-0017).
//!
//! Tooling crate: it sits outside `crates/` and `extensions/`, so it is the only
//! place that may depend on `hecs`. Nothing here is used by the engine.
//!
//! The engine `World` implements hierarchy and frame-rebasing semantics that
//! `hecs` does not have. The benches therefore measure the cost of what the
//! engine needs, not feature parity between the two.

use engine_math::DVec3;
use engine_world::Transform;
use glam::{Quat, Vec3};

/// Entity counts compared by the benches.
pub const SIZES: [usize; 2] = [10_000, 100_000];

/// Entities per hierarchy group: one root plus `GROUP - 1` direct children.
pub const GROUP: usize = 10;

/// splitmix64 step: a seeded integer hash, so the setup needs no `rand` crate.
#[must_use]
pub fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Deterministic value in `[-500, 500)` for `(seed, i)`.
#[must_use]
pub fn unit(seed: u64, i: u64) -> f64 {
    let u = (mix(seed ^ mix(i)) >> 11) as f64 / (1u64 << 53) as f64;
    u * 1000.0 - 500.0
}

/// Deterministic local transform for entity `i`.
#[must_use]
pub fn transform(i: usize) -> Transform {
    let i = i as u64;
    Transform::new(
        DVec3::new(unit(1, i), unit(2, i), unit(3, i)),
        Quat::from_rotation_y(unit(4, i) as f32 * 0.001),
        Vec3::ONE,
    )
}

/// Deterministic 1 percent sample of indices below `n`.
#[must_use]
pub fn sample_indices(n: usize) -> Vec<usize> {
    (0..n / 100)
        .map(|k| (mix(0xABCD ^ k as u64) % n as u64) as usize)
        .collect()
}

/// The same small translation nudge applied by both worlds.
#[must_use]
pub fn nudge(mut t: Transform) -> Transform {
    t.translation.x += 0.25;
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_is_deterministic() {
        assert_eq!(transform(7), transform(7));
        assert_eq!(sample_indices(10_000), sample_indices(10_000));
        assert_eq!(sample_indices(10_000).len(), 100);
    }
}
