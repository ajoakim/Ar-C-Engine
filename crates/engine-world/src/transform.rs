//! Local transform data type.

use engine_math::DVec3;
use glam::{Quat, Vec3};

/// Local translation/rotation/scale, relative to the parent entity (or, for
/// roots, relative to the root's `FrameId`).
///
/// TRS only, no shear. Translation is f64; rotation and scale are f32.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Local translation.
    pub translation: DVec3,
    /// Local rotation.
    pub rotation: Quat,
    /// Local scale.
    pub scale: Vec3,
}

impl Transform {
    /// Identity transform.
    pub const IDENTITY: Self = Self {
        translation: DVec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// Identity rotation and scale with the given translation.
    #[must_use]
    pub fn from_translation(t: DVec3) -> Self {
        Self {
            translation: t,
            ..Self::IDENTITY
        }
    }

    /// Full constructor.
    #[must_use]
    pub fn new(translation: DVec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }

    /// True when every component is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.translation.is_finite() && self.rotation.is_finite() && self.scale.is_finite()
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}
