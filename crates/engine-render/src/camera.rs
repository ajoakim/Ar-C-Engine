//! Camera with a rotation-only view and a reverse-Z infinite projection.

use engine_math::WorldPos;
use glam::{Mat4, Quat};

use crate::error::RenderError;

/// A camera positioned in f64 world space.
///
/// The view matrix is rotation-only because rendering happens in camera-relative space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// World position (f64).
    pub position: WorldPos,
    /// Orientation (right-handed, Y up, -Z forward).
    pub orientation: Quat,
    /// Vertical field of view in radians.
    pub fov_y_radians: f32,
    /// Near plane distance.
    pub z_near: f32,
}

impl Camera {
    /// Default vertical field of view (60 degrees).
    pub const DEFAULT_FOV_Y: f32 = std::f32::consts::FRAC_PI_3;
    /// Default near plane.
    pub const DEFAULT_Z_NEAR: f32 = 0.1;

    /// Camera at `position` with identity orientation and default lens.
    pub fn new(position: WorldPos) -> Self {
        Self {
            position,
            orientation: Quat::IDENTITY,
            fov_y_radians: Self::DEFAULT_FOV_Y,
            z_near: Self::DEFAULT_Z_NEAR,
        }
    }

    /// Rotation-only view matrix (camera sits at the origin of camera-relative space).
    pub fn view(&self) -> Mat4 {
        Mat4::from_quat(self.orientation.normalize()).inverse()
    }

    /// Reverse-Z infinite-far projection (ADR-0001).
    pub fn projection(&self, aspect: f32) -> Result<Mat4, RenderError> {
        if !(aspect.is_finite() && aspect > 0.0) {
            return Err(RenderError::InvalidCamera("aspect must be finite and > 0"));
        }
        if !(self.fov_y_radians.is_finite()
            && self.fov_y_radians > 0.0
            && self.fov_y_radians < std::f32::consts::PI)
        {
            return Err(RenderError::InvalidCamera("fov_y out of range"));
        }
        if !(self.z_near.is_finite() && self.z_near > 0.0) {
            return Err(RenderError::InvalidCamera("z_near must be > 0"));
        }
        let len = self.orientation.length();
        if !(self.orientation.is_finite() && len > 0.0 && len.is_finite()) {
            return Err(RenderError::InvalidCamera("orientation invalid"));
        }
        // glam 0.34 equivalent of the old `Mat4::perspective_infinite_reverse_rh`:
        // right-handed, Y-up, NDC depth [0,1], reversed-Z.
        Ok(
            glam::camera::rh::proj::directx::perspective_infinite_reverse(
                self.fov_y_radians,
                aspect,
                self.z_near,
            ),
        )
    }
}
