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
        // O(1): non-root frames are only a hint; `root_frame` walks the parents.
        let frame = self
            .nodes
            .get(parent)
            .ok_or(WorldError::StaleEntity(parent))?
            .frame;
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

    /// Reparent `child` under `parent`. Keeps the local transform (it is
    /// reinterpreted relative to the new parent), not the world transform.
    ///
    /// # Errors
    /// `StaleEntity`, `SelfParent` or `Cycle`; the hierarchy is unchanged on error.
    pub fn set_parent(&mut self, child: EntityId, parent: EntityId) -> Result<(), WorldError> {
        if !self.contains(child) {
            return Err(WorldError::StaleEntity(child));
        }
        if !self.contains(parent) {
            return Err(WorldError::StaleEntity(parent));
        }
        if child == parent {
            return Err(WorldError::SelfParent(child));
        }
        let mut cur = Some(parent);
        while let Some(a) = cur {
            if a == child {
                return Err(WorldError::Cycle { child, parent });
            }
            cur = self.parent(a);
        }
        let old = self.parent(child);
        if old == Some(parent) {
            return Ok(());
        }
        if let Some(p) = old.and_then(|p| self.nodes.get_mut(p)) {
            p.children.retain(|c| *c != child);
        }
        if let Some(p) = self.nodes.get_mut(parent) {
            p.children.push(child);
        }
        if let Some(n) = self.nodes.get_mut(child) {
            n.parent = Some(parent);
        }
        Ok(())
    }

    /// Make `child` a root in `frame`, keeping its local transform.
    ///
    /// # Errors
    /// `StaleEntity(child)`.
    pub fn detach(&mut self, child: EntityId, frame: FrameId) -> Result<(), WorldError> {
        let old = self
            .nodes
            .get(child)
            .ok_or(WorldError::StaleEntity(child))?
            .parent;
        if let Some(p) = old.and_then(|p| self.nodes.get_mut(p)) {
            p.children.retain(|c| *c != child);
        }
        if let Some(n) = self.nodes.get_mut(child) {
            n.parent = None;
            n.frame = frame;
        }
        Ok(())
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

    fn root(w: &mut World) -> EntityId {
        w.spawn(frame(), Transform::IDENTITY).unwrap()
    }

    fn child(w: &mut World, p: EntityId) -> EntityId {
        w.spawn_child(p, Transform::IDENTITY).unwrap()
    }

    #[test]
    fn hierarchy_spawn_child_links_both_ways() {
        let mut w = World::new();
        let p = root(&mut w);
        let c = child(&mut w, p);
        assert_eq!(w.parent(c), Some(p));
        assert_eq!(w.children(p), Some(&[c][..]));
    }

    #[test]
    fn hierarchy_despawn_parent_removes_descendants() {
        let mut w = World::new();
        let other = root(&mut w);
        let p = root(&mut w);
        let c1 = child(&mut w, p);
        let _c2 = child(&mut w, p);
        let g1 = child(&mut w, c1);
        let removed = w.despawn(p).unwrap();
        assert_eq!(removed.len(), 4);
        assert_eq!(removed[0], p);
        assert!(removed.iter().all(|id| !w.contains(*id)));
        assert!(!w.contains(g1));
        assert!(w.contains(other));
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn hierarchy_despawn_child_detaches_from_parent() {
        let mut w = World::new();
        let p = root(&mut w);
        let c1 = child(&mut w, p);
        let c2 = child(&mut w, p);
        let g1 = child(&mut w, c1);
        w.despawn(c1).unwrap();
        assert!(!w.contains(g1));
        assert_eq!(w.children(p), Some(&[c2][..]));
    }

    #[test]
    fn hierarchy_set_parent_rejects_self() {
        let mut w = World::new();
        let a = root(&mut w);
        assert_eq!(w.set_parent(a, a), Err(WorldError::SelfParent(a)));
        assert_eq!(w.parent(a), None);
    }

    #[test]
    fn hierarchy_set_parent_rejects_cycle() {
        let mut w = World::new();
        let p = root(&mut w);
        let c = child(&mut w, p);
        let g = child(&mut w, c);
        assert_eq!(
            w.set_parent(p, g),
            Err(WorldError::Cycle {
                child: p,
                parent: g
            })
        );
        assert_eq!(w.parent(p), None);
        assert_eq!(w.children(p), Some(&[c][..]));
        assert_eq!(w.children(c), Some(&[g][..]));
        assert_eq!(w.children(g), Some(&[][..]));
    }

    #[test]
    fn hierarchy_set_parent_rejects_stale_parent_and_child() {
        let mut w = World::new();
        let a = root(&mut w);
        let dead = root(&mut w);
        w.despawn(dead).unwrap();
        assert_eq!(w.set_parent(a, dead), Err(WorldError::StaleEntity(dead)));
        assert_eq!(w.set_parent(dead, a), Err(WorldError::StaleEntity(dead)));
        assert_eq!(w.parent(a), None);
        assert_eq!(w.children(a), Some(&[][..]));
    }

    #[test]
    fn hierarchy_reparent_moves_child() {
        let mut w = World::new();
        let p1 = root(&mut w);
        let p2 = root(&mut w);
        let c = child(&mut w, p1);
        w.set_parent(c, p2).unwrap();
        assert_eq!(w.children(p1), Some(&[][..]));
        assert_eq!(w.children(p2), Some(&[c][..]));
        assert_eq!(w.root_frame(c), w.root_frame(p2));
    }

    #[test]
    fn hierarchy_detach_makes_root() {
        let mut w = World::new();
        let p = root(&mut w);
        let c = child(&mut w, p);
        let f = frame();
        w.detach(c, f).unwrap();
        assert_eq!(w.parent(c), None);
        assert!(w.roots().contains(&c));
        assert_eq!(w.root_frame(c), Some(f));
        assert_eq!(w.children(p), Some(&[][..]));
    }

    #[test]
    fn hierarchy_deep_chain_no_stack_overflow() {
        let mut w = World::new();
        let top = root(&mut w);
        let mut cur = top;
        for _ in 0..99_999 {
            cur = child(&mut w, cur);
        }
        assert_eq!(w.len(), 100_000);
        let removed = w.despawn(top).unwrap();
        assert_eq!(removed.len(), 100_000);
        assert!(w.is_empty());
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
