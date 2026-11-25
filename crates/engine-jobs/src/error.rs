//! Job system errors.

/// Errors raised while building a job system.
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    /// The builder was asked for zero workers.
    #[error("job system needs at least one worker")]
    ZeroWorkers,
    /// A worker thread could not be spawned.
    #[error("failed to spawn worker thread: {0}")]
    Spawn(#[from] std::io::Error),
}
