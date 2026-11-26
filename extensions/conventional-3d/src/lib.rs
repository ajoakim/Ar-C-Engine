//! Conventional 3D scene extension (meshes, lights, materials, camera controllers).
//!
//! Extension crate (tier 6). Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.
//! Engine crates under crates/ must never depend on this crate.

// SdkError conversions are cold-path (consistent with engine-sdk).
#![allow(clippy::result_large_err)]

mod error;
pub mod fly_camera;
pub mod import;
pub mod loader;
pub mod procedural;
pub mod scene_builder;

pub use error::ImportError;
pub use fly_camera::FlyCamera;
pub use import::{
    ImportLimits, ImportedMaterial, ImportedMesh, ImportedModel, ImportedNode, ImportedTexture,
    decode_image, import_gltf_path, import_gltf_path_with, import_gltf_slice,
    import_gltf_slice_with,
};
pub use loader::{LoadResult, LoadTicket, ModelAssets, ModelLoader, RegisteredModel};
pub use scene_builder::{SceneBuilder, UploadedModel, upload_model, upload_registered};
