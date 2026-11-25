//! Render error type.

/// Errors produced by render backends and packet validation.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// Unknown, destroyed or forged handle.
    #[error("invalid {kind} handle {id}")]
    InvalidHandle {
        /// Handle kind.
        kind: &'static str,
        /// Raw id.
        id: u32,
    },
    /// Buffer data was empty.
    #[error("buffer data is empty")]
    EmptyBuffer,
    /// Buffer exceeded the size limit.
    #[error("buffer of {size} bytes exceeds maximum {max}")]
    BufferTooLarge {
        /// Requested size.
        size: usize,
        /// Maximum size.
        max: usize,
    },
    /// Buffer used with the wrong usage.
    #[error("buffer {id} has wrong usage for this operation")]
    WrongBufferUsage {
        /// Raw buffer id.
        id: u32,
    },
    /// Mesh with zero vertices.
    #[error("mesh has zero vertices")]
    EmptyMesh,
    /// Mesh reads past its buffer.
    #[error("mesh needs {required} bytes of buffer {buffer} but it holds {size}")]
    MeshOutOfBounds {
        /// Raw buffer id.
        buffer: u32,
        /// Bytes required.
        required: u64,
        /// Bytes available.
        size: u64,
    },
    /// A matrix or color contained NaN or infinity.
    #[error("non-finite transform or value")]
    NonFiniteTransform,
    /// Depth clear value violates reverse-Z.
    #[error("depth clear {value} violates reverse-Z (must be 0.0)")]
    InvalidDepthClear {
        /// Offending value.
        value: f32,
    },
    /// Too many draws in a frame.
    #[error("{count} draws exceeds per-frame maximum {max}")]
    TooManyDraws {
        /// Submitted draw count.
        count: usize,
        /// Maximum.
        max: usize,
    },
    /// Invalid camera description.
    #[error("invalid camera: {0}")]
    InvalidCamera(&'static str),
    /// Handle counter exhausted.
    #[error("handle space exhausted")]
    HandleExhausted,
    /// Math error.
    #[error(transparent)]
    Math(#[from] engine_math::MathError),
    /// Render graph error.
    #[error("render graph: {0}")]
    Graph(#[from] crate::graph::GraphError),
    /// Pass does not target the backbuffer.
    #[error("pass {pass} does not write the backbuffer")]
    UnsupportedPassTarget {
        /// Pass name.
        pass: String,
    },
    /// Backend-specific failure.
    #[error("backend error: {0}")]
    Backend(String),
}
