//! Error type for the math and precision model.

use crate::frame::FrameId;
use crate::rebase::PosId;

/// Errors returned by fallible `engine-math` operations. Invalid input never panics.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MathError {
    /// A coordinate or transform component was NaN or infinite.
    #[error("non-finite coordinate or transform")]
    NonFinite,
    /// The frame id is not registered in this registry.
    #[error("unknown frame {0:?}")]
    UnknownFrame(FrameId),
    /// The frame transform is singular.
    #[error("frame transform is not invertible for {0:?}")]
    NonInvertible(FrameId),
    /// The root frame has no parent transform.
    #[error("the root frame has no parent transform and cannot be rebased")]
    RootFrame,
    /// The position id is not (or no longer) in the store.
    #[error("unknown position {0:?}")]
    UnknownPosition(PosId),
}
