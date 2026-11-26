//! Exact-arithmetic value noise (ADR-0008 decision 8): only `+ - * /`, `floor`
//! and exact casts, so results are bit-identical on every platform.

use crate::hash::{hash2, hash3, splitmix64, unit_f64};

#[inline]
fn fade(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// 2D value noise in [0, 1). Inputs must be finite with `|coord| < 2^52`.
#[must_use]
pub fn value_noise_2d(seed: u64, x: f64, y: f64) -> f64 {
    let fx = x.floor();
    let fy = y.floor();
    let ix = fx as i64;
    let iy = fy as i64;
    let tx = fade(x - fx);
    let ty = fade(y - fy);
    let ix1 = ix.wrapping_add(1);
    let iy1 = iy.wrapping_add(1);
    let c00 = unit_f64(hash2(seed, ix, iy));
    let c10 = unit_f64(hash2(seed, ix1, iy));
    let c01 = unit_f64(hash2(seed, ix, iy1));
    let c11 = unit_f64(hash2(seed, ix1, iy1));
    let a = lerp(c00, c10, tx);
    let b = lerp(c01, c11, tx);
    lerp(a, b, ty)
}

/// 3D value noise in [0, 1). Inputs must be finite with `|coord| < 2^52`.
#[must_use]
pub fn value_noise_3d(seed: u64, x: f64, y: f64, z: f64) -> f64 {
    let fx = x.floor();
    let fy = y.floor();
    let fz = z.floor();
    let ix = fx as i64;
    let iy = fy as i64;
    let iz = fz as i64;
    let tx = fade(x - fx);
    let ty = fade(y - fy);
    let tz = fade(z - fz);
    let ix1 = ix.wrapping_add(1);
    let iy1 = iy.wrapping_add(1);
    let iz1 = iz.wrapping_add(1);
    let c000 = unit_f64(hash3(seed, ix, iy, iz));
    let c100 = unit_f64(hash3(seed, ix1, iy, iz));
    let c010 = unit_f64(hash3(seed, ix, iy1, iz));
    let c110 = unit_f64(hash3(seed, ix1, iy1, iz));
    let c001 = unit_f64(hash3(seed, ix, iy, iz1));
    let c101 = unit_f64(hash3(seed, ix1, iy, iz1));
    let c011 = unit_f64(hash3(seed, ix, iy1, iz1));
    let c111 = unit_f64(hash3(seed, ix1, iy1, iz1));
    let x00 = lerp(c000, c100, tx);
    let x10 = lerp(c010, c110, tx);
    let x01 = lerp(c001, c101, tx);
    let x11 = lerp(c011, c111, tx);
    let y0 = lerp(x00, x10, ty);
    let y1 = lerp(x01, x11, ty);
    lerp(y0, y1, tz)
}

/// Fractal sum of `octaves` value-noise octaves, normalised to [0, 1).
/// Octave `o` uses seed `splitmix64(seed ^ o)`; frequency doubles, amplitude halves.
#[must_use]
pub fn fbm_3d(seed: u64, x: f64, y: f64, z: f64, octaves: u32) -> f64 {
    if octaves == 0 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut norm = 0.0;
    let mut freq = 1.0;
    let mut amp = 1.0;
    for o in 0..octaves {
        sum += amp
            * value_noise_3d(
                splitmix64(seed ^ u64::from(o)),
                x * freq,
                y * freq,
                z * freq,
            );
        norm += amp;
        freq *= 2.0;
        amp *= 0.5;
    }
    sum / norm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_lattice_points_equal_hash() {
        let s = 12345;
        assert_eq!(
            value_noise_3d(s, 3.0, 4.0, 5.0),
            unit_f64(hash3(s, 3, 4, 5))
        );
        assert_eq!(value_noise_2d(s, -2.0, 7.0), unit_f64(hash2(s, -2, 7)));
    }

    #[test]
    fn noise_range() {
        for i in 0..24 {
            for j in 0..24 {
                for k in 0..24 {
                    let (x, y, z) = (
                        f64::from(i) * 0.37 - 20.0,
                        f64::from(j) * 0.37 - 20.0,
                        f64::from(k) * 0.37 - 20.0,
                    );
                    let a = value_noise_3d(9, x, y, z);
                    let b = fbm_3d(9, x, y, z, 5);
                    let c = value_noise_2d(9, x, y);
                    for v in [a, b, c] {
                        assert!(v.is_finite() && (0.0..1.0).contains(&v));
                    }
                }
            }
        }
    }

    #[test]
    fn noise_deterministic_and_seed_sensitive() {
        let mut differ = false;
        for i in 0..16 {
            let x = f64::from(i) * 0.731 + 0.1;
            let a = value_noise_3d(1, x, 1.3, 2.7);
            assert_eq!(a.to_bits(), value_noise_3d(1, x, 1.3, 2.7).to_bits());
            if a.to_bits() != value_noise_3d(2, x, 1.3, 2.7).to_bits() {
                differ = true;
            }
        }
        assert!(differ);
    }

    #[test]
    fn noise_fbm_zero_octaves() {
        assert_eq!(fbm_3d(5, 1.5, 2.5, 3.5, 0), 0.0);
    }

    #[test]
    fn noise_fbm_one_octave_equals_value_noise() {
        let (s, x, y, z) = (77, 1.25, -3.5, 9.75);
        assert_eq!(
            fbm_3d(s, x, y, z, 1),
            value_noise_3d(splitmix64(s ^ u64::from(0u32)), x, y, z)
        );
    }

    #[test]
    fn noise_golden_bits() {
        let samples: [(u64, f64, f64, f64, u64); 4] = [
            (0, 0.5, 0.5, 0.5, 0x3fe4_7fef_be68_1fa4),
            (1, -3.25, 7.75, 0.125, 0x3fdb_a71a_dda9_605d),
            (0xDEAD_BEEF, 100.3, -200.7, 55.5, 0x3fe2_d38a_17d2_6cb2),
            (42, 0.1, 0.2, 0.3, 0x3fe5_2aea_314e_5916),
        ];
        for (s, x, y, z, bits) in samples {
            let got = fbm_3d(s, x, y, z, 4).to_bits();
            assert_eq!(got, bits, "seed {s} -> {got:#018x}");
        }
    }
}
