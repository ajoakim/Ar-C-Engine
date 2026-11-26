//! Spatial error type.

use engine_math::FrameId;
use engine_world::EntityId;

/// Errors from spatial volumes and indices.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SpatialError {
    /// A coordinate or volume parameter was NaN or infinite.
    #[error("non-finite volume or coordinate")]
    NonFinite,
    /// A volume was malformed (e.g. min > max, negative radius).
    #[error("invalid volume: {0}")]
    InvalidVolume(&'static str),
    /// An index was configured with invalid parameters.
    #[error("invalid index configuration: {0}")]
    InvalidConfig(&'static str),
    /// A position was expressed in a frame other than the index frame.
    #[error("expected frame {expected:?}, found {found:?}")]
    FrameMismatch {
        /// The index frame.
        expected: FrameId,
        /// The frame of the supplied position.
        found: FrameId,
    },
    /// The entity is already present in the index.
    #[error("entity {0:?} is already in the index")]
    DuplicateEntity(EntityId),
    /// The entity is not present in the index.
    #[error("entity {0:?} is not in the index")]
    UnknownEntity(EntityId),
}
