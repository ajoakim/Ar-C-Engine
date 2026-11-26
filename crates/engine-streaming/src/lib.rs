//! Generic asynchronous streaming: keyed requests carrying priority, budget
//! class and a cancellation token, counted per-frame caps, and coarse-ancestor
//! fallback while refinements load (ADR-0008).
//!
//! Tier 3 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`. Contains no planetary concepts:
//! keys are opaque and only expose a stable asset id and an optional parent.

mod config;
mod error;
mod key;
mod manager;
mod pins;
mod request;
mod source;

pub use config::{FrameReport, Resolved, StreamConfig};
pub use engine_assets::{
    AssetClass, AssetError, AssetId, ClassBudget, MemoryPool, ResidencyManager, ResidencyState,
};
pub use engine_jobs::{CancelToken, JobSystem, Priority};
pub use error::StreamError;
pub use key::StreamKey;
pub use manager::StreamManager;
pub use request::StreamRequest;
pub use source::{StreamSource, Uploader};
