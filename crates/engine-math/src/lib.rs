//! Math types, coordinate conventions and the precision model (f64 world positions, camera-relative f32).
//!
//! Tier 0 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

mod error;
mod frame;
mod rebase;
mod world_pos;

pub use error::MathError;
pub use frame::{FrameId, FrameRegistry};
pub use glam::{DAffine3, DQuat, DVec3, Vec3};
pub use rebase::{PosId, PositionStore, rebase};
pub use world_pos::WorldPos;
