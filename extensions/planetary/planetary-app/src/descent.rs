//! Scripted orbit-to-surface descent: a pure function of simulated time.

use engine_sdk::{DQuat, DVec3};

/// A deterministic descent path used by the example (`--descent`) and the descent test.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DescentScript {
    pub start_dir: DVec3,
    pub start_altitude: f64,
    pub end_altitude: f64,
    pub descent_seconds: f64,
    pub lateral_radians: f64,
    pub pitch_down_radians: f64,
}

impl DescentScript {
    /// Default demo path toward a cube corner region.
    #[must_use]
    pub fn orbit_to_surface(radius: f64) -> Self {
        Self {
            start_dir: DVec3::new(1.0, 0.8, 0.6).normalize(),
            start_altitude: radius,
            end_altitude: 3.0,
            descent_seconds: 8.0,
            lateral_radians: 0.3,
            pitch_down_radians: 0.5,
        }
    }

    /// Fast free-flight descent: from orbit to 10 m above the surface in a third of the time of
    /// [`DescentScript::orbit_to_surface`], with more lateral travel and a shallower pitch.
    /// Pure function of time like the other scripts.
    #[must_use]
    pub fn fast_free_flight(radius: f64) -> Self {
        let d = Self::orbit_to_surface(radius);
        Self {
            end_altitude: 10.0,
            descent_seconds: d.descent_seconds / 3.0,
            lateral_radians: 0.6,
            pitch_down_radians: 0.35,
            ..d
        }
    }

    /// Scripted ascent from the surface back to orbit.
    ///
    /// Starts at the direction where [`DescentScript::orbit_to_surface`] ends (its start
    /// direction rotated by `lateral_radians` about the same perpendicular axis, computed with
    /// the same quaternion formula as `raw_pose`) at altitude 3 m and rises geometrically to
    /// `radius`.
    #[must_use]
    pub fn surface_to_orbit(radius: f64) -> Self {
        let d = Self::orbit_to_surface(radius);
        let start = d.start_dir.normalize();
        let up_ref = if start.y.abs() > 0.99 {
            DVec3::Z
        } else {
            DVec3::Y
        };
        let axis = start.cross(up_ref).normalize();
        let end_dir = (DQuat::from_axis_angle(axis, d.lateral_radians) * start).normalize();
        Self {
            start_dir: end_dir,
            start_altitude: 3.0,
            end_altitude: radius,
            ..d
        }
    }

    /// Pose at simulated time `t` (clamped to `[0, descent_seconds]`).
    ///
    /// Altitude decays geometrically from start to end; the direction rotates by
    /// `lateral_radians * t / T` about a fixed perpendicular axis.
    #[must_use]
    pub fn pose_at(&self, t: f64, surface_radius_at: &dyn Fn(DVec3) -> f64) -> (DVec3, DQuat) {
        let p = self.raw_pose(t, surface_radius_at);
        let finite = p.0.x.is_finite()
            && p.0.y.is_finite()
            && p.0.z.is_finite()
            && p.1.x.is_finite()
            && p.1.y.is_finite()
            && p.1.z.is_finite()
            && p.1.w.is_finite();
        if finite {
            p
        } else {
            self.raw_pose(0.0, surface_radius_at)
        }
    }

    fn raw_pose(&self, t: f64, surface_radius_at: &dyn Fn(DVec3) -> f64) -> (DVec3, DQuat) {
        let total = self.descent_seconds.max(f64::MIN_POSITIVE);
        let t = if t.is_finite() {
            t.clamp(0.0, total)
        } else {
            0.0
        };
        let frac = t / total;
        let start = self.start_dir.normalize();
        let altitude = self.start_altitude * (self.end_altitude / self.start_altitude).powf(frac);

        let up_ref = if start.y.abs() > 0.99 {
            DVec3::Z
        } else {
            DVec3::Y
        };
        let axis = start.cross(up_ref).normalize();
        let dir = (DQuat::from_axis_angle(axis, self.lateral_radians * frac) * start).normalize();
        let pos = dir * (surface_radius_at(dir) + altitude);

        let tangent = axis.cross(dir).normalize();
        let p = self.pitch_down_radians;
        let forward = (tangent * p.cos() - dir * p.sin()).normalize();
        let q1 = DQuat::from_rotation_arc(DVec3::NEG_Z, forward);
        let want_up = (dir - forward * dir.dot(forward)).normalize();
        let up1 = q1 * DVec3::Y;
        let roll = forward.dot(up1.cross(want_up)).atan2(up1.dot(want_up));
        let orient = (DQuat::from_axis_angle(forward, roll) * q1).normalize();
        (pos, orient)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere(_: DVec3) -> f64 {
        1000.0
    }

    #[test]
    fn altitude_decays_and_is_pure() {
        let s = DescentScript::orbit_to_surface(1000.0);
        let (p0, _) = s.pose_at(0.0, &sphere);
        let (p1, _) = s.pose_at(4.0, &sphere);
        let (p2, _) = s.pose_at(100.0, &sphere);
        assert!((p0.length() - 2000.0).abs() < 1e-6);
        assert!(p1.length() < p0.length());
        assert!((p2.length() - 1003.0).abs() < 1e-6);
        assert_eq!(s.pose_at(4.0, &sphere), s.pose_at(4.0, &sphere));
    }

    #[test]
    fn fast_free_flight_is_faster_and_pure() {
        let o = DescentScript::orbit_to_surface(1000.0);
        let s = DescentScript::fast_free_flight(1000.0);
        assert!(s.descent_seconds <= o.descent_seconds / 3.0 + 1e-12);
        let (p0, _) = s.pose_at(0.0, &sphere);
        let (p2, _) = s.pose_at(100.0, &sphere);
        assert!((p0.length() - 2000.0).abs() < 1e-6);
        assert!((p2.length() - 1010.0).abs() < 1e-6);
        assert_eq!(s.pose_at(1.0, &sphere), s.pose_at(1.0, &sphere));
    }

    #[test]
    fn ascent_rises_and_is_pure() {
        let s = DescentScript::surface_to_orbit(1000.0);
        let (p0, _) = s.pose_at(0.0, &sphere);
        let (p1, _) = s.pose_at(4.0, &sphere);
        let (p2, _) = s.pose_at(100.0, &sphere);
        assert!((p0.length() - 1003.0).abs() < 1e-6);
        assert!(p1.length() > p0.length());
        assert!((p2.length() - 2000.0).abs() < 1e-6);
        assert_eq!(s.pose_at(4.0, &sphere), s.pose_at(4.0, &sphere));
        // Starts where the descent ends.
        let d = DescentScript::orbit_to_surface(1000.0);
        let (e, _) = d.pose_at(d.descent_seconds, &sphere);
        assert!((e - p0).length() < 1e-6);
    }
}
