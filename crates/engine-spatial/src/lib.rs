//! Pluggable spatial query interface and default backend.
//!
//! Tier 3 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

mod error;
mod index;
mod linear;
mod volume;

pub use error::SpatialError;
pub use index::SpatialIndex;
pub use linear::LinearIndex;
pub use volume::{Aabb, Frustum, Plane, Sphere};
