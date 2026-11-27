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

pub use app::{App, AppControl, Resources, SceneState, StartupInfo};
pub use engine_assets::{AssetError, AssetId, AssetRegistry, Handle};
pub use engine_core::{
    Engine, EngineConfig, EngineError, EnginePhase, Module, ModuleConfig, ModuleError,
};
pub use engine_diagnostics::{Budget, DiagnosticsSnapshot, FrameStats, Histogram, tracing};
pub use engine_jobs::{CancelToken, JobError, JobHandle, JobStatus, JobSystem, Priority};
pub use engine_math::{DQuat, DVec3, FrameId, FrameRegistry, MathError, Vec3, WorldPos};
pub use engine_platform::{InputEvent, KeyCode, MouseButton, WindowConfig};
/// Deterministic procedural generation (re-export of `engine-procedural`).
pub use engine_procedural as procedural;
pub use engine_render::{
    Camera, DirectionalLight, DistanceFog, IndexBufferHandle, MAX_DRAWS_PER_FRAME,
    MAX_TEXTURE_DIMENSION, Mat3, Mat4, Material, MaterialId, MaterialTable, MeshHandle,
    NullBackend, Quat, RenderBackend, RenderError, RenderItem, TextureId, VertexPosColor,
    VertexPosNormalUv, VertexPosNormalUvMorph, fog_factor,
};
/// Generic asynchronous streaming (re-export of `engine-streaming`).
pub use engine_streaming as streaming;
pub use engine_world::{EntityId, Transform, World, WorldError, WorldTransform};
pub use error::SdkError;
pub use run::{AppConfig, FrameLatency, PresentConfig, PresentInfo, PresentModeSetting, run_app};
pub use runtime::Runtime;

/// Convenient glob import for apps.
pub mod prelude {
    pub use crate::{
        App, AppConfig, AppControl, Camera, DQuat, DVec3, DirectionalLight, InputEvent, KeyCode,
        Material, MaterialId, Quat, RenderItem, Resources, SceneState, SdkError, Transform, Vec3,
        VertexPosColor, VertexPosNormalUv, WorldPos, run_app,
    };
}
