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

/// Failure reported by the [`Engine`](crate::Engine) driver.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// Illegal lifecycle transition.
    #[error(transparent)]
    Lifecycle(#[from] LifecycleError),
    /// Module registry failure.
    #[error(transparent)]
    Registry(#[from] RegistryError),
    /// Configuration failure.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// Diagnostics failure.
    #[error(transparent)]
    Diagnostics(#[from] engine_diagnostics::DiagnosticsError),
    /// A module hook failed.
    #[error("module `{module}` failed: {source}")]
    Module {
        /// Module name.
        module: &'static str,
        /// Underlying error.
        #[source]
        source: ModuleError,
    },
    /// Registration attempted after initialization began.
    #[error("modules can only be registered before initialize (phase {phase:?})")]
    RegistrationClosed {
        /// Current phase.
        phase: EnginePhase,
    },
    /// The engine has not been (successfully) initialized.
    #[error("engine has not been initialized")]
    NotInitialized,
    /// `initialize` was already called.
    #[error("engine is already initialized")]
    AlreadyInitialized,
    /// `tick` outside the Running phase.
    #[error("tick requires the Running phase, current phase is {phase:?}")]
    NotRunning {
        /// Current phase.
        phase: EnginePhase,
    },
}
