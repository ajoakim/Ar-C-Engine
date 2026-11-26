//! Streaming errors.

use engine_assets::{AssetError, AssetId};

/// Errors returned by the streaming pipeline.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StreamError {
    /// The pending queue is at capacity; the request was rejected.
    #[error("pending queue full ({max} requests)")]
    QueueFull {
        /// Configured maximum.
        max: usize,
    },
    /// The key is already pending, in flight or resident.
    #[error("key {0} is already pending, in flight or resident")]
    Duplicate(AssetId),
    /// A configuration field is invalid.
    #[error("invalid stream config: {0}")]
    InvalidConfig(&'static str),
    /// Underlying residency error.
    #[error(transparent)]
    Asset(#[from] AssetError),
}
