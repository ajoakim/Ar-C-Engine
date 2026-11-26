//! Opaque streaming key contract.

use engine_assets::AssetId;

/// Opaque streaming key. Implementations must be cheap to clone and map injectively to an [`AssetId`].
pub trait StreamKey: Clone + Ord + std::fmt::Debug + Send + Sync + 'static {
    /// Stable residency id of this key.
    fn asset_id(&self) -> AssetId;
    /// Coarser key used as fallback while this one loads; `None` at a root.
    fn parent(&self) -> Option<Self>;
}
