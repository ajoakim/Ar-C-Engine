//! Error types for the engine-core crate.

use crate::lifecycle::EnginePhase;

/// Illegal lifecycle operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleError {
    /// The requested transition is not in the allow-list.
    #[error("illegal lifecycle transition {from:?} -> {to:?}")]
    IllegalTransition {
        /// Phase the engine was in.
        from: EnginePhase,
        /// Requested phase.
        to: EnginePhase,
    },
}

/// Configuration loading/parsing failure.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// File could not be read.
    #[error("failed to read config file {path}: {source}")]
    Io {
        /// Path of the file.
        path: std::path::PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// File exceeds the size limit.
    #[error("config file {path} is {size} bytes, limit is {limit}")]
    TooLarge {
        /// Path of the file.
        path: std::path::PathBuf,
        /// Actual size.
        size: u64,
        /// Maximum allowed size.
        limit: u64,
    },
    /// Top-level RON was invalid.
    #[error("invalid RON config: {0}")]
    Parse(#[from] ron::error::SpannedError),
    /// A module section failed to deserialize.
    #[error("invalid config section for module `{module}`: {source}")]
    ModuleSection {
        /// Module name.
        module: String,
        /// Underlying error.
        #[source]
        source: ron::error::SpannedError,
    },
}

/// Failure reported by a module hook.
#[derive(Debug, thiserror::Error)]
pub enum ModuleError {
    /// Free-form message.
    #[error("{0}")]
    Message(String),
    /// Configuration failure.
    #[error(transparent)]
    Config(#[from] ConfigError),
}

/// Module registration / ordering failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// Same module name registered twice.
    #[error("module `{0}` registered twice")]
    DuplicateModule(String),
    /// A declared dependency is not registered.
    #[error("module `{module}` depends on unregistered module `{dependency}`")]
    MissingDependency {
        /// Module declaring the dependency.
        module: String,
        /// Missing dependency name.
        dependency: String,
    },
    /// Dependencies form a cycle.
    #[error("dependency cycle among modules {0:?}")]
    DependencyCycle(Vec<String>),
}
