//! Resource IDs, typed handles and residency state.
//!
//! Tier 2 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

mod error;
mod handle;
mod id;
mod manager;
mod registry;
mod residency;

pub use manager::{
    AssetClass, ClassBudget, ClassStats, MemoryPool, ResidencyManager, ResidencyStats,
};
pub use residency::ResidencyState;

pub use error::AssetError;
pub use handle::Handle;
pub use id::{ASSET_NAMESPACE, AssetId, normalize_key};
pub use registry::AssetRegistry;
