//! Geometry-agnostic render scene and camera-relative extraction.

use engine_math::{FrameRegistry, Vec3, WorldPos};
use glam::{Mat3, Mat4, Quat};

use crate::camera::Camera;
use crate::error::RenderError;
use crate::handles::MeshHandle;
use crate::material::{DirectionalLight, Material, MaterialId, MaterialTable};

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
    /// Material (defaults to [`MaterialId::DEFAULT`]).
    pub material: MaterialId,
}

impl RenderItem {
    /// Item with identity rotation and unit scale.
    pub fn new(mesh: MeshHandle, position: WorldPos) -> Self {
        Self {
            mesh,
            position,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            material: MaterialId::DEFAULT,
        }
    }

    /// Set the material.
    #[must_use]
    pub fn with_material(mut self, material: MaterialId) -> Self {
        self.material = material;
        self
    }
}

/// One extracted draw with a camera-relative model matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawItem {
    /// Mesh to draw.
    pub mesh: MeshHandle,
    /// Camera-relative model matrix.
    pub model: Mat4,
    /// Inverse-transpose of the model's upper 3x3 (normal matrix).
    pub normal: Mat3,
    /// Resolved material.
    pub material: Material,
}

/// Per-frame extracted scene (f32 only).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderScene {
    /// Frame index.
    pub frame_index: u64,
    /// Projection times rotation-only view.
    pub view_proj: Mat4,
    /// Directional light in world axes.
    pub light: DirectionalLight,
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
    materials: &MaterialTable,
    light: &DirectionalLight,
) -> Result<RenderScene, RenderError> {
    light.validate()?;
    let view_proj = camera.projection(aspect)? * camera.view();
    let mut draws = Vec::with_capacity(items.len());
    for item in items {
        let rel = frames.camera_relative_f32(item.position, camera.position)?;
        let model = Mat4::from_scale_rotation_translation(item.scale, item.rotation, rel);
        if !model.is_finite() {
            return Err(RenderError::NonFiniteTransform);
        }
        // Light and normals stay in world-axis-aligned camera-relative space; the light is
        // intentionally NOT multiplied by the view matrix.
        let normal = Mat3::from_mat4(model).inverse().transpose();
        if !normal.is_finite() {
            return Err(RenderError::NonFiniteTransform);
        }
        let material = *materials.get(item.material)?;
        draws.push(DrawItem {
            mesh: item.mesh,
            model,
            normal,
            material,
        });
    }
    Ok(RenderScene {
        frame_index,
        view_proj,
        light: *light,
        draws,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::VertexLayout;
    use crate::handles::BufferId;
    use engine_math::{DAffine3, MathError};

    fn mt() -> MaterialTable {
        MaterialTable::new()
    }

    fn lt() -> DirectionalLight {
        DirectionalLight::default()
    }

    fn close(a: Mat3, b: Mat3) -> bool {
        a.abs_diff_eq(b, 1e-6)
    }

    fn mesh(n: u32) -> MeshHandle {
        MeshHandle::new(VertexLayout::PosColor, BufferId::from_raw(n), 3)
    }

    fn pos(frames: &FrameRegistry, x: f64, y: f64, z: f64) -> WorldPos {
        WorldPos::from_xyz(frames.root(), x, y, z).unwrap()
    }

    #[test]
    fn extract_translation_is_camera_relative() {
        let f = FrameRegistry::new();
        let cam = Camera::new(pos(&f, 1e6, 2e6, 3e6));
        let item = RenderItem::new(mesh(1), pos(&f, 1e6 + 1.0, 2e6 + 2.0, 3e6 + 3.0));
        let s = extract(&f, &cam, 1.5, &[item], 0, &mt(), &lt()).unwrap();
        assert_eq!(s.draws[0].model.w_axis.truncate(), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn extract_applies_rotation_and_scale() {
        let f = FrameRegistry::new();
        let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
        let mut item = RenderItem::new(mesh(1), pos(&f, 1.0, 2.0, -3.0));
        item.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        item.scale = Vec3::splat(2.0);
        let s = extract(&f, &cam, 1.0, &[item], 0, &mt(), &lt()).unwrap();
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
        let s = extract(&f, &cam, 2.0, &[], 9, &mt(), &lt()).unwrap();
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
        let r = extract(
            &f,
            &cam,
            1.0,
            &[RenderItem::new(mesh(1), bad)],
            0,
            &mt(),
            &lt(),
        );
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
                extract(&f, &base, a, &[], 0, &mt(), &lt()),
                Err(RenderError::InvalidCamera(_))
            ));
        }
        for fov in [0.0, std::f32::consts::PI, 4.0, f32::NAN] {
            let mut c = base;
            c.fov_y_radians = fov;
            assert!(matches!(
                extract(&f, &c, 1.0, &[], 0, &mt(), &lt()),
                Err(RenderError::InvalidCamera(_))
            ));
        }
        for zn in [0.0, -1.0] {
            let mut c = base;
            c.z_near = zn;
            assert!(matches!(
                extract(&f, &c, 1.0, &[], 0, &mt(), &lt()),
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
        let s = extract(&f, &cam, 1.0, &items, 42, &mt(), &lt()).unwrap();
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

    #[test]
    fn extract_resolves_material() {
        let f = FrameRegistry::new();
        let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
        let mut t = MaterialTable::new();
        let m = Material {
            base_color: [0.2, 0.4, 0.6, 1.0],
            base_color_texture: None,
        };
        let id = t.add(m).unwrap();
        let a = RenderItem::new(mesh(1), pos(&f, 0.0, 0.0, -1.0)).with_material(id);
        let b = RenderItem::new(mesh(1), pos(&f, 0.0, 0.0, -2.0));
        let s = extract(&f, &cam, 1.0, &[a, b], 0, &t, &lt()).unwrap();
        assert_eq!(s.draws[0].material.base_color, [0.2, 0.4, 0.6, 1.0]);
        assert_eq!(s.draws[1].material, Material::default());
    }

    #[test]
    fn extract_unknown_material_errors() {
        let f = FrameRegistry::new();
        let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
        let mut big = MaterialTable::new();
        big.add(Material::default()).unwrap();
        let id = big.add(Material::default()).unwrap();
        let item = RenderItem::new(mesh(1), pos(&f, 0.0, 0.0, -1.0)).with_material(id);
        let r = extract(&f, &cam, 1.0, &[item], 0, &mt(), &lt());
        assert!(matches!(
            r,
            Err(RenderError::InvalidHandle {
                kind: "material",
                ..
            })
        ));
    }

    #[test]
    fn extract_normal_matrix_inverse_transpose() {
        let f = FrameRegistry::new();
        let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
        let mut item = RenderItem::new(mesh(1), pos(&f, 1.0, 2.0, -3.0));
        item.scale = Vec3::new(2.0, 1.0, 1.0);
        item.rotation = Quat::from_rotation_y(0.6);
        let s = extract(&f, &cam, 1.0, &[item], 0, &mt(), &lt()).unwrap();
        let d = &s.draws[0];
        assert!(close(
            d.normal,
            Mat3::from_mat4(d.model).inverse().transpose()
        ));

        let mut u = RenderItem::new(mesh(1), pos(&f, 0.0, 0.0, -3.0));
        u.rotation = Quat::from_rotation_x(0.9);
        let s = extract(&f, &cam, 1.0, &[u], 0, &mt(), &lt()).unwrap();
        assert!(close(s.draws[0].normal, Mat3::from_quat(u.rotation)));
    }

    #[test]
    fn extract_zero_scale_rejected() {
        let f = FrameRegistry::new();
        let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
        let mut item = RenderItem::new(mesh(1), pos(&f, 0.0, 0.0, -3.0));
        item.scale = Vec3::ZERO;
        let r = extract(&f, &cam, 1.0, &[item], 0, &mt(), &lt());
        assert_eq!(r, Err(RenderError::NonFiniteTransform));
    }

    #[test]
    fn extract_carries_light_and_rejects_invalid() {
        let f = FrameRegistry::new();
        let cam = Camera::new(pos(&f, 0.0, 0.0, 0.0));
        let light = DirectionalLight {
            intensity: 3.0,
            ..DirectionalLight::default()
        };
        let s = extract(&f, &cam, 1.0, &[], 0, &mt(), &light).unwrap();
        assert_eq!(s.light, light);
        let bad = DirectionalLight {
            direction: Vec3::new(f32::NAN, 0.0, 0.0),
            ..DirectionalLight::default()
        };
        assert!(matches!(
            extract(&f, &cam, 1.0, &[], 0, &mt(), &bad),
            Err(RenderError::InvalidLight(_))
        ));
    }
}
