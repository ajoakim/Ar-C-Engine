//! Coordinate frames, cross-frame conversion and camera-relative output.

use glam::{DAffine3, DVec3, Vec3};

use crate::error::MathError;
use crate::world_pos::WorldPos;

/// Handle to a coordinate frame registered in a [`FrameRegistry`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameId(u32);

#[derive(Clone, Debug)]
struct FrameData {
    name: String,
    parent: Option<FrameId>,
    to_parent: DAffine3,
}

/// Registry of coordinate frames forming a tree rooted at the root frame.
#[derive(Clone, Debug)]
pub struct FrameRegistry {
    frames: Vec<FrameData>,
}

impl Default for FrameRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn affine_is_finite(a: &DAffine3) -> bool {
    a.matrix3.is_finite() && a.translation.is_finite()
}

impl FrameRegistry {
    /// Creates a registry containing only the root frame (`FrameId(0)`).
    pub fn new() -> Self {
        Self {
            frames: vec![FrameData {
                name: "root".to_owned(),
                parent: None,
                to_parent: DAffine3::IDENTITY,
            }],
        }
    }

    /// The root frame.
    pub fn root(&self) -> FrameId {
        FrameId(0)
    }

    fn get(&self, f: FrameId) -> Result<&FrameData, MathError> {
        usize::try_from(f.0)
            .ok()
            .and_then(|i| self.frames.get(i))
            .ok_or(MathError::UnknownFrame(f))
    }

    fn validate_transform(id: FrameId, t: &DAffine3) -> Result<(), MathError> {
        if !affine_is_finite(t) {
            return Err(MathError::NonFinite);
        }
        if t.matrix3.determinant().abs() <= f64::EPSILON {
            return Err(MathError::NonInvertible(id));
        }
        Ok(())
    }

    /// Registers a new frame under `parent` with the given child-to-parent transform.
    pub fn add_frame(
        &mut self,
        name: &str,
        parent: FrameId,
        to_parent: DAffine3,
    ) -> Result<FrameId, MathError> {
        self.get(parent)?;
        let id = FrameId(u32::try_from(self.frames.len()).map_err(|_| MathError::NonFinite)?);
        Self::validate_transform(id, &to_parent)?;
        self.frames.push(FrameData {
            name: name.to_owned(),
            parent: Some(parent),
            to_parent,
        });
        Ok(id)
    }

    /// Name of a frame.
    pub fn name(&self, f: FrameId) -> Result<&str, MathError> {
        Ok(&self.get(f)?.name)
    }

    /// Parent of a frame (`None` for the root).
    pub fn parent(&self, f: FrameId) -> Result<Option<FrameId>, MathError> {
        Ok(self.get(f)?.parent)
    }

    /// Child-to-parent transform; `RootFrame` for the root.
    pub fn to_parent(&self, f: FrameId) -> Result<DAffine3, MathError> {
        let d = self.get(f)?;
        if d.parent.is_none() {
            return Err(MathError::RootFrame);
        }
        Ok(d.to_parent)
    }

    /// Replaces a frame's child-to-parent transform; `RootFrame` for the root.
    pub fn set_to_parent(&mut self, f: FrameId, to_parent: DAffine3) -> Result<(), MathError> {
        if self.get(f)?.parent.is_none() {
            return Err(MathError::RootFrame);
        }
        Self::validate_transform(f, &to_parent)?;
        let idx = usize::try_from(f.0).map_err(|_| MathError::UnknownFrame(f))?;
        let d = self.frames.get_mut(idx).ok_or(MathError::UnknownFrame(f))?;
        d.to_parent = to_parent;
        Ok(())
    }

    /// Frames whose parent is `f`.
    pub fn children(&self, f: FrameId) -> Result<Vec<FrameId>, MathError> {
        self.get(f)?;
        Ok(self
            .frames
            .iter()
            .enumerate()
            .filter(|(_, d)| d.parent == Some(f))
            .filter_map(|(i, _)| u32::try_from(i).ok().map(FrameId))
            .collect())
    }

    /// Transform from frame `f` up to ancestor `ancestor` (inclusive of `f`'s own transform,
    /// exclusive of `ancestor`'s).
    fn up_to(&self, f: FrameId, ancestor: FrameId) -> Result<DAffine3, MathError> {
        let mut result = DAffine3::IDENTITY;
        let mut cur = f;
        while cur != ancestor {
            let d = self.get(cur)?;
            let Some(p) = d.parent else {
                return Err(MathError::UnknownFrame(ancestor));
            };
            result = d.to_parent * result;
            cur = p;
        }
        Ok(result)
    }

    /// Composed transform from frame `f` to the root frame.
    pub fn to_root(&self, f: FrameId) -> Result<DAffine3, MathError> {
        self.up_to(f, self.root())
    }

    fn lowest_common_ancestor(&self, a: FrameId, b: FrameId) -> Result<FrameId, MathError> {
        let mut ancestors_of_b = Vec::new();
        let mut cur = Some(b);
        while let Some(c) = cur {
            ancestors_of_b.push(c);
            cur = self.get(c)?.parent;
        }
        let mut cur = Some(a);
        while let Some(c) = cur {
            if ancestors_of_b.contains(&c) {
                return Ok(c);
            }
            cur = self.get(c)?.parent;
        }
        Err(MathError::UnknownFrame(a))
    }

    /// Converts a position into frame `to`, going through the lowest common ancestor.
    pub fn convert(&self, pos: WorldPos, to: FrameId) -> Result<WorldPos, MathError> {
        self.get(pos.frame())?;
        self.get(to)?;
        if pos.frame() == to {
            return Ok(pos);
        }
        let lca = self.lowest_common_ancestor(pos.frame(), to)?;
        let up = self.up_to(pos.frame(), lca)?;
        let down = self.up_to(to, lca)?;
        let local = down
            .inverse()
            .transform_point3(up.transform_point3(pos.local()));
        WorldPos::new(to, local)
    }

    /// `to - from`, expressed in `from.frame()`.
    pub fn delta(&self, from: WorldPos, to: WorldPos) -> Result<DVec3, MathError> {
        let converted = self.convert(to, from.frame())?;
        Ok(converted.local() - from.local())
    }

    /// Offset of `pos` from `camera`, subtracted in f64 and then cast to f32.
    pub fn camera_relative_f32(&self, pos: WorldPos, camera: WorldPos) -> Result<Vec3, MathError> {
        Ok(self.delta(camera, pos)?.as_vec3())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DQuat;

    fn near(a: DVec3, b: DVec3, tol: f64) -> bool {
        (a - b).abs().max_element() <= tol
    }

    fn setup() -> (FrameRegistry, FrameId, FrameId) {
        let mut r = FrameRegistry::new();
        let a = r
            .add_frame(
                "A",
                r.root(),
                DAffine3::from_translation(DVec3::new(6.4e6, 0.0, 0.0)),
            )
            .unwrap();
        let t = DAffine3::from_rotation_translation(
            DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2),
            DVec3::new(10.0, 0.0, -5.0),
        );
        let b = r.add_frame("B", a, t).unwrap();
        (r, a, b)
    }

    #[test]
    fn frame_root_exists() {
        let r = FrameRegistry::new();
        let root = r.root();
        assert_eq!(r.name(root).unwrap(), "root");
        assert_eq!(r.parent(root).unwrap(), None);
        assert_eq!(r.to_parent(root), Err(MathError::RootFrame));
        let mut r2 = r.clone();
        assert_eq!(
            r2.set_to_parent(root, DAffine3::IDENTITY),
            Err(MathError::RootFrame)
        );
    }

    #[test]
    fn frame_add_unknown_parent() {
        let mut r = FrameRegistry::new();
        let bogus = FrameId(42);
        assert_eq!(
            r.add_frame("x", bogus, DAffine3::IDENTITY),
            Err(MathError::UnknownFrame(bogus))
        );
    }

    #[test]
    fn frame_add_non_finite_and_singular() {
        let mut r = FrameRegistry::new();
        let root = r.root();
        assert_eq!(
            r.add_frame(
                "n",
                root,
                DAffine3::from_translation(DVec3::new(f64::NAN, 0.0, 0.0))
            ),
            Err(MathError::NonFinite)
        );
        assert!(matches!(
            r.add_frame("s", root, DAffine3::from_scale(DVec3::ZERO)),
            Err(MathError::NonInvertible(_))
        ));
    }

    #[test]
    fn frame_convert_round_trip() {
        let (r, _a, b) = setup();
        let p = WorldPos::from_xyz(b, 1.25, -3.5, 7.75).unwrap();
        let in_root = r.convert(p, r.root()).unwrap();
        let back = r.convert(in_root, b).unwrap();
        assert!(near(back.local(), p.local(), 1e-9));
        assert_eq!(back.frame(), b);
    }

    #[test]
    fn frame_convert_sibling() {
        let (mut r, a, b) = setup();
        let c = r
            .add_frame(
                "C",
                a,
                DAffine3::from_translation(DVec3::new(1.0, 2.0, 3.0)),
            )
            .unwrap();
        let p = WorldPos::from_xyz(b, 0.5, 0.25, -2.0).unwrap();
        let got = r.convert(p, c).unwrap();
        let tb = r.to_parent(b).unwrap();
        let tc = r.to_parent(c).unwrap();
        let manual = tc
            .inverse()
            .transform_point3(tb.transform_point3(p.local()));
        assert!(near(got.local(), manual, 1e-9));
    }

    #[test]
    fn frame_delta_cross_frame() {
        let (r, a, b) = setup();
        let from = WorldPos::from_xyz(a, 100.0, 2.0, 3.0).unwrap();
        let to = WorldPos::from_xyz(b, 1.0, 2.0, 3.0).unwrap();
        let d = r.delta(from, to).unwrap();
        let expect = r.convert(to, a).unwrap().local() - from.local();
        assert!(near(d, expect, 1e-12));
    }

    #[test]
    fn frame_camera_relative_f32() {
        let r = FrameRegistry::new();
        let cam = WorldPos::from_xyz(r.root(), 6.4e6, 0.0, 0.0).unwrap();
        let p = WorldPos::from_xyz(r.root(), 6.4e6 + 0.25, 1.0, -0.5).unwrap();
        let v = r.camera_relative_f32(p, cam).unwrap();
        assert!((v - Vec3::new(0.25, 1.0, -0.5)).abs().max_element() < 1e-6);
    }

    #[test]
    fn frame_worldpos_rejects_non_finite() {
        let r = FrameRegistry::new();
        let root = r.root();
        assert_eq!(
            WorldPos::from_xyz(root, f64::NAN, 0.0, 0.0),
            Err(MathError::NonFinite)
        );
        assert_eq!(
            WorldPos::from_xyz(root, 0.0, f64::INFINITY, 0.0),
            Err(MathError::NonFinite)
        );
        let p = WorldPos::from_xyz(root, 0.0, 0.0, 0.0).unwrap();
        assert_eq!(
            p.translated(DVec3::new(f64::NAN, 0.0, 0.0)),
            Err(MathError::NonFinite)
        );
    }

    #[test]
    fn frame_unknown_frame_errors() {
        let (big, _a, b) = setup();
        let small = FrameRegistry::new();
        let p = WorldPos::from_xyz(b, 1.0, 1.0, 1.0).unwrap();
        assert_eq!(
            small.convert(p, small.root()),
            Err(MathError::UnknownFrame(b))
        );
        let q = WorldPos::from_xyz(small.root(), 1.0, 1.0, 1.0).unwrap();
        assert_eq!(small.convert(q, b), Err(MathError::UnknownFrame(b)));
        assert!(big.convert(q, b).is_ok());
    }
}
