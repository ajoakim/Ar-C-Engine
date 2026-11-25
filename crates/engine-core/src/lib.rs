//! Engine lifecycle, configuration and module registry.
//!
//! Tier 1 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

// ConfigError embeds ron's SpannedError (fixed by the 02-06 contract); errors here are
// cold-path, so the large Err variant is acceptable.
#![allow(clippy::result_large_err)]

mod config;
mod engine;
mod error;
mod lifecycle;
mod module;
mod registry;

pub use config::{EngineConfig, EngineSection, MAX_CONFIG_BYTES, ModuleConfig};
pub use engine::Engine;
pub use error::{ConfigError, EngineError, LifecycleError, ModuleError, RegistryError};
pub use lifecycle::{EnginePhase, Lifecycle, is_legal_transition};
pub use module::Module;
pub use registry::ModuleRegistry;
