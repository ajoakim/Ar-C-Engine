//! Engine lifecycle, configuration and module registry.
//!
//! Tier 1 crate. Allowed dependencies and layering rules are documented in
//! `docs/architecture/dependency-direction.md`.

// ConfigError embeds ron's SpannedError (fixed by the 02-06 contract); errors here are
// cold-path, so the large Err variant is acceptable.
#![allow(clippy::result_large_err)]

mod config;
mod error;
mod lifecycle;
mod module;
mod registry;

pub use config::{EngineConfig, EngineSection, ModuleConfig, MAX_CONFIG_BYTES};
pub use error::{ConfigError, LifecycleError, ModuleError, RegistryError};
pub use lifecycle::{is_legal_transition, EnginePhase, Lifecycle};
pub use module::Module;
pub use registry::ModuleRegistry;
