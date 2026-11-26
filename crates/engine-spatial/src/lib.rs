//! Pluggable spatial query interface and default backend.
//!
//! Tier 3 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

mod error;
mod volume;

pub use error::SpatialError;
pub use volume::{Aabb, Frustum, Plane, Sphere};
