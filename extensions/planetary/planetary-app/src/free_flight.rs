//! Free-flight 6DOF camera controller (pure f64 quaternion maths, no GPU or window types).
//!
//! The controller lives in the planetary extension; the engine core only knows key codes.

use engine_sdk::{DQuat, DVec3};

/// Default roll rate in radians per second while a roll key is held.
pub const DEFAULT_ROLL_RATE: f64 = 1.2;

/// Applies body-axis increments to `q` and returns a normalised quaternion.
///
/// Yaw is about the local Y (camera up) axis, pitch about local X (camera right) and roll about
/// local Z (the view axis; a positive angle is counterclockwise as seen by the pilot looking
/// along -Z). The product is always renormalised, even for zero increments, so a non-unit input
/// is repaired and rounding drift cannot accumulate.
pub fn rotate_local(q: DQuat, pitch: f64, yaw: f64, roll: f64) -> DQuat {
    let dq = DQuat::from_axis_angle(DVec3::Y, yaw)
        * DQuat::from_axis_angle(DVec3::X, pitch)
        * DQuat::from_axis_angle(DVec3::Z, roll);
    (q * dq).normalize()
}

/// Levels the horizon: keeps the view direction, makes camera right perpendicular to
/// `planet_up` and camera up point toward it. Returns `q` unchanged when the view direction is
/// (nearly) parallel to `planet_up`.
pub fn level_orientation(q: DQuat, planet_up: DVec3) -> DQuat {
    let f = q * DVec3::NEG_Z;
    let side = f.cross(planet_up);
    if side.length() < 1e-9 {
        return q;
    }
    let r = side.normalize();
    let u = r.cross(f);
    // Roll about the (unchanged) view axis from the current camera up to the levelled up.
    let u0 = q * DVec3::Y;
    let angle = f.dot(u0.cross(u)).atan2(u0.dot(u));
    (DQuat::from_axis_angle(f, angle) * q).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DQuat {
        (DQuat::from_axis_angle(DVec3::Y, 0.7)
            * DQuat::from_axis_angle(DVec3::X, -0.4)
            * DQuat::from_axis_angle(DVec3::Z, 0.2))
        .normalize()
    }

    #[test]
    fn rotate_local_renormalises_non_unit_input() {
        let q = sample();
        let r = rotate_local(q * 1.001, 0.0, 0.0, 0.0);
        assert!((r.length() - 1.0).abs() < 1e-12);
        let r2 = rotate_local(q * 0.999, 0.01, -0.02, 0.03);
        assert!((r2.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn roll_keeps_forward_and_rotates_up_by_angle() {
        let q = sample();
        let a = 0.3;
        let r = rotate_local(q, 0.0, 0.0, a);
        let f0 = q * DVec3::NEG_Z;
        let f1 = r * DVec3::NEG_Z;
        assert!((f0 - f1).length() < 1e-12);
        let u0 = q * DVec3::Y;
        let u1 = r * DVec3::Y;
        assert!((u0.dot(u1).acos() - a).abs() < 1e-12);
        assert!(u0.cross(u1).dot(f0).abs() > 0.0);
    }

    #[test]
    fn roll_plus_minus_returns_to_start() {
        let q = sample();
        let back = rotate_local(rotate_local(q, 0.0, 0.0, 0.7), 0.0, 0.0, -0.7);
        assert!(q.dot(back).abs() > 1.0 - 1e-9);
    }

    #[test]
    fn level_orientation_levels_horizon_keeping_forward() {
        let q = sample();
        let up = DVec3::new(0.2, 0.9, -0.3).normalize();
        let l = level_orientation(q, up);
        assert!(((l * DVec3::NEG_Z) - (q * DVec3::NEG_Z)).length() < 1e-12);
        assert!((l * DVec3::X).dot(up).abs() < 1e-12);
        assert!((l * DVec3::Y).dot(up) > 0.0);
        assert!((l.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn level_orientation_parallel_forward_is_unchanged() {
        let q = DQuat::IDENTITY; // forward = -Z
        assert_eq!(level_orientation(q, DVec3::Z), q);
        assert_eq!(level_orientation(q, DVec3::NEG_Z), q);
    }
}
