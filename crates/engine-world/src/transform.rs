//! Local transform data type.

use engine_math::{DVec3, FrameId, WorldPos};
use glam::{Quat, Vec3};

use crate::WorldError;

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

/// World-space transform: frame-aware f64 position plus accumulated rotation/scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldTransform {
    /// World position (frame + f64 local).
    pub position: WorldPos,
    /// Accumulated rotation.
    pub rotation: Quat,
    /// Accumulated scale.
    pub scale: Vec3,
}

impl WorldTransform {
    /// World transform of a root entity in `frame`.
    ///
    /// # Errors
    /// `Math(NonFinite)` for non-finite translation.
    pub fn from_root(frame: FrameId, local: &Transform) -> Result<Self, WorldError> {
        Ok(Self {
            position: WorldPos::new(frame, local.translation)?,
            rotation: local.rotation,
            scale: local.scale,
        })
    }

    /// Compose with a child's local transform. Translation math is entirely f64.
    ///
    /// TRS only: non-uniform scale combined with rotated children cannot express shear.
    ///
    /// # Errors
    /// `Math(NonFinite)` if the result overflows.
    pub fn compose(&self, child: &Transform) -> Result<Self, WorldError> {
        let offset = self.rotation.as_dquat() * (self.scale.as_dvec3() * child.translation);
        let position = self.position.translated(offset)?;
        Ok(Self {
            position,
            rotation: (self.rotation * child.rotation).normalize(),
            scale: self.scale * child.scale,
        })
    }
}
