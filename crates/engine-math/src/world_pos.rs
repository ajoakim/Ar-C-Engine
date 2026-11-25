//! Opaque world position: f64 frame-local coordinates tagged with their frame.

use glam::DVec3;

use crate::error::MathError;
use crate::frame::FrameId;

/// A position in meters, local to a coordinate frame.
///
/// Fields are private on purpose: there is no `Add`/`Sub` between positions and no
/// `From<DVec3>`, so positions from different frames can never be mixed implicitly.
/// Use `FrameRegistry::convert`/`delta` to relate positions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldPos {
    frame: FrameId,
    local: DVec3,
}

impl WorldPos {
    /// Creates a position; `NonFinite` if any component is NaN or infinite.
    pub fn new(frame: FrameId, local: DVec3) -> Result<Self, MathError> {
        if !local.is_finite() {
            return Err(MathError::NonFinite);
        }
        Ok(Self { frame, local })
    }

    /// Creates a position from components.
    pub fn from_xyz(frame: FrameId, x: f64, y: f64, z: f64) -> Result<Self, MathError> {
        Self::new(frame, DVec3::new(x, y, z))
    }

    /// The frame this position is expressed in.
    pub fn frame(&self) -> FrameId {
        self.frame
    }

    /// Read-only frame-local coordinates in meters.
    pub fn local(&self) -> DVec3 {
        self.local
    }

    /// Returns this position moved by `offset` (expressed in the same frame).
    pub fn translated(&self, offset: DVec3) -> Result<Self, MathError> {
        Self::new(self.frame, self.local + offset)
    }
}
