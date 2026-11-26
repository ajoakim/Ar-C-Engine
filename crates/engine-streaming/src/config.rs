//! Streaming configuration and per-frame report.

use crate::error::StreamError;

/// Counted caps for the streaming pipeline (never time based).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamConfig {
    /// Maximum queued (not yet started) requests; beyond this requests are rejected.
    pub max_pending: usize,
    /// Maximum requests started per frame.
    pub max_starts_per_frame: u32,
    /// Maximum completions applied per frame.
    pub max_completions_per_frame: u32,
}

impl StreamConfig {
    /// Every field must be at least 1.
    pub fn validate(&self) -> Result<(), StreamError> {
        if self.max_pending < 1 {
            return Err(StreamError::InvalidConfig("max_pending must be >= 1"));
        }
        if self.max_starts_per_frame < 1 {
            return Err(StreamError::InvalidConfig(
                "max_starts_per_frame must be >= 1",
            ));
        }
        if self.max_completions_per_frame < 1 {
            return Err(StreamError::InvalidConfig(
                "max_completions_per_frame must be >= 1",
            ));
        }
        Ok(())
    }
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            max_pending: 256,
            max_starts_per_frame: 8,
            max_completions_per_frame: 8,
        }
    }
}

/// Counted per-frame work report (the frame budget is expressed in these units, never in time).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameReport {
    /// Frame index.
    pub frame: u64,
    /// Requests started.
    pub started: u32,
    /// Completions applied.
    pub completions_applied: u32,
    /// Uploads completed.
    pub uploads: u32,
    /// Bytes uploaded.
    pub uploaded_bytes: u64,
    /// Requests that failed.
    pub failed: u32,
    /// Requests cancelled.
    pub cancelled: u32,
    /// Queued requests remaining.
    pub pending: usize,
    /// Started, not yet resident.
    pub in_flight: usize,
    /// Polled completions not yet applied.
    pub deferred_completions: usize,
    /// Applied completions awaiting upload.
    pub awaiting_upload: usize,
    /// Residents evicted to make room this frame.
    pub evicted: u32,
}

/// Result of [`crate::StreamManager::resolve`]: the best drawable key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved<K> {
    /// The resident key to draw (the request itself or a coarser ancestor).
    pub key: K,
    /// Whether `key` is the requested key rather than an ancestor fallback.
    pub exact: bool,
}
