//! Diagnostics error type.

/// Errors produced by the diagnostics module.
#[derive(Debug, thiserror::Error)]
pub enum DiagnosticsError {
    /// A global tracing subscriber was already installed.
    #[error("a global tracing subscriber is already installed")]
    AlreadyInitialized,
    /// The configured log filter could not be parsed.
    #[error("invalid log filter `{filter}`: {reason}")]
    InvalidFilter {
        /// The offending filter string.
        filter: String,
        /// Parser failure description.
        reason: String,
    },
    /// `end_frame` was called without `begin_frame`.
    #[error("end_frame called without a matching begin_frame")]
    FrameNotStarted,
}
