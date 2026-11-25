//! Prioritized, cancellable job scheduling.
//!
//! Tier 1 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

mod cancel;
mod error;
mod priority;
mod scheduler;

pub use cancel::CancelToken;
pub use error::JobError;
pub use priority::Priority;
pub use scheduler::{JobHandle, JobStatus, JobSystem, JobSystemBuilder};
