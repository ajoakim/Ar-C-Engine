//! World error type.

use crate::EntityId;

/// Errors returned by [`World`](crate::World) operations.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WorldError {
    /// The entity id is stale or unknown.
    #[error("stale or unknown entity {0:?}")]
    StaleEntity(EntityId),
    /// An entity cannot be parented to itself.
    #[error("entity {0:?} cannot be its own parent")]
    SelfParent(EntityId),
    /// Parenting would create a cycle.
    #[error("parenting {child:?} under {parent:?} would create a cycle")]
    Cycle {
        /// The entity being reparented.
        child: EntityId,
        /// The requested parent.
        parent: EntityId,
    },
    /// A transform contained NaN or infinite components.
    #[error("non-finite transform")]
    NonFinite,
    /// The cached world transform is out of date.
    #[error("world transform of {0:?} is dirty; call update_transforms")]
    DirtyTransform(EntityId),
    /// A math-layer error.
    #[error(transparent)]
    Math(#[from] engine_math::MathError),
}
