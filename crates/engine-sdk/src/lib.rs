//! Public SDK facade for applications and extensions.
//!
//! Tier 5 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//!
//! Apps implement [`App`] and never name GPU or windowing crates.

// SdkError wraps the large EngineError; errors are cold-path (consistent with engine-core).
#![allow(clippy::result_large_err)]

mod app;
mod error;
mod run;
mod runtime;

pub use app::{App, AppControl, Resources, SceneState};
pub use engine_core::{
    Engine, EngineConfig, EngineError, EnginePhase, Module, ModuleConfig, ModuleError,
};
pub use engine_diagnostics::{FrameStats, tracing};
pub use engine_math::{DQuat, DVec3, FrameId, FrameRegistry, MathError, Vec3, WorldPos};
pub use engine_platform::{InputEvent, KeyCode, MouseButton, WindowConfig};
pub use engine_render::{
    Camera, Mat4, MeshHandle, NullBackend, Quat, RenderBackend, RenderError, RenderItem,
    VertexPosColor,
};
pub use error::SdkError;
pub use run::{AppConfig, run_app};
pub use runtime::Runtime;

/// Convenient glob import for apps.
pub mod prelude {
    pub use crate::{
        App, AppConfig, AppControl, Camera, DQuat, DVec3, InputEvent, KeyCode, Quat, RenderItem,
        Resources, SceneState, SdkError, Vec3, VertexPosColor, WorldPos, run_app,
    };
}
