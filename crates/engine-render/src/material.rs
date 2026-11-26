//! Backend-agnostic material, light and texture description types (no GPU types).

use crate::error::RenderError;
use crate::handles::TextureId;

/// Maximum width or height of a sampled texture.
pub const MAX_TEXTURE_DIMENSION: u32 = 8192;
/// Maximum number of materials in a table.
pub const MAX_MATERIALS: usize = 65_536;

/// Handle to a material in a [`MaterialTable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialId(u32);

impl MaterialId {
    /// The default (white, untextured) material.
    pub const DEFAULT: MaterialId = MaterialId(0);

    /// Raw numeric value.
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// A surface material (glTF base-color semantics).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// Linear RGBA factor.
    pub base_color: [f32; 4],
    /// Optional sampled base color texture.
    pub base_color_texture: Option<TextureId>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: [1.0; 4],
            base_color_texture: None,
        }
    }
}

/// Append-only material storage; ids are never reused.
#[derive(Debug, Clone)]
pub struct MaterialTable {
    materials: Vec<Material>,
}

impl MaterialTable {
    /// Table containing only the default material.
    pub fn new() -> Self {
        Self {
            materials: vec![Material::default()],
        }
    }

    /// Add a material.
    pub fn add(&mut self, m: Material) -> Result<MaterialId, RenderError> {
        if !m.base_color.iter().all(|c| c.is_finite()) {
            return Err(RenderError::NonFiniteTransform);
        }
        if let Some(t) = m.base_color_texture
            && t.raw() < TextureId::FIRST_SAMPLED_RAW
        {
            return Err(RenderError::InvalidHandle {
                kind: "texture",
                id: t.raw(),
            });
        }
        if self.materials.len() >= MAX_MATERIALS {
            return Err(RenderError::HandleExhausted);
        }
        let id = MaterialId(self.materials.len() as u32);
        self.materials.push(m);
        Ok(id)
    }

    /// Look up a material.
    pub fn get(&self, id: MaterialId) -> Result<&Material, RenderError> {
        self.materials
            .get(id.0 as usize)
            .ok_or(RenderError::InvalidHandle {
                kind: "material",
                id: id.0,
            })
    }

    /// Number of materials (including the default).
    pub fn len(&self) -> usize {
        self.materials.len()
    }

    /// Always false: the default material always exists.
    pub fn is_empty(&self) -> bool {
        false
    }
}

impl Default for MaterialTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Directional light. `direction` is the unit vector the light travels (light -> scene).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectionalLight {
    /// Travel direction in world axes.
    pub direction: glam::Vec3,
    /// Linear RGB color.
    pub color: [f32; 3],
    /// Intensity multiplier (>= 0).
    pub intensity: f32,
    /// Linear RGB ambient term.
    pub ambient: [f32; 3],
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            direction: glam::Vec3::new(-0.4, -1.0, -0.3).normalize(),
            color: [1.0; 3],
            intensity: 1.0,
            ambient: [0.1; 3],
        }
    }
}

impl DirectionalLight {
    /// Check all values are finite and in range.
    pub fn validate(&self) -> Result<(), RenderError> {
        if !self.direction.is_finite()
            || !self.color.iter().all(|c| c.is_finite())
            || !self.ambient.iter().all(|c| c.is_finite())
            || !self.intensity.is_finite()
        {
            return Err(RenderError::InvalidLight("non-finite light value"));
        }
        if self.direction.length() <= 1e-6 {
            return Err(RenderError::InvalidLight("zero light direction"));
        }
        if self.intensity < 0.0 {
            return Err(RenderError::InvalidLight("negative intensity"));
        }
        if self.color.iter().any(|c| *c < 0.0) || self.ambient.iter().any(|c| *c < 0.0) {
            return Err(RenderError::InvalidLight("negative color component"));
        }
        Ok(())
    }

    /// CPU reference Lambert term: `max(dot(n, -direction), 0)`.
    pub fn lambert(&self, normal: glam::Vec3) -> f32 {
        normal.normalize().dot(-self.direction.normalize()).max(0.0)
    }
}

/// Sampled texture description; data is RGBA8 sRGB, `width*height*4` bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureDesc {
    /// Debug label.
    pub label: &'static str,
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_table_default_is_white() {
        let t = MaterialTable::new();
        assert_eq!(
            *t.get(MaterialId::DEFAULT).unwrap(),
            Material {
                base_color: [1.0; 4],
                base_color_texture: None
            }
        );
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn material_table_ids_sequential() {
        let mut t = MaterialTable::new();
        assert_eq!(t.add(Material::default()).unwrap().raw(), 1);
        assert_eq!(t.add(Material::default()).unwrap().raw(), 2);
    }

    #[test]
    fn material_table_rejects_non_finite() {
        let mut t = MaterialTable::new();
        let m = Material {
            base_color: [f32::NAN, 1.0, 1.0, 1.0],
            base_color_texture: None,
        };
        assert_eq!(t.add(m), Err(RenderError::NonFiniteTransform));
    }

    #[test]
    fn material_table_rejects_surface_texture() {
        let mut t = MaterialTable::new();
        let m = Material {
            base_color: [1.0; 4],
            base_color_texture: Some(TextureId::BACKBUFFER),
        };
        assert_eq!(
            t.add(m),
            Err(RenderError::InvalidHandle {
                kind: "texture",
                id: 1
            })
        );
    }

    #[test]
    fn material_table_unknown_id() {
        let mut a = MaterialTable::new();
        a.add(Material::default()).unwrap();
        let id = a.add(Material::default()).unwrap();
        assert_eq!(id.raw(), 2);
        let b = MaterialTable::new();
        assert_eq!(
            b.get(id),
            Err(RenderError::InvalidHandle {
                kind: "material",
                id: 2
            })
        );
    }

    #[test]
    fn light_default_validates_and_lambert() {
        let l = DirectionalLight::default();
        assert!(l.validate().is_ok());
        assert!((l.lambert(-l.direction) - 1.0).abs() < 1e-5);
        assert_eq!(l.lambert(l.direction), 0.0);
    }

    #[test]
    fn light_rejects_bad() {
        let l = DirectionalLight {
            direction: glam::Vec3::ZERO,
            ..DirectionalLight::default()
        };
        assert!(matches!(l.validate(), Err(RenderError::InvalidLight(_))));
        let l = DirectionalLight {
            color: [f32::NAN, 1.0, 1.0],
            ..DirectionalLight::default()
        };
        assert!(matches!(l.validate(), Err(RenderError::InvalidLight(_))));
        let l = DirectionalLight {
            intensity: -1.0,
            ..DirectionalLight::default()
        };
        assert!(matches!(l.validate(), Err(RenderError::InvalidLight(_))));
    }
}
