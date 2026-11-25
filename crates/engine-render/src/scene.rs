//! Geometry-agnostic render scene and camera-relative extraction.

use engine_math::{FrameRegistry, Vec3, WorldPos};
use glam::{Mat4, Quat};

use crate::camera::Camera;
use crate::error::RenderError;
use crate::handles::MeshHandle;

/// One renderable item in f64 world space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderItem {
    /// Mesh to draw.
    pub mesh: MeshHandle,
    /// World position.
    pub position: WorldPos,
    /// Rotation.
    pub rotation: Quat,
    /// Scale.
    pub scale: Vec3,
}

impl RenderItem {
    /// Item with identity rotation and unit scale.
    pub fn new(mesh: MeshHandle, position: WorldPos) -> Self {
        Self {
            mesh,
            position,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

/// One extracted draw with a camera-relative model matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawItem {
    /// Mesh to draw.
    pub mesh: MeshHandle,
    /// Camera-relative model matrix.
    pub model: Mat4,
}

/// Per-frame extracted scene (f32 only).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderScene {
    /// Frame index.
    pub frame_index: u64,
    /// Projection times rotation-only view.
    pub view_proj: Mat4,
    /// Draws in input order.
    pub draws: Vec<DrawItem>,
}

/// Extract a camera-relative scene; positions are subtracted in f64 before the f32 cast.
pub fn extract(
    frames: &FrameRegistry,
    camera: &Camera,
    aspect: f32,
    items: &[RenderItem],
    frame_index: u64,
) -> Result<RenderScene, RenderError> {
    let view_proj = camera.projection(aspect)? * camera.view();
    let mut draws = Vec::with_capacity(items.len());
    for item in items {
        let rel = frames.camera_relative_f32(item.position, camera.position)?;
        let model = Mat4::from_scale_rotation_translation(item.scale, item.rotation, rel);
        if !model.is_finite() {
            return Err(RenderError::NonFiniteTransform);
        }
        draws.push(DrawItem {
            mesh: item.mesh,
            model,
        });
    }
    Ok(RenderScene {
        frame_index,
        view_proj,
        draws,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handles::BufferId;
    use engine_math::{DAffine3, MathError};

    fn mesh(n: u32) -> MeshHandle {
        MeshHandle {
            vertex_buffer: BufferId::from_raw(n),
            vertex_count: 3,
        }
    }

    fn pos(frames: &FrameRegistry, x: f64, y: f64, z: f64) -> WorldPos {
        WorldPos::from_xyz(frames.root(), x, y, z).unwrap()
    }

    #[test]
    fn extract_translation_is_camera_relative() {
        let f = FrameRegistry::new();
        let cam = Camera::new(pos(&f, 1e6, 2e6, 3e6));
        let item = RenderItem::new(mesh(1), pos(&f, 1e6 + 1.0, 2e6 + 2.0, 3e6 + 3.0));
        let s = extract(&f, &cam, 1.5, &[item], 0).unwrap();
        assert_eq!(s.draws[0].model.w_axis.truncate(), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn extract_applies_rotation_and_scale() {
        let f = FrameRegistry::new();
        let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
        let mut item = RenderItem::new(mesh(1), pos(&f, 1.0, 2.0, -3.0));
        item.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        item.scale = Vec3::splat(2.0);
        let s = extract(&f, &cam, 1.0, &[item], 0).unwrap();
        let want = Mat4::from_scale_rotation_translation(
            item.scale,
            item.rotation,
            Vec3::new(1.0, 2.0, -3.0),
        );
        assert_eq!(s.draws[0].model, want);
    }

    #[test]
    fn extract_view_proj_is_rotation_only_times_projection() {
        let f = FrameRegistry::new();
        let mut cam = Camera::new(pos(&f, 5e6, 1.0, 2.0));
        cam.orientation = Quat::from_rotation_y(0.7);
        let s = extract(&f, &cam, 2.0, &[], 9).unwrap();
        assert_eq!(s.view_proj, cam.projection(2.0).unwrap() * cam.view());
        assert_eq!(cam.view().w_axis.truncate(), Vec3::ZERO);
    }

    #[test]
    fn extract_rejects_unknown_frame() {
        let f = FrameRegistry::new();
        let mut other = FrameRegistry::new();
        let extra = other
            .add_frame("x", other.root(), DAffine3::IDENTITY)
            .unwrap();
        let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
        let bad = WorldPos::from_xyz(extra, 0.0, 0.0, 0.0).unwrap();
        let r = extract(&f, &cam, 1.0, &[RenderItem::new(mesh(1), bad)], 0);
        assert!(matches!(
            r,
            Err(RenderError::Math(MathError::UnknownFrame(_)))
        ));
    }

    #[test]
    fn extract_rejects_bad_aspect_and_fov() {
        let f = FrameRegistry::new();
        let base = Camera::new(pos(&f, 0.0, 0.0, 0.0));
        for a in [0.0, f32::NAN] {
            assert!(matches!(
                extract(&f, &base, a, &[], 0),
                Err(RenderError::InvalidCamera(_))
            ));
        }
        for fov in [0.0, std::f32::consts::PI, 4.0, f32::NAN] {
            let mut c = base;
            c.fov_y_radians = fov;
            assert!(matches!(
                extract(&f, &c, 1.0, &[], 0),
                Err(RenderError::InvalidCamera(_))
            ));
        }
        for zn in [0.0, -1.0] {
            let mut c = base;
            c.z_near = zn;
            assert!(matches!(
                extract(&f, &c, 1.0, &[], 0),
                Err(RenderError::InvalidCamera(_))
            ));
        }
    }

    #[test]
    fn extract_preserves_item_order_and_frame_index() {
        let f = FrameRegistry::new();
        let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
        let items: Vec<_> = (1..=3)
            .map(|i| RenderItem::new(mesh(i), pos(&f, f64::from(i), 0.0, 0.0)))
            .collect();
        let s = extract(&f, &cam, 1.0, &items, 42).unwrap();
        assert_eq!(s.frame_index, 42);
        assert_eq!(s.draws.len(), 3);
        for (d, i) in s.draws.iter().zip(&items) {
            assert_eq!(d.mesh, i.mesh);
        }
    }

    #[test]
    fn extract_projection_is_reverse_z() {
        let f = FrameRegistry::new();
        let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
        let p = cam.projection(1.0).unwrap();
        let near = p.project_point3(Vec3::new(0.0, 0.0, -cam.z_near));
        assert!((near.z - 1.0).abs() < 1e-6);
        let far = p.project_point3(Vec3::new(0.0, 0.0, -1e6));
        assert!(far.z < 1e-6 && far.z > 0.0);
    }
}
