//! Errors for generator layer composition and hashing.

/// Failure while composing layers or hashing their outputs.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProcError {
    /// Two layers in one stack share a name.
    #[error("duplicate layer name {0:?}")]
    DuplicateLayer(&'static str),
    /// A layer declared zero outputs.
    #[error("layer {0:?} declares zero outputs")]
    EmptyLayer(&'static str),
    /// A layer produced NaN or infinity.
    #[error("layer {layer:?} produced a non-finite value at index {index}")]
    NonFinite {
        /// Layer name.
        layer: &'static str,
        /// Index of the offending value within the layer output.
        index: u64,
    },
}
