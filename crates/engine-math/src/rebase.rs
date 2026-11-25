//! Explicit origin rebasing over a store of registered positions.
//!
//! There is deliberately no automatic trigger or threshold here: when to rebase is a policy
//! owned by the app or a module.

use glam::{DAffine3, DVec3};

use crate::error::MathError;
use crate::frame::{FrameId, FrameRegistry};
use crate::world_pos::WorldPos;

/// Handle to a position registered in a [`PositionStore`]. Slots are never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PosId(u32);

/// Store of positions that participate in rebasing.
#[derive(Default)]
pub struct PositionStore {
    slots: Vec<Option<WorldPos>>,
}

impl PositionStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a position and returns its id.
    pub fn insert(&mut self, pos: WorldPos) -> PosId {
        // A store with more than u32::MAX slots is not realistic; saturate rather than panic.
        let id = PosId(u32::try_from(self.slots.len()).unwrap_or(u32::MAX));
        self.slots.push(Some(pos));
        id
    }

    fn index(id: PosId) -> usize {
        usize::try_from(id.0).unwrap_or(usize::MAX)
    }

    /// Returns the position, or `None` if absent or removed.
    pub fn get(&self, id: PosId) -> Option<WorldPos> {
        self.slots.get(Self::index(id)).copied().flatten()
    }

    /// Replaces a live position; `UnknownPosition` if absent or removed.
    pub fn set(&mut self, id: PosId, pos: WorldPos) -> Result<(), MathError> {
        match self.slots.get_mut(Self::index(id)) {
            Some(slot @ Some(_)) => {
                *slot = Some(pos);
                Ok(())
            }
            _ => Err(MathError::UnknownPosition(id)),
        }
    }

    /// Removes and returns a position.
    pub fn remove(&mut self, id: PosId) -> Option<WorldPos> {
        self.slots.get_mut(Self::index(id)).and_then(Option::take)
    }

    /// Number of live positions.
    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// True if no live positions exist.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterates live positions.
    pub fn iter(&self) -> impl Iterator<Item = (PosId, WorldPos)> + '_ {
        self.slots.iter().enumerate().filter_map(|(i, s)| {
            let id = PosId(u32::try_from(i).ok()?);
            s.map(|p| (id, p))
        })
    }
}

/// Moves the origin of `frame` to its old-local point `new_origin`.
///
/// The frame's transform, its child frames' transforms and all stored positions in `frame`
/// are adjusted so every absolute placement and pairwise relative offset is preserved.
/// All inputs are validated before anything is mutated, so an `Err` leaves state untouched.
///
/// There is no automatic trigger: callers decide when to rebase. f64 rebasing is not
/// bit-exact in general (ulp at 6.4e6 m is about 9.3e-10 m), and repeated rebases accumulate
/// bounded drift.
pub fn rebase(
    frames: &mut FrameRegistry,
    store: &mut PositionStore,
    frame: FrameId,
    new_origin: DVec3,
) -> Result<(), MathError> {
    if frame == frames.root() {
        return Err(MathError::RootFrame);
    }
    if !new_origin.is_finite() {
        return Err(MathError::NonFinite);
    }
    let old_to_parent = frames.to_parent(frame)?;
    let new_frame_t = old_to_parent * DAffine3::from_translation(new_origin);
    let shift_back = DAffine3::from_translation(-new_origin);

    let mut child_updates = Vec::new();
    for c in frames.children(frame)? {
        child_updates.push((c, shift_back * frames.to_parent(c)?));
    }
    let mut pos_updates = Vec::new();
    for (id, p) in store.iter().filter(|(_, p)| p.frame() == frame) {
        pos_updates.push((id, WorldPos::new(frame, p.local() - new_origin)?));
    }
    if !new_frame_t.matrix3.is_finite()
        || !new_frame_t.translation.is_finite()
        || child_updates
            .iter()
            .any(|(_, t)| !t.matrix3.is_finite() || !t.translation.is_finite())
    {
        return Err(MathError::NonFinite);
    }

    frames.set_to_parent(frame, new_frame_t)?;
    for (c, t) in child_updates {
        frames.set_to_parent(c, t)?;
    }
    for (id, p) in pos_updates {
        store.set(id, p)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn assert_vec_near(a: DVec3, b: DVec3, tol: f64) {
        assert!(
            (a - b).abs().max_element() <= tol,
            "{a:?} vs {b:?} (tol {tol})"
        );
    }

    struct Scene {
        frames: FrameRegistry,
        store: PositionStore,
        world: FrameId,
        vehicle: FrameId,
        ids: Vec<PosId>,
    }

    fn scene() -> Scene {
        let mut frames = FrameRegistry::new();
        let world = frames
            .add_frame("world", frames.root(), DAffine3::IDENTITY)
            .unwrap();
        let vehicle = frames
            .add_frame(
                "vehicle",
                world,
                DAffine3::from_translation(DVec3::new(6.4e6 + 10.0, 0.0, 0.0)),
            )
            .unwrap();
        let mut store = PositionStore::new();
        let mut ids = Vec::new();
        for (x, y, z) in [
            (6.4e6, 0.0, 0.0),
            (6.4e6 + 0.001, 0.0, 0.0),
            (6.4e6, 1.5, -2.25),
        ] {
            ids.push(store.insert(WorldPos::from_xyz(world, x, y, z).unwrap()));
        }
        ids.push(store.insert(WorldPos::from_xyz(vehicle, 0.5, 0.0, 0.0).unwrap()));
        Scene {
            frames,
            store,
            world,
            vehicle,
            ids,
        }
    }

    fn pairwise(s: &Scene) -> Vec<DVec3> {
        let mut out = Vec::new();
        for &a in &s.ids {
            for &b in &s.ids {
                let (pa, pb) = (s.store.get(a).unwrap(), s.store.get(b).unwrap());
                out.push(s.frames.delta(pa, pb).unwrap());
            }
        }
        out
    }

    fn roots(s: &Scene) -> Vec<DVec3> {
        let root = s.frames.root();
        s.ids
            .iter()
            .map(|&i| {
                s.frames
                    .convert(s.store.get(i).unwrap(), root)
                    .unwrap()
                    .local()
            })
            .collect()
    }

    #[test]
    fn rebase_round_trip_preserves_relative_offsets_at_6_4e6() {
        let mut s = scene();
        let before = pairwise(&s);
        let o = DVec3::new(6.4e6, 0.0, 0.0);
        rebase(&mut s.frames, &mut s.store, s.world, o).unwrap();
        assert_eq!(s.store.get(s.ids[0]).unwrap().local(), DVec3::ZERO);
        for (a, b) in before.iter().zip(pairwise(&s)) {
            assert_vec_near(*a, b, 1e-8);
        }
        rebase(&mut s.frames, &mut s.store, s.world, -o).unwrap();
        assert_vec_near(
            s.store.get(s.ids[0]).unwrap().local(),
            DVec3::new(6.4e6, 0.0, 0.0),
            1e-8,
        );
        assert_vec_near(
            s.store.get(s.ids[1]).unwrap().local(),
            DVec3::new(6.4e6 + 0.001, 0.0, 0.0),
            1e-8,
        );
        for (a, b) in before.iter().zip(pairwise(&s)) {
            assert_vec_near(*a, b, 1e-8);
        }
    }

    #[test]
    fn rebase_preserves_root_positions() {
        let mut s = scene();
        let before = roots(&s);
        let veh_before = s.frames.to_root(s.vehicle).unwrap().translation;
        rebase(
            &mut s.frames,
            &mut s.store,
            s.world,
            DVec3::new(6.4e6, 3.0, -7.0),
        )
        .unwrap();
        for (a, b) in before.iter().zip(roots(&s)) {
            assert_vec_near(*a, b, 1e-8);
        }
        assert_vec_near(
            s.frames.to_root(s.vehicle).unwrap().translation,
            veh_before,
            1e-8,
        );
    }

    #[test]
    fn rebase_repeated_drift_bounded() {
        let mut s = scene();
        let before = roots(&s);
        let o = DVec3::new(1234.5678, -98.765, 4321.0);
        for _ in 0..1000 {
            rebase(&mut s.frames, &mut s.store, s.world, o).unwrap();
        }
        for (a, b) in before.iter().zip(roots(&s)) {
            assert_vec_near(*a, b, 1e-6);
        }
    }

    #[test]
    fn rebase_f32_motivation() {
        let big = std::hint::black_box(6.4e6_f32);
        assert!((big + 0.001_f32) == big);
        let mut s = scene();
        rebase(
            &mut s.frames,
            &mut s.store,
            s.world,
            DVec3::new(6.4e6, 0.0, 0.0),
        )
        .unwrap();
        let p0 = s.store.get(s.ids[0]).unwrap();
        let p1 = s.store.get(s.ids[1]).unwrap();
        let v = s.frames.camera_relative_f32(p1, p0).unwrap();
        assert!((v - Vec3::new(0.001, 0.0, 0.0)).abs().max_element() < 1e-7);
    }

    #[test]
    fn rebase_rejects_root_and_non_finite() {
        let mut s = scene();
        let root = s.frames.root();
        let locals: Vec<_> = s.store.iter().collect();
        let veh_t = s.frames.to_parent(s.vehicle).unwrap();
        assert_eq!(
            rebase(&mut s.frames, &mut s.store, root, DVec3::X),
            Err(MathError::RootFrame)
        );
        assert_eq!(
            rebase(
                &mut s.frames,
                &mut s.store,
                s.world,
                DVec3::new(f64::NAN, 0.0, 0.0)
            ),
            Err(MathError::NonFinite)
        );
        assert_eq!(s.store.iter().collect::<Vec<_>>(), locals);
        assert_eq!(s.frames.to_parent(s.vehicle).unwrap(), veh_t);
    }

    #[test]
    fn rebase_leaves_other_frames_positions() {
        let mut s = scene();
        let rp = WorldPos::from_xyz(s.frames.root(), 5.0, 6.0, 7.0).unwrap();
        let id = s.store.insert(rp);
        rebase(
            &mut s.frames,
            &mut s.store,
            s.world,
            DVec3::new(100.0, 0.0, 0.0),
        )
        .unwrap();
        assert_eq!(s.store.get(id), Some(rp));
    }

    #[test]
    fn rebase_store_basics() {
        let mut st = PositionStore::new();
        let f = FrameRegistry::new().root();
        assert!(st.is_empty());
        let a = st.insert(WorldPos::from_xyz(f, 1.0, 0.0, 0.0).unwrap());
        let b = st.insert(WorldPos::from_xyz(f, 2.0, 0.0, 0.0).unwrap());
        assert_eq!(st.len(), 2);
        st.set(a, WorldPos::from_xyz(f, 3.0, 0.0, 0.0).unwrap())
            .unwrap();
        assert_eq!(st.get(a).unwrap().local().x, 3.0);
        assert!(st.remove(a).is_some());
        assert_eq!(st.get(a), None);
        assert_eq!(
            st.set(a, WorldPos::from_xyz(f, 0.0, 0.0, 0.0).unwrap()),
            Err(MathError::UnknownPosition(a))
        );
        let c = st.insert(WorldPos::from_xyz(f, 4.0, 0.0, 0.0).unwrap());
        assert_ne!(c, a);
        assert_ne!(c, b);
        assert_eq!(st.len(), 2);
    }
}
