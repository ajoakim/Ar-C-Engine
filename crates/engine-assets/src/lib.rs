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

pub use manager::{AssetClass, ClassBudget, MemoryPool, ResidencyManager};
pub use residency::ResidencyState;

pub use error::AssetError;
pub use handle::Handle;
pub use registry::AssetRegistry;
pub use id::{normalize_key, AssetId, ASSET_NAMESPACE};
