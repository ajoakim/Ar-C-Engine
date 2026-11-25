//! Platform error types.

/// Boxed error returned by [`crate::PlatformHandler`] callbacks.
pub type HandlerError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Errors produced by the platform layer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PlatformError {
    /// The supplied window configuration was invalid.
    #[error("invalid window config: {0}")]
    InvalidConfig(&'static str),
    /// The OS event loop failed.
    #[error("event loop error: {0}")]
    EventLoop(String),
    /// Window creation failed.
    #[error("window creation failed: {0}")]
    CreateWindow(String),
    /// The application handler returned an error.
    #[error("handler failed: {0}")]
    Handler(HandlerError),
}
