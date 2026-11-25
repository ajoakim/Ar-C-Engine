//! SDK error type.

use engine_core::EngineError;
use engine_math::MathError;
use engine_platform::PlatformError;
use engine_render::RenderError;

/// Errors surfaced through the SDK facade.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SdkError {
    /// Engine lifecycle or configuration failure.
    #[error("engine error: {0}")]
    Engine(#[from] EngineError),
    /// Render backend or graph failure.
    #[error("render error: {0}")]
    Render(#[from] RenderError),
    /// Window or event loop failure.
    #[error("platform error: {0}")]
    Platform(#[from] PlatformError),
    /// Math failure (unknown frame, non-finite position).
    #[error("math error: {0}")]
    Math(#[from] MathError),
    /// Application-level failure.
    #[error("app error: {0}")]
    App(String),
}
