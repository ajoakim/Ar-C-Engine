//! Default spatial backend: a uniform hash grid (ADR-0006).

use crate::{Aabb, Frustum, SpatialError, SpatialIndex, Sphere};
use engine_math::FrameId;
use engine_world::EntityId;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Configuration for [`HashGridIndex`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HashGridConfig {
    /// Edge length of a grid cell in metres. Must be finite and > 0.
    pub cell_size: f64,
    /// Entities spanning more cells than this go to the oversize list.
    pub max_cells_per_entity: u64,
}

impl Default for HashGridConfig {
    fn default() -> Self {
        Self {
            cell_size: 64.0,
            max_cells_per_entity: 512,
        }
    }
}

type CellKey = [i64; 3];

#[derive(Clone, Debug)]
struct Entry {
    bounds: Aabb,
    /// Inclusive cell range; `None` means the entity lives in the oversize set.
    cells: Option<(CellKey, CellKey)>,
}

/// Uniform hash-grid spatial index over f64 coordinates in one index frame.
#[derive(Debug, Clone)]
pub struct HashGridIndex {
    frame: FrameId,
    config: HashGridConfig,
    cells: HashMap<CellKey, Vec<EntityId>>,
    entries: BTreeMap<EntityId, Entry>,
    oversize: BTreeSet<EntityId>,
}

fn cell_coord(v: f64, cell: f64) -> i64 {
    // `as` saturates for out-of-range floats; inputs are finite.
    (v / cell).floor() as i64
}

/// Number of cells in an inclusive range; `None` on overflow.
fn span(lo: CellKey, hi: CellKey) -> Option<u128> {
    let mut total: u128 = 1;
    for a in 0..3 {
        let n = (i128::from(hi[a]) - i128::from(lo[a]) + 1) as u128;
        total = total.checked_mul(n)?;
    }
    Some(total)
}

impl HashGridIndex {
    /// Creates an empty index in `index_frame`.
    pub fn new(index_frame: FrameId, config: HashGridConfig) -> Result<Self, SpatialError> {
        if !config.cell_size.is_finite() || config.cell_size <= 0.0 {
            return Err(SpatialError::InvalidConfig(
                "cell_size must be finite and > 0",
            ));
        }
        if config.max_cells_per_entity == 0 {
            return Err(SpatialError::InvalidConfig(
                "max_cells_per_entity must be >= 1",
            ));
        }
        Ok(Self {
            frame: index_frame,
            config,
            cells: HashMap::new(),
            entries: BTreeMap::new(),
            oversize: BTreeSet::new(),
        })
    }

    /// The configuration in use.
    pub fn config(&self) -> HashGridConfig {
        self.config
    }

    /// Number of non-empty cells.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Number of entities in the oversize list.
    pub fn oversize_count(&self) -> usize {
        self.oversize.len()
    }

    fn range_of(&self, b: &Aabb) -> (CellKey, CellKey) {
        let c = self.config.cell_size;
        let (mn, mx) = (b.min(), b.max());
        (
            [
                cell_coord(mn.x, c),
                cell_coord(mn.y, c),
                cell_coord(mn.z, c),
            ],
            [
                cell_coord(mx.x, c),
                cell_coord(mx.y, c),
                cell_coord(mx.z, c),
            ],
        )
    }

    fn place(&mut self, id: EntityId, bounds: Aabb) {
        let (lo, hi) = self.range_of(&bounds);
        let fits = span(lo, hi).is_some_and(|n| n <= u128::from(self.config.max_cells_per_entity));
        if fits {
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        self.cells.entry([x, y, z]).or_default().push(id);
                    }
                }
            }
            self.entries.insert(
                id,
                Entry {
                    bounds,
                    cells: Some((lo, hi)),
                },
            );
        } else {
            self.oversize.insert(id);
            self.entries.insert(
                id,
                Entry {
                    bounds,
                    cells: None,
                },
            );
        }
    }

    fn unplace(&mut self, id: EntityId) -> bool {
        let Some(entry) = self.entries.remove(&id) else {
            return false;
        };
        match entry.cells {
            Some((lo, hi)) => {
                for x in lo[0]..=hi[0] {
                    for y in lo[1]..=hi[1] {
                        for z in lo[2]..=hi[2] {
                            let key = [x, y, z];
                            if let Some(v) = self.cells.get_mut(&key) {
                                v.retain(|e| *e != id);
                                if v.is_empty() {
                                    self.cells.remove(&key);
                                }
                            }
                        }
                    }
                }
            }
            None => {
                self.oversize.remove(&id);
            }
        }
        true
    }

    fn query_with(&self, range_box: &Aabb, pred: impl Fn(&Aabb) -> bool, out: &mut Vec<EntityId>) {
        out.clear();
        let (lo, hi) = self.range_of(range_box);
        let use_grid = span(lo, hi).is_some_and(|n| n <= self.entries.len() as u128);
        if !use_grid {
            out.extend(
                self.entries
                    .iter()
                    .filter(|(_, e)| pred(&e.bounds))
                    .map(|(id, _)| *id),
            );
            return;
        }
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    if let Some(v) = self.cells.get(&[x, y, z]) {
                        out.extend_from_slice(v);
                    }
                }
            }
        }
        out.extend(self.oversize.iter().copied());
        out.sort_unstable();
        out.dedup();
        out.retain(|id| self.entries.get(id).is_some_and(|e| pred(&e.bounds)));
    }
}

impl SpatialIndex for HashGridIndex {
    fn index_frame(&self) -> FrameId {
        self.frame
    }

    fn insert(&mut self, id: EntityId, bounds: Aabb) -> Result<(), SpatialError> {
        if self.entries.contains_key(&id) {
            return Err(SpatialError::DuplicateEntity(id));
        }
        self.place(id, bounds);
        Ok(())
    }

    fn update(&mut self, id: EntityId, bounds: Aabb) -> Result<(), SpatialError> {
        if !self.unplace(id) {
            return Err(SpatialError::UnknownEntity(id));
        }
        self.place(id, bounds);
        Ok(())
    }

    fn remove(&mut self, id: EntityId) -> bool {
        self.unplace(id)
    }

    fn bounds(&self, id: EntityId) -> Option<Aabb> {
        self.entries.get(&id).map(|e| e.bounds)
    }

    fn query_frustum(&self, frustum: &Frustum, out: &mut Vec<EntityId>) {
        self.query_with(&frustum.bounds(), |b| frustum.intersects_aabb(b), out);
    }

    fn query_region(&self, region: &Aabb, out: &mut Vec<EntityId>) {
        self.query_with(region, |b| b.intersects(region), out);
    }

    fn query_radius(&self, sphere: &Sphere, out: &mut Vec<EntityId>) {
        self.query_with(&sphere.bounds(), |b| sphere.intersects_aabb(b), out);
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn clear(&mut self) {
        self.cells.clear();
        self.entries.clear();
        self.oversize.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LinearIndex;
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
    fn bx(c: DVec3, h: f64) -> Aabb {
        Aabb::from_center_half_extents(c, DVec3::splat(h)).unwrap()
    }
    fn grid(f: FrameId) -> HashGridIndex {
        HashGridIndex::new(f, HashGridConfig::default()).unwrap()
    }

    #[test]
    fn hash_grid_rejects_bad_config() {
        let (f, _) = ids(0);
        for s in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let c = HashGridConfig {
                cell_size: s,
                max_cells_per_entity: 8,
            };
            assert!(matches!(
                HashGridIndex::new(f, c),
                Err(SpatialError::InvalidConfig(_))
            ));
        }
        let c = HashGridConfig {
            cell_size: 1.0,
            max_cells_per_entity: 0,
        };
        assert!(matches!(
            HashGridIndex::new(f, c),
            Err(SpatialError::InvalidConfig(_))
        ));
    }

    #[test]
    fn hash_grid_insert_update_remove() {
        let (f, e) = ids(2);
        let mut i = grid(f);
        i.insert(e[0], bx(DVec3::ZERO, 100.0)).unwrap();
        assert_eq!(
            i.insert(e[0], bx(DVec3::ZERO, 1.0)),
            Err(SpatialError::DuplicateEntity(e[0]))
        );
        assert_eq!(
            i.update(e[1], bx(DVec3::ZERO, 1.0)),
            Err(SpatialError::UnknownEntity(e[1]))
        );
        i.insert(e[1], bx(DVec3::splat(500.0), 1.0)).unwrap();
        i.update(e[1], bx(DVec3::splat(-500.0), 1.0)).unwrap();
        assert!(i.remove(e[0]));
        assert!(!i.remove(e[0]));
        assert!(i.remove(e[1]));
        assert_eq!(i.cell_count(), 0);
        assert!(i.is_empty());
    }

    #[test]
    fn hash_grid_spanning_entity_deduplicated() {
        let (f, e) = ids(1);
        let mut i = grid(f);
        i.insert(e[0], bx(DVec3::ZERO, 10.0)).unwrap();
        assert_eq!(i.cell_count(), 8);
        let mut out = Vec::new();
        i.query_region(&bx(DVec3::ZERO, 100.0), &mut out);
        assert_eq!(out, vec![e[0]]);
    }

    #[test]
    fn hash_grid_oversize_entity() {
        let (f, e) = ids(1);
        let mut i = grid(f);
        i.insert(e[0], bx(DVec3::ZERO, 1e6)).unwrap();
        assert_eq!(i.oversize_count(), 1);
        let mut out = Vec::new();
        i.query_region(&bx(DVec3::splat(5e5), 1.0), &mut out);
        assert_eq!(out, vec![e[0]]);
        i.query_region(&bx(DVec3::splat(5e6), 1.0), &mut out);
        assert!(out.is_empty());
        assert!(i.remove(e[0]));
        assert_eq!(i.oversize_count(), 0);
    }

    #[test]
    fn hash_grid_negative_coordinates() {
        let (f, e) = ids(2);
        let mut i = grid(f);
        i.insert(e[0], bx(DVec3::splat(-64.0), 0.01)).unwrap();
        i.insert(e[1], bx(DVec3::splat(-0.0001), 0.00001)).unwrap();
        let mut out = Vec::new();
        i.query_region(&bx(DVec3::splat(-64.0), 0.5), &mut out);
        assert_eq!(out, vec![e[0]]);
        i.query_region(&bx(DVec3::splat(-0.0001), 0.001), &mut out);
        assert_eq!(out, vec![e[1]]);
    }

    #[test]
    fn hash_grid_huge_frustum_fast() {
        let (f, e) = ids(1000);
        let mut g = grid(f);
        let mut l = LinearIndex::new(f);
        for (k, id) in e.iter().enumerate() {
            let c = DVec3::new(
                (k % 10) as f64 * 100.0,
                (k / 10 % 10) as f64 * 100.0,
                -((k / 100) as f64) * 100.0 - 10.0,
            );
            g.insert(*id, bx(c, 1.0)).unwrap();
            l.insert(*id, bx(c, 1.0)).unwrap();
        }
        let fr =
            Frustum::from_perspective(DVec3::ZERO, DQuat::IDENTITY, 1.2, 1.0, 0.1, 1e7).unwrap();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        g.query_frustum(&fr, &mut a);
        l.query_frustum(&fr, &mut b);
        assert_eq!(a, b);
    }
}
