//! Asset error type.

use crate::id::AssetId;
use crate::manager::{AssetClass, MemoryPool};
use crate::residency::ResidencyState;

/// Errors produced by asset IDs and registries.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AssetError {
    /// The key was empty after normalization.
    #[error("asset key is empty after normalization")]
    EmptyKey,
    /// The handle is stale or was never issued by this registry.
    #[error("stale or unknown handle")]
    StaleHandle,
    /// No asset is registered under this ID.
    #[error("unknown asset {0}")]
    UnknownAsset(AssetId),
    /// Illegal residency transition.
    #[error("illegal residency transition {from:?} -> {to:?}")]
    IllegalTransition {
        /// Source state.
        from: ResidencyState,
        /// Requested target state.
        to: ResidencyState,
    },
    /// Asset already tracked.
    #[error("asset {0} is already tracked")]
    AlreadyTracked(AssetId),
    /// Class has no budget.
    #[error("asset class {0:?} has no budget")]
    UnknownClass(AssetClass),
    /// Budget exceeded.
    #[error("{pool:?} budget exceeded for {class:?}: need {needed} bytes, {available} available")]
    OverBudget {
        /// Class.
        class: AssetClass,
        /// Pool.
        pool: MemoryPool,
        /// Bytes needed.
        needed: u64,
        /// Bytes available.
        available: u64,
    },
    /// Per-frame upload cap exceeded.
    #[error("per-frame upload cap exceeded: need {needed} bytes, {remaining} remaining")]
    UploadCapExceeded {
        /// Bytes needed.
        needed: u64,
        /// Bytes remaining this frame.
        remaining: u64,
    },
    /// Completion for a superseded request.
    #[error("stale request generation for asset {0}")]
    StaleRequest(AssetId),
    /// Byte counter overflow.
    #[error("byte accounting overflow")]
    AccountingOverflow,
}
