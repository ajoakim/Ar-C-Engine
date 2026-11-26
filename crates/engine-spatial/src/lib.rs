//! Pluggable spatial query interface and default backend.
//!
//! [`HashGridIndex`] is the default backend; [`LinearIndex`] is the brute-force oracle.
//!
//! Tier 3 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

mod error;
mod hash_grid;
mod index;
mod linear;
mod volume;

pub use error::SpatialError;
pub use hash_grid::{HashGridConfig, HashGridIndex};
pub use index::SpatialIndex;
pub use linear::LinearIndex;
pub use volume::{Aabb, Frustum, Plane, Sphere};
