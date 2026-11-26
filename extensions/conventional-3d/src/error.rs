//! Import error type.

/// Errors produced while importing untrusted glTF/image data.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ImportError {
    /// Filesystem error.
    #[error("io error: {0}")]
    Io(String),
    /// Input exceeds the configured size cap.
    #[error("file too large: {size} bytes (max {max})")]
    FileTooLarge {
        /// Actual size in bytes.
        size: u64,
        /// Allowed maximum.
        max: u64,
    },
    /// glTF parsing/validation error.
    #[error("gltf error: {0}")]
    Gltf(String),
    /// External URI while importing from a byte slice.
    #[error("external URI {0} not allowed when importing from a byte slice")]
    ExternalReference(String),
    /// URI rejected by the confinement policy.
    #[error("unsupported or unsafe URI: {0}")]
    UnsupportedUri(String),
    /// Image decode error.
    #[error("image error: {0}")]
    Image(String),
    /// Image format/color type not supported.
    #[error("unsupported image format: {0}")]
    UnsupportedImageFormat(String),
    /// Texture dimensions exceed the cap.
    #[error("texture {width}x{height} exceeds max dimension {max}")]
    TextureTooLarge {
        /// Width in texels.
        width: u32,
        /// Height in texels.
        height: u32,
        /// Allowed maximum dimension.
        max: u32,
    },
    /// Too many vertices in a primitive.
    #[error("mesh {mesh} has {count} vertices (max {max})")]
    TooManyVertices {
        /// Mesh index.
        mesh: usize,
        /// Vertex count.
        count: usize,
        /// Allowed maximum.
        max: u32,
    },
    /// Too many indices in a primitive.
    #[error("mesh {mesh} has {count} indices (max {max})")]
    TooManyIndices {
        /// Mesh index.
        mesh: usize,
        /// Index count.
        count: usize,
        /// Allowed maximum.
        max: u32,
    },
    /// Index out of range.
    #[error("mesh {mesh} index {index} out of range for {vertex_count} vertices")]
    InvalidIndex {
        /// Mesh index.
        mesh: usize,
        /// Offending index.
        index: u32,
        /// Vertex count.
        vertex_count: u32,
    },
    /// Malformed accessor or view.
    #[error("invalid accessor: {0}")]
    InvalidAccessor(&'static str),
    /// Non-finite numeric data.
    #[error("non-finite value in {0}")]
    NonFinite(&'static str),
    /// Too many nodes.
    #[error("too many nodes (max {max})")]
    TooManyNodes {
        /// Allowed maximum.
        max: usize,
    },
    /// Hierarchy too deep.
    #[error("node hierarchy too deep (max {max})")]
    HierarchyTooDeep {
        /// Allowed maximum depth.
        max: usize,
    },
    /// Node graph contains a cycle or shared node.
    #[error("node hierarchy contains a cycle")]
    HierarchyCycle,
    /// Background job failed.
    #[error("import job failed: {0}")]
    JobFailed(String),
    /// Asset registration failed.
    #[error("asset error: {0}")]
    Asset(String),
}

impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<gltf::Error> for ImportError {
    fn from(e: gltf::Error) -> Self {
        Self::Gltf(e.to_string())
    }
}

impl From<image::ImageError> for ImportError {
    fn from(e: image::ImageError) -> Self {
        Self::Image(e.to_string())
    }
}

impl From<ImportError> for engine_sdk::SdkError {
    fn from(e: ImportError) -> Self {
        Self::App(format!("import: {e}"))
    }
}
