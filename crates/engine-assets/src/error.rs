//! Asset error type.

use crate::id::AssetId;

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
}
