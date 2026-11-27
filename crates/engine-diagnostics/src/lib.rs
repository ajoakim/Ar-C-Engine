//! Logging, tracing spans, metrics and frame timing.
//!
//! Tier 0 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

mod error;
mod frame_stats;
mod init;
mod layer;
mod snapshot;

pub use error::DiagnosticsError;
pub use frame_stats::{FrameClock, FrameStats};
pub use init::{DiagnosticsConfig, init};
pub use layer::{FrameTimingHandle, FrameTimingLayer, SpanTiming, frame_timing_subscriber};
pub use snapshot::{Budget, DiagnosticsSnapshot, Histogram};
pub use tracing;
