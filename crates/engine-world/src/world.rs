//! Entity storage and hierarchy.

use engine_math::FrameId;
use slotmap::SlotMap;

use crate::{EntityId, Transform, WorldError};

struct Node {
    parent: Option<EntityId>,
    children: Vec<EntityId>,
    /// Meaningful for roots only.
    frame: FrameId,
    local: Transform,
}

/// Entity world: generational ids plus a parent/child hierarchy.
pub struct World {
    nodes: SlotMap<EntityId, Node>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    /// Empty world.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::with_key(),
        }
    }

    /// Spawn a root entity in `frame`.
    ///
    /// # Errors
    /// `NonFinite` if `local` has NaN/inf components.
    pub fn spawn(&mut self, frame: FrameId, local: Transform) -> Result<EntityId, WorldError> {
        if !local.is_finite() {
            return Err(WorldError::NonFinite);
        }
        Ok(self.nodes.insert(Node {
            parent: None,
            children: Vec::new(),
            frame,
            local,
        }))
    }

    /// Spawn a child of `parent`.
    ///
    /// # Errors
    /// `StaleEntity(parent)` or `NonFinite`.
    pub fn spawn_child(
        &mut self,
        parent: EntityId,
        local: Transform,
    ) -> Result<EntityId, WorldError> {
        if !local.is_finite() {
            return Err(WorldError::NonFinite);
        }
        let frame = self
            .root_frame(parent)
            .ok_or(WorldError::StaleEntity(parent))?;
        let id = self.nodes.insert(Node {
            parent: Some(parent),
            children: Vec::new(),
            frame,
            local,
        });
        if let Some(p) = self.nodes.get_mut(parent) {
            p.children.push(id);
        }
        Ok(id)
    }

    /// Despawn `id` and all descendants; returns removed ids in pre-order.
    ///
    /// # Errors
    /// `StaleEntity(id)`.
    pub fn despawn(&mut self, id: EntityId) -> Result<Vec<EntityId>, WorldError> {
        let parent = self
            .nodes
            .get(id)
            .ok_or(WorldError::StaleEntity(id))?
            .parent;
        if let Some(p) = parent.and_then(|p| self.nodes.get_mut(p)) {
            p.children.retain(|c| *c != id);
        }
        let mut removed = Vec::new();
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            if let Some(node) = self.nodes.remove(cur) {
                removed.push(cur);
                // Reverse so children pop in insertion order.
                stack.extend(node.children.into_iter().rev());
            }
        }
        Ok(removed)
    }

    /// Whether `id` is live.
    #[must_use]
    pub fn contains(&self, id: EntityId) -> bool {
        self.nodes.contains_key(id)
    }

    /// Number of live entities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// True when no entities are live.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Local transform of `id`.
    #[must_use]
    pub fn local(&self, id: EntityId) -> Option<Transform> {
        self.nodes.get(id).map(|n| n.local)
    }

    /// Replace the local transform; state untouched on error.
    ///
    /// # Errors
    /// `NonFinite` or `StaleEntity(id)`.
    pub fn set_local(&mut self, id: EntityId, local: Transform) -> Result<(), WorldError> {
        if !local.is_finite() {
            return Err(WorldError::NonFinite);
        }
        self.nodes
            .get_mut(id)
            .ok_or(WorldError::StaleEntity(id))?
            .local = local;
        Ok(())
    }

    /// Parent of `id`, if any.
    #[must_use]
    pub fn parent(&self, id: EntityId) -> Option<EntityId> {
        self.nodes.get(id).and_then(|n| n.parent)
    }

    /// Children of `id` in insertion order.
    #[must_use]
    pub fn children(&self, id: EntityId) -> Option<&[EntityId]> {
        self.nodes.get(id).map(|n| n.children.as_slice())
    }

    /// Frame of the root of the tree containing `id`.
    #[must_use]
    pub fn root_frame(&self, id: EntityId) -> Option<FrameId> {
        let mut node = self.nodes.get(id)?;
        while let Some(p) = node.parent {
            node = self.nodes.get(p)?;
        }
        Some(node.frame)
    }

    /// All root entities, sorted ascending.
    #[must_use]
    pub fn roots(&self) -> Vec<EntityId> {
        let mut r: Vec<EntityId> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.parent.is_none())
            .map(|(k, _)| k)
            .collect();
        r.sort();
        r
    }

    /// All live ids.
    pub fn iter(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.nodes.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_math::{DVec3, FrameRegistry};

    fn frame() -> FrameId {
        FrameRegistry::new().root()
    }

    #[test]
    fn entity_spawn_returns_distinct_ids() {
        let mut w = World::new();
        let a = w.spawn(frame(), Transform::IDENTITY).unwrap();
        let b = w.spawn(frame(), Transform::IDENTITY).unwrap();
        assert_ne!(a, b);
        assert!(w.contains(a) && w.contains(b));
    }

    #[test]
    fn entity_stale_id_rejected() {
        let mut w = World::new();
        let a = w.spawn(frame(), Transform::IDENTITY).unwrap();
        w.despawn(a).unwrap();
        assert!(!w.contains(a));
        assert!(w.local(a).is_none());
        assert!(w.parent(a).is_none());
        assert_eq!(
            w.set_local(a, Transform::IDENTITY),
            Err(WorldError::StaleEntity(a))
        );
        assert_eq!(w.despawn(a), Err(WorldError::StaleEntity(a)));
    }

    #[test]
    fn entity_slot_reuse_gives_new_id() {
        let mut w = World::new();
        let a = w.spawn(frame(), Transform::IDENTITY).unwrap();
        w.despawn(a).unwrap();
        let b = w.spawn(frame(), Transform::IDENTITY).unwrap();
        assert_ne!(a, b);
        assert!(!w.contains(a));
        assert!(w.contains(b));
    }

    #[test]
    fn entity_default_id_is_never_valid() {
        let mut w = World::new();
        w.spawn(frame(), Transform::IDENTITY).unwrap();
        assert!(!w.contains(EntityId::default()));
    }

    #[test]
    fn entity_rejects_non_finite_transform() {
        let mut w = World::new();
        let bad = Transform::from_translation(DVec3::new(f64::NAN, 0.0, 0.0));
        assert_eq!(w.spawn(frame(), bad), Err(WorldError::NonFinite));
        assert_eq!(w.len(), 0);
    }
}
