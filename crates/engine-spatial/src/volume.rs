//! f64 query volumes expressed in the index frame.
//!
//! Camera convention: right-handed, Y up, -Z forward (ADR-0006).

use crate::SpatialError;
use engine_math::{DQuat, DVec3, FrameId, WorldPos};

fn finite3(v: DVec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

fn check_frame(index_frame: FrameId, pos: &WorldPos) -> Result<DVec3, SpatialError> {
    if pos.frame() != index_frame {
        return Err(SpatialError::FrameMismatch {
            expected: index_frame,
            found: pos.frame(),
        });
    }
    Ok(pos.local())
}

/// Axis-aligned bounding box (inclusive).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    min: DVec3,
    max: DVec3,
}

impl Aabb {
    /// Creates a box from corners; requires finite values and `min <= max`.
    pub fn new(min: DVec3, max: DVec3) -> Result<Self, SpatialError> {
        if !finite3(min) || !finite3(max) {
            return Err(SpatialError::NonFinite);
        }
        if min.x > max.x || min.y > max.y || min.z > max.z {
            return Err(SpatialError::InvalidVolume("aabb min exceeds max"));
        }
        Ok(Self { min, max })
    }

    /// Creates a box from a center and non-negative half extents.
    pub fn from_center_half_extents(center: DVec3, half: DVec3) -> Result<Self, SpatialError> {
        if !finite3(center) || !finite3(half) {
            return Err(SpatialError::NonFinite);
        }
        if half.x < 0.0 || half.y < 0.0 || half.z < 0.0 {
            return Err(SpatialError::InvalidVolume("negative half extent"));
        }
        Self::new(center - half, center + half)
    }

    /// Creates a box centered on a `WorldPos`, which must be in `index_frame`.
    pub fn from_world_pos(
        index_frame: FrameId,
        center: WorldPos,
        half: DVec3,
    ) -> Result<Self, SpatialError> {
        let c = check_frame(index_frame, &center)?;
        Self::from_center_half_extents(c, half)
    }

    /// Minimum corner.
    pub fn min(&self) -> DVec3 {
        self.min
    }
    /// Maximum corner.
    pub fn max(&self) -> DVec3 {
        self.max
    }
    /// Center point.
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }
    /// Half extents.
    pub fn half_extents(&self) -> DVec3 {
        (self.max - self.min) * 0.5
    }
    /// Inclusive overlap test (touching boxes intersect).
    pub fn intersects(&self, o: &Aabb) -> bool {
        self.min.x <= o.max.x
            && self.max.x >= o.min.x
            && self.min.y <= o.max.y
            && self.max.y >= o.min.y
            && self.min.z <= o.max.z
            && self.max.z >= o.min.z
    }
    /// Inclusive point containment.
    pub fn contains_point(&self, p: DVec3) -> bool {
        p.cmpge(self.min).all() && p.cmple(self.max).all()
    }
}

/// Sphere volume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    center: DVec3,
    radius: f64,
}

impl Sphere {
    /// Creates a sphere; radius must be finite and non-negative.
    pub fn new(center: DVec3, radius: f64) -> Result<Self, SpatialError> {
        if !finite3(center) || !radius.is_finite() {
            return Err(SpatialError::NonFinite);
        }
        if radius < 0.0 {
            return Err(SpatialError::InvalidVolume("negative radius"));
        }
        Ok(Self { center, radius })
    }

    /// Creates a sphere centered on a `WorldPos`, which must be in `index_frame`.
    pub fn from_world_pos(
        index_frame: FrameId,
        center: WorldPos,
        radius: f64,
    ) -> Result<Self, SpatialError> {
        let c = check_frame(index_frame, &center)?;
        Self::new(c, radius)
    }

    /// Center.
    pub fn center(&self) -> DVec3 {
        self.center
    }
    /// Radius.
    pub fn radius(&self) -> f64 {
        self.radius
    }
    /// Tight axis-aligned bounds.
    pub fn bounds(&self) -> Aabb {
        let r = DVec3::splat(self.radius);
        Aabb {
            min: self.center - r,
            max: self.center + r,
        }
    }
    /// Closest-point test: `dist^2 <= r^2`.
    pub fn intersects_aabb(&self, b: &Aabb) -> bool {
        let closest = self.center.clamp(b.min, b.max);
        closest.distance_squared(self.center) <= self.radius * self.radius
    }
}

/// Plane; a point is inside when `normal.dot(p) + d >= 0`. Normal is unit length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    /// Unit inward normal.
    pub normal: DVec3,
    /// Plane offset.
    pub d: f64,
}

impl Plane {
    fn from_points(a: DVec3, b: DVec3, c: DVec3, inside: DVec3) -> Self {
        let mut n = (b - a).cross(c - a).normalize();
        if n.dot(inside - a) < 0.0 {
            n = -n;
        }
        Self {
            normal: n,
            d: -n.dot(a),
        }
    }
}

/// Perspective view frustum with a finite far plane, in the index frame.
///
/// The render camera uses an infinite far plane; spatial culling needs a
/// finite far distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frustum {
    planes: [Plane; 6],
    corners: [DVec3; 8],
    bounds: Aabb,
}

impl Frustum {
    /// Builds a frustum from camera pose and projection parameters.
    pub fn from_perspective(
        position: DVec3,
        rotation: DQuat,
        fov_y: f64,
        aspect: f64,
        near: f64,
        far: f64,
    ) -> Result<Self, SpatialError> {
        let q = rotation;
        let params = [fov_y, aspect, near, far, q.x, q.y, q.z, q.w];
        if !finite3(position) || params.iter().any(|v| !v.is_finite()) {
            return Err(SpatialError::NonFinite);
        }
        if (q.length() - 1.0).abs() > 1e-6 {
            return Err(SpatialError::InvalidVolume("rotation not normalized"));
        }
        if fov_y <= 0.0 || fov_y >= std::f64::consts::PI {
            return Err(SpatialError::InvalidVolume("fov out of range"));
        }
        if aspect <= 0.0 {
            return Err(SpatialError::InvalidVolume("aspect must be positive"));
        }
        if near <= 0.0 || far <= near {
            return Err(SpatialError::InvalidVolume("require 0 < near < far"));
        }
        let fwd = q * DVec3::NEG_Z;
        let up = q * DVec3::Y;
        let right = q * DVec3::X;
        let t = (fov_y * 0.5).tan();
        let ring = |dist: f64| {
            let h = dist * t;
            let w = h * aspect;
            let c = position + fwd * dist;
            [
                c - right * w - up * h,
                c + right * w - up * h,
                c + right * w + up * h,
                c - right * w + up * h,
            ]
        };
        let n = ring(near);
        let f = ring(far);
        let corners = [n[0], n[1], n[2], n[3], f[0], f[1], f[2], f[3]];
        let centroid = corners.iter().fold(DVec3::ZERO, |a, c| a + *c) / 8.0;
        let planes = [
            Plane::from_points(n[0], n[1], n[2], centroid),
            Plane::from_points(f[0], f[1], f[2], centroid),
            Plane::from_points(n[0], n[3], f[3], centroid),
            Plane::from_points(n[1], n[2], f[2], centroid),
            Plane::from_points(n[3], n[2], f[2], centroid),
            Plane::from_points(n[0], n[1], f[1], centroid),
        ];
        let (mut lo, mut hi) = (corners[0], corners[0]);
        for c in &corners {
            lo = lo.min(*c);
            hi = hi.max(*c);
        }
        Ok(Self {
            planes,
            corners,
            bounds: Aabb { min: lo, max: hi },
        })
    }

    /// Builds a frustum from a camera `WorldPos`, which must be in `index_frame`.
    pub fn from_world_pos(
        index_frame: FrameId,
        camera: WorldPos,
        rotation: DQuat,
        fov_y: f64,
        aspect: f64,
        near: f64,
        far: f64,
    ) -> Result<Self, SpatialError> {
        let p = check_frame(index_frame, &camera)?;
        Self::from_perspective(p, rotation, fov_y, aspect, near, far)
    }

    /// The six inward planes (near, far, left, right, top, bottom).
    pub fn planes(&self) -> &[Plane; 6] {
        &self.planes
    }
    /// The eight corner points.
    pub fn corners(&self) -> &[DVec3; 8] {
        &self.corners
    }
    /// Axis-aligned bounds of the frustum corners.
    pub fn bounds(&self) -> Aabb {
        self.bounds
    }

    /// Conservative box test: may report true near frustum corners, never
    /// false for an intersecting box. Every match also intersects `bounds()`,
    /// which grid backends rely on.
    pub fn intersects_aabb(&self, b: &Aabb) -> bool {
        if !self.bounds.intersects(b) {
            return false;
        }
        for p in &self.planes {
            let pv = DVec3::new(
                if p.normal.x >= 0.0 { b.max.x } else { b.min.x },
                if p.normal.y >= 0.0 { b.max.y } else { b.min.y },
                if p.normal.z >= 0.0 { b.max.z } else { b.min.z },
            );
            if p.normal.dot(pv) + p.d < 0.0 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_math::FrameRegistry;
    use std::f64::consts::PI;

    fn bx(c: DVec3, h: f64) -> Aabb {
        Aabb::from_center_half_extents(c, DVec3::splat(h)).unwrap()
    }
    fn cam(pos: DVec3, rot: DQuat) -> Frustum {
        Frustum::from_perspective(pos, rot, PI / 3.0, 1.0, 0.1, 100.0).unwrap()
    }

    #[test]
    fn volume_aabb_rejects_invalid() {
        assert!(matches!(
            Aabb::new(DVec3::ONE, DVec3::ZERO),
            Err(SpatialError::InvalidVolume(_))
        ));
        assert_eq!(
            Aabb::new(DVec3::splat(f64::NAN), DVec3::ZERO),
            Err(SpatialError::NonFinite)
        );
        assert_eq!(
            Aabb::new(DVec3::ZERO, DVec3::splat(f64::INFINITY)),
            Err(SpatialError::NonFinite)
        );
    }

    #[test]
    fn volume_aabb_intersects_inclusive() {
        let a = bx(DVec3::ZERO, 1.0);
        assert!(a.intersects(&bx(DVec3::new(2.0, 0.0, 0.0), 1.0)));
        assert!(!a.intersects(&bx(DVec3::new(2.5, 0.0, 0.0), 1.0)));
    }

    #[test]
    fn volume_sphere_intersects_aabb() {
        let b = bx(DVec3::ZERO, 1.0);
        assert!(
            Sphere::new(DVec3::new(2.0, 0.0, 0.0), 1.0)
                .unwrap()
                .intersects_aabb(&b)
        );
        // Corner at (1,1,1); center at (2,2,2) distance sqrt(3) ~ 1.73 > 1.5.
        assert!(
            !Sphere::new(DVec3::splat(2.0), 1.5)
                .unwrap()
                .intersects_aabb(&b)
        );
    }

    #[test]
    fn volume_sphere_rejects_negative_radius() {
        assert!(matches!(
            Sphere::new(DVec3::ZERO, -1.0),
            Err(SpatialError::InvalidVolume(_))
        ));
    }

    #[test]
    fn volume_frustum_default_looks_down_neg_z() {
        let f = cam(DVec3::ZERO, DQuat::IDENTITY);
        assert!(f.intersects_aabb(&bx(DVec3::new(0.0, 0.0, -10.0), 0.5)));
        assert!(!f.intersects_aabb(&bx(DVec3::new(0.0, 0.0, 10.0), 0.5)));
        assert!(!f.intersects_aabb(&bx(DVec3::new(0.0, 0.0, -200.0), 0.5)));
        assert!(!f.intersects_aabb(&bx(DVec3::new(100.0, 0.0, -10.0), 0.5)));
    }

    #[test]
    fn volume_frustum_rotated() {
        let f = cam(DVec3::ZERO, DQuat::from_rotation_y(PI));
        assert!(f.intersects_aabb(&bx(DVec3::new(0.0, 0.0, 10.0), 0.5)));
        assert!(!f.intersects_aabb(&bx(DVec3::new(0.0, 0.0, -10.0), 0.5)));
    }

    #[test]
    fn volume_frustum_at_planet_distance() {
        let f = cam(DVec3::new(6.4e6, 0.0, 0.0), DQuat::IDENTITY);
        assert!(f.intersects_aabb(&bx(DVec3::new(6.4e6, 0.0, -5.0), 0.5)));
        assert!(!f.intersects_aabb(&bx(DVec3::new(6.4e6, 0.0, 5.0), 0.5)));
    }

    #[test]
    fn volume_frustum_rejects_bad_params() {
        let p = DVec3::ZERO;
        let r = DQuat::IDENTITY;
        let bad = [
            (0.0, 1.0, 0.1, 100.0),
            (PI, 1.0, 0.1, 100.0),
            (1.0, 0.0, 0.1, 100.0),
            (1.0, 1.0, 0.0, 100.0),
            (1.0, 1.0, 1.0, 1.0),
            (f64::NAN, 1.0, 0.1, 100.0),
            (1.0, 1.0, 0.1, f64::INFINITY),
        ];
        for (fov, asp, n, fa) in bad {
            assert!(Frustum::from_perspective(p, r, fov, asp, n, fa).is_err());
        }
    }

    #[test]
    fn volume_from_world_pos_frame_mismatch() {
        let mut reg = FrameRegistry::new();
        let other = reg
            .add_frame("o", reg.root(), engine_math::DAffine3::IDENTITY)
            .unwrap();
        let pos = WorldPos::new(other, DVec3::ZERO).unwrap();
        let expected = reg.root();
        let want = SpatialError::FrameMismatch {
            expected,
            found: other,
        };
        assert_eq!(
            Aabb::from_world_pos(expected, pos, DVec3::ONE),
            Err(want.clone())
        );
        assert_eq!(
            Sphere::from_world_pos(expected, pos, 1.0),
            Err(want.clone())
        );
        assert_eq!(
            Frustum::from_world_pos(expected, pos, DQuat::IDENTITY, 1.0, 1.0, 0.1, 10.0),
            Err(want)
        );
        let ok = WorldPos::new(expected, DVec3::ZERO).unwrap();
        assert!(Aabb::from_world_pos(expected, ok, DVec3::ONE).is_ok());
    }

    #[test]
    fn volume_frustum_bounds_contain_corners() {
        let f = cam(DVec3::new(3.0, 4.0, 5.0), DQuat::from_rotation_x(0.3));
        for c in f.corners() {
            assert!(f.bounds().contains_point(*c));
        }
    }

    #[test]
    fn volume_frustum_rejects_box_outside_bounds() {
        let f = cam(DVec3::ZERO, DQuat::IDENTITY);
        let y0 = 100.0 * (PI / 6.0).tan() + 1.0;
        let b = Aabb::new(DVec3::new(-1e4, y0, -200.0), DVec3::new(1e4, 1e4, -0.05)).unwrap();
        assert!(!f.intersects_aabb(&b));
    }
}
