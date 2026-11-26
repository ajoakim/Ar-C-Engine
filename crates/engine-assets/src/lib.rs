//! Resource IDs, typed handles and residency state.
//!
//! Tier 2 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

mod error;
mod id;

pub use error::AssetError;
pub use id::{normalize_key, AssetId, ASSET_NAMESPACE};
