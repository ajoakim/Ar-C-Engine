//! Brute-force reference backend.

use crate::{Aabb, Frustum, SpatialError, SpatialIndex, Sphere};
use engine_math::FrameId;
use engine_world::EntityId;
use std::collections::BTreeMap;

/// Brute-force reference backend, used as the oracle in tests and as the
/// trivial second backend proving the trait is pluggable.
#[derive(Debug, Clone)]
pub struct LinearIndex {
    frame: FrameId,
    entries: BTreeMap<EntityId, Aabb>,
}

impl LinearIndex {
    /// Creates an empty index in `index_frame`.
    pub fn new(index_frame: FrameId) -> Self {
        Self {
            frame: index_frame,
            entries: BTreeMap::new(),
        }
    }

    fn collect(&self, out: &mut Vec<EntityId>, pred: impl Fn(&Aabb) -> bool) {
        out.clear();
        out.extend(
            self.entries
                .iter()
                .filter(|(_, b)| pred(b))
                .map(|(id, _)| *id),
        );
    }
}

impl SpatialIndex for LinearIndex {
    fn index_frame(&self) -> FrameId {
        self.frame
    }

    fn insert(&mut self, id: EntityId, bounds: Aabb) -> Result<(), SpatialError> {
        if self.entries.contains_key(&id) {
            return Err(SpatialError::DuplicateEntity(id));
        }
        self.entries.insert(id, bounds);
        Ok(())
    }

    fn update(&mut self, id: EntityId, bounds: Aabb) -> Result<(), SpatialError> {
        match self.entries.get_mut(&id) {
            Some(b) => {
                *b = bounds;
                Ok(())
            }
            None => Err(SpatialError::UnknownEntity(id)),
        }
    }

    fn remove(&mut self, id: EntityId) -> bool {
        self.entries.remove(&id).is_some()
    }

    fn bounds(&self, id: EntityId) -> Option<Aabb> {
        self.entries.get(&id).copied()
    }

    fn query_frustum(&self, frustum: &Frustum, out: &mut Vec<EntityId>) {
        self.collect(out, |b| frustum.intersects_aabb(b));
    }

    fn query_region(&self, region: &Aabb, out: &mut Vec<EntityId>) {
        self.collect(out, |b| b.intersects(region));
    }

    fn query_radius(&self, sphere: &Sphere, out: &mut Vec<EntityId>) {
        self.collect(out, |b| sphere.intersects_aabb(b));
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_math::{DQuat, DVec3, FrameRegistry};
    use engine_world::{Transform, World};

    fn ids(n: usize) -> (FrameId, Vec<EntityId>) {
        let root = FrameRegistry::new().root();
        let mut w = World::new();
        let v = (0..n)
            .map(|_| w.spawn(root, Transform::IDENTITY).unwrap())
            .collect();
        (root, v)
    }
    fn bx(c: DVec3) -> Aabb {
        Aabb::from_center_half_extents(c, DVec3::splat(0.5)).unwrap()
    }

    #[test]
    fn linear_insert_duplicate_rejected() {
        let (f, e) = ids(1);
        let mut i = LinearIndex::new(f);
        i.insert(e[0], bx(DVec3::ZERO)).unwrap();
        assert_eq!(
            i.insert(e[0], bx(DVec3::ONE)),
            Err(SpatialError::DuplicateEntity(e[0]))
        );
        assert_eq!(i.len(), 1);
    }

    #[test]
    fn linear_update_absent_rejected() {
        let (f, e) = ids(1);
        let mut i = LinearIndex::new(f);
        assert_eq!(
            i.update(e[0], bx(DVec3::ZERO)),
            Err(SpatialError::UnknownEntity(e[0]))
        );
    }

    #[test]
    fn linear_remove_returns_presence() {
        let (f, e) = ids(1);
        let mut i = LinearIndex::new(f);
        i.insert(e[0], bx(DVec3::ZERO)).unwrap();
        assert!(i.remove(e[0]));
        assert!(!i.remove(e[0]));
        assert_eq!(i.len(), 0);
        assert!(i.is_empty());
    }

    #[test]
    fn linear_queries_sorted_and_cleared() {
        let (f, mut e) = ids(5);
        let mut i = LinearIndex::new(f);
        // Insert in reverse order.
        for (k, id) in e.iter().rev().enumerate() {
            i.insert(*id, bx(DVec3::new(0.0, 0.0, -(k as f64) - 2.0)))
                .unwrap();
        }
        e.sort();
        let junk = vec![e[0]; 3];
        let mut out = junk.clone();
        i.query_region(
            &Aabb::new(DVec3::splat(-100.0), DVec3::splat(100.0)).unwrap(),
            &mut out,
        );
        assert_eq!(out, e);
        let mut out = junk.clone();
        i.query_radius(&Sphere::new(DVec3::ZERO, 50.0).unwrap(), &mut out);
        assert_eq!(out, e);
        let mut out = junk;
        let fr =
            Frustum::from_perspective(DVec3::ZERO, DQuat::IDENTITY, 1.0, 1.0, 0.1, 100.0).unwrap();
        i.query_frustum(&fr, &mut out);
        assert_eq!(out, e);
        let mut out = vec![e[0]];
        i.query_region(&bx(DVec3::splat(1e6)), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn linear_update_moves_entity() {
        let (f, e) = ids(1);
        let mut i = LinearIndex::new(f);
        i.insert(e[0], bx(DVec3::ZERO)).unwrap();
        i.update(e[0], bx(DVec3::splat(1000.0))).unwrap();
        let mut out = Vec::new();
        i.query_region(&bx(DVec3::ZERO), &mut out);
        assert!(out.is_empty());
        i.query_region(&bx(DVec3::splat(1000.0)), &mut out);
        assert_eq!(out, vec![e[0]]);
    }

    #[test]
    fn linear_bounds_accessor() {
        let (f, e) = ids(1);
        let mut i = LinearIndex::new(f);
        let b = bx(DVec3::ONE);
        i.insert(e[0], b).unwrap();
        assert_eq!(i.bounds(e[0]), Some(b));
        i.remove(e[0]);
        assert_eq!(i.bounds(e[0]), None);
        let _boxed: Box<dyn SpatialIndex> = Box::new(LinearIndex::new(f));
    }
}
