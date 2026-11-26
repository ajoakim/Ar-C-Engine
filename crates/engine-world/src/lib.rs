//! Entities, transforms and coordinate spaces.
//!
//! Tier 2 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

mod entity;
mod error;
mod transform;
mod world;

pub use entity::EntityId;
pub use error::WorldError;
pub use transform::{Transform, WorldTransform};
pub use world::World;
