//! The pluggable spatial index trait.

use crate::{Aabb, Frustum, SpatialError, Sphere};
use engine_math::FrameId;
use engine_world::EntityId;

/// Pluggable spatial index over EntityIds in one index frame (ADR-0006).
///
/// Query contract: `out` is cleared, then filled with matching ids sorted
/// ascending, without duplicates.
/// - Region: `bounds.intersects(region)`.
/// - Radius: `sphere.intersects_aabb(bounds)` (takes a validated [`Sphere`]).
/// - Frustum: `frustum.intersects_aabb(bounds)` (conservative).
///
/// Object-safe: `Box<dyn SpatialIndex>` works.
pub trait SpatialIndex {
    /// Frame all coordinates are expressed in.
    fn index_frame(&self) -> FrameId;
    /// Inserts an entity; errors with `DuplicateEntity` if present.
    fn insert(&mut self, id: EntityId, bounds: Aabb) -> Result<(), SpatialError>;
    /// Updates an entity's bounds; errors with `UnknownEntity` if absent.
    fn update(&mut self, id: EntityId, bounds: Aabb) -> Result<(), SpatialError>;
    /// Removes an entity; returns whether it was present.
    fn remove(&mut self, id: EntityId) -> bool;
    /// Current bounds of an entity.
    fn bounds(&self, id: EntityId) -> Option<Aabb>;
    /// Entities whose bounds pass the frustum test.
    fn query_frustum(&self, frustum: &Frustum, out: &mut Vec<EntityId>);
    /// Entities whose bounds intersect the region.
    fn query_region(&self, region: &Aabb, out: &mut Vec<EntityId>);
    /// Entities whose bounds intersect the sphere.
    fn query_radius(&self, sphere: &Sphere, out: &mut Vec<EntityId>);
    /// Number of indexed entities.
    fn len(&self) -> usize;
    /// Whether the index is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Removes all entities.
    fn clear(&mut self);
}
