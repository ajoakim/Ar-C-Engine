//! The application-facing trait and scene state.

use engine_math::{FrameRegistry, WorldPos};
use engine_platform::InputEvent;
use engine_render::{
    BufferDesc, BufferUsage, Camera, DirectionalLight, DistanceFog, IndexBufferHandle, Material,
    MaterialId, MaterialTable, MeshHandle, RenderBackend, RenderError, RenderItem, TextureDesc,
    TextureId, VertexLayout, VertexPosColor, VertexPosNormalUv, VertexPosNormalUvMorph,
    encode_indices, encode_lit_morph_vertices, encode_lit_vertices, encode_vertices,
    validate_indices,
};

use crate::error::SdkError;

/// What the app wants the loop to do after an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppControl {
    /// Keep running.
    Continue,
    /// Stop the loop.
    Exit,
}

/// Mutable scene owned by the runtime and edited by the app.
#[derive(Debug, Clone)]
pub struct SceneState {
    /// Coordinate frames.
    pub frames: FrameRegistry,
    /// The camera.
    pub camera: Camera,
    /// Items to draw.
    pub items: Vec<RenderItem>,
    /// Clear color.
    pub clear_color: [f32; 4],
    /// Material table (id 0 is the default material).
    pub materials: MaterialTable,
    /// The directional light.
    pub light: DirectionalLight,
    /// Distance fog (density 0, the default, disables it).
    pub fog: DistanceFog,
}

impl SceneState {
    /// Scene with the camera at the root origin and a dark blue clear color.
    pub fn new() -> Self {
        let frames = FrameRegistry::new();
        let origin = WorldPos::from_xyz(frames.root(), 0.0, 0.0, 0.0)
            .expect("origin is finite and root frame exists");
        Self {
            frames,
            camera: Camera::new(origin),
            items: Vec::new(),
            clear_color: [0.02, 0.02, 0.05, 1.0],
            materials: MaterialTable::new(),
            light: DirectionalLight::default(),
            fog: DistanceFog::default(),
        }
    }

    /// Add a material.
    ///
    /// # Errors
    /// Returns an error when the material has non-finite values or the table is full.
    pub fn add_material(&mut self, material: Material) -> Result<MaterialId, SdkError> {
        Ok(self.materials.add(material)?)
    }
}

impl Default for SceneState {
    fn default() -> Self {
        Self::new()
    }
}

/// GPU resource creation and destruction, available during setup and in [`App::gpu_frame`].
pub struct Resources<'a> {
    pub(crate) backend: &'a mut dyn RenderBackend,
}

impl Resources<'_> {
    /// Upload a vertex mesh.
    ///
    /// # Errors
    /// Returns an error when the mesh is empty, too large or the backend rejects it.
    pub fn create_mesh(
        &mut self,
        label: &'static str,
        vertices: &[VertexPosColor],
    ) -> Result<MeshHandle, SdkError> {
        let Ok(vertex_count) = u32::try_from(vertices.len()) else {
            return Err(SdkError::App("mesh has too many vertices".into()));
        };
        let bytes = encode_vertices(vertices);
        let vertex_buffer = self.backend.create_buffer(
            &BufferDesc {
                label,
                usage: BufferUsage::Vertex,
            },
            &bytes,
        )?;
        Ok(MeshHandle::new(
            VertexLayout::PosColor,
            vertex_buffer,
            vertex_count,
        ))
    }

    /// Upload an indexed lit mesh (position, normal, uv; u32 indices).
    ///
    /// # Errors
    /// Returns an error when the mesh is empty, indices are out of range, or the backend
    /// rejects an upload. No buffer is leaked on failure.
    pub fn create_lit_mesh(
        &mut self,
        label: &'static str,
        vertices: &[VertexPosNormalUv],
        indices: &[u32],
    ) -> Result<MeshHandle, SdkError> {
        let Ok(vertex_count) = u32::try_from(vertices.len()) else {
            return Err(SdkError::App("mesh has too many vertices".into()));
        };
        let Ok(index_count) = u32::try_from(indices.len()) else {
            return Err(SdkError::App("mesh has too many indices".into()));
        };
        if vertices.is_empty() {
            return Err(RenderError::EmptyMesh.into());
        }
        validate_indices(indices, vertex_count)?;
        let vertex_buffer = self.backend.create_buffer(
            &BufferDesc {
                label,
                usage: BufferUsage::Vertex,
            },
            &encode_lit_vertices(vertices),
        )?;
        let index_buffer = match self.backend.create_buffer(
            &BufferDesc {
                label,
                usage: BufferUsage::Index,
            },
            &encode_indices(indices),
        ) {
            Ok(id) => id,
            Err(e) => {
                let _ = self.backend.destroy_buffer(vertex_buffer);
                return Err(e.into());
            }
        };
        Ok(
            MeshHandle::new(VertexLayout::PosNormalUv, vertex_buffer, vertex_count)
                .with_indices(index_buffer, index_count),
        )
    }

    /// Upload a static u32 index buffer that several meshes can draw ranges of.
    ///
    /// # Errors
    /// Returns an error when the list is empty, its length is not a multiple of 3, it is
    /// too large, or the backend rejects the upload.
    pub fn create_index_buffer(
        &mut self,
        label: &'static str,
        indices: &[u32],
    ) -> Result<IndexBufferHandle, SdkError> {
        let Ok(count) = u32::try_from(indices.len()) else {
            return Err(SdkError::App("index buffer has too many indices".into()));
        };
        if indices.is_empty() || !indices.len().is_multiple_of(3) {
            return Err(RenderError::InvalidIndices(
                "index count must be a positive multiple of 3",
            )
            .into());
        }
        let max_index = indices.iter().copied().max().unwrap_or(0);
        let buffer = self.backend.create_buffer(
            &BufferDesc {
                label,
                usage: BufferUsage::Index,
            },
            &encode_indices(indices),
        )?;
        Ok(IndexBufferHandle {
            buffer,
            count,
            max_index,
        })
    }

    /// Upload a lit vertex buffer without indices (draw it with
    /// `MeshHandle::with_index_range` and a shared index buffer).
    ///
    /// # Errors
    /// Returns an error when the list is empty, too large, or the backend rejects it.
    pub fn create_lit_vertices(
        &mut self,
        label: &'static str,
        vertices: &[VertexPosNormalUv],
    ) -> Result<MeshHandle, SdkError> {
        let Ok(vertex_count) = u32::try_from(vertices.len()) else {
            return Err(SdkError::App("mesh has too many vertices".into()));
        };
        if vertices.is_empty() {
            return Err(RenderError::EmptyMesh.into());
        }
        let vertex_buffer = self.backend.create_buffer(
            &BufferDesc {
                label,
                usage: BufferUsage::Vertex,
            },
            &encode_lit_vertices(vertices),
        )?;
        Ok(MeshHandle::new(
            VertexLayout::PosNormalUv,
            vertex_buffer,
            vertex_count,
        ))
    }

    /// Create a mesh of morph vertices (vertex buffer only; attach indices with
    /// [`MeshHandle::with_indices`] or [`MeshHandle::with_index_range`]).
    ///
    /// # Errors
    /// Returns an error when the list is empty, too large, or the backend rejects it.
    pub fn create_lit_morph_vertices(
        &mut self,
        label: &'static str,
        vertices: &[VertexPosNormalUvMorph],
    ) -> Result<MeshHandle, SdkError> {
        let Ok(vertex_count) = u32::try_from(vertices.len()) else {
            return Err(SdkError::App("mesh has too many vertices".into()));
        };
        if vertices.is_empty() {
            return Err(RenderError::EmptyMesh.into());
        }
        let vertex_buffer = self.backend.create_buffer(
            &BufferDesc {
                label,
                usage: BufferUsage::Vertex,
            },
            &encode_lit_morph_vertices(vertices),
        )?;
        Ok(MeshHandle::new(
            VertexLayout::PosNormalUvMorph,
            vertex_buffer,
            vertex_count,
        ))
    }

    /// Destroy a shared index buffer. Meshes still drawing ranges of it become invalid.
    ///
    /// # Errors
    /// Returns an error when the buffer is unknown (e.g. double destroy).
    pub fn destroy_index_buffer(&mut self, ib: IndexBufferHandle) -> Result<(), SdkError> {
        Ok(self.backend.destroy_buffer(ib.buffer)?)
    }

    /// Destroy the vertex buffer and, if present and not shared, the index buffer of `mesh`.
    /// Shared index buffers are owned by the app (see [`Resources::destroy_index_buffer`]).
    /// Both destroys are attempted; the first error is returned.
    ///
    /// # Errors
    /// Returns an error when the backend does not know a buffer (e.g. double destroy).
    pub fn destroy_mesh(&mut self, mesh: MeshHandle) -> Result<(), SdkError> {
        let first = self.backend.destroy_buffer(mesh.vertex_buffer);
        let second = match mesh.index {
            Some(ix) if !ix.shared => self.backend.destroy_buffer(ix.buffer),
            _ => Ok(()),
        };
        first?;
        second?;
        Ok(())
    }

    /// Upload an RGBA8 (sRGB) texture of width*height*4 bytes.
    ///
    /// # Errors
    /// Returns an error when dimensions or data length are invalid.
    pub fn create_texture(
        &mut self,
        label: &'static str,
        width: u32,
        height: u32,
        rgba8: &[u8],
    ) -> Result<TextureId, SdkError> {
        Ok(self.backend.create_texture(
            &TextureDesc {
                label,
                width,
                height,
            },
            rgba8,
        )?)
    }
}

/// An application driven by the SDK runtime.
pub trait App {
    /// Create GPU resources and populate the scene.
    ///
    /// # Errors
    /// Any error aborts startup.
    fn setup(
        &mut self,
        resources: &mut Resources<'_>,
        scene: &mut SceneState,
    ) -> Result<(), SdkError>;

    /// Handle one input event.
    fn input(&mut self, _event: &InputEvent, _scene: &mut SceneState) {}

    /// Advance the app by `dt` seconds.
    fn update(&mut self, dt: f64, scene: &mut SceneState) -> AppControl;

    /// Called once per frame after [`App::update`] (and the engine tick), before extraction,
    /// with GPU resource access. Use it to upload or free resources produced at runtime
    /// (e.g. streamed meshes). Default: no-op.
    ///
    /// # Errors
    /// Any error aborts the frame and is returned from `Runtime::frame`.
    fn gpu_frame(
        &mut self,
        _resources: &mut Resources<'_>,
        _scene: &mut SceneState,
    ) -> Result<(), SdkError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_render::NullBackend;

    fn vtx() -> [VertexPosNormalUv; 3] {
        let v = VertexPosNormalUv {
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0; 2],
        };
        [v, v, v]
    }

    #[test]
    fn lit_mesh_ok() {
        let mut b = NullBackend::new();
        let m = Resources { backend: &mut b }
            .create_lit_mesh("m", &vtx(), &[0, 1, 2])
            .unwrap();
        assert_eq!(m.layout, VertexLayout::PosNormalUv);
        assert_eq!(m.index.unwrap().count, 3);
        assert_eq!(b.buffer_count(), 2);
    }

    #[test]
    fn lit_mesh_bad_index_leaks_nothing() {
        let mut b = NullBackend::new();
        let err = Resources { backend: &mut b }
            .create_lit_mesh("m", &vtx(), &[0, 1, 3])
            .unwrap_err();
        assert!(matches!(
            err,
            SdkError::Render(RenderError::InvalidIndices(_))
        ));
        assert_eq!(b.buffer_count(), 0);
        let err = Resources { backend: &mut b }
            .create_lit_mesh("m", &[], &[])
            .unwrap_err();
        assert!(matches!(err, SdkError::Render(_)));
    }

    #[test]
    fn texture_ok_and_bad_length() {
        let mut b = NullBackend::new();
        let t = Resources { backend: &mut b }
            .create_texture("t", 2, 2, &[255; 16])
            .unwrap();
        assert!(t.raw() >= 3);
        let err = Resources { backend: &mut b }
            .create_texture("t", 2, 2, &[255; 15])
            .unwrap_err();
        assert!(matches!(
            err,
            SdkError::Render(RenderError::InvalidTexture(_))
        ));
    }

    #[test]
    fn scene_materials_and_light() {
        let mut s = SceneState::new();
        assert_eq!(s.light, DirectionalLight::default());
        let id = s.add_material(Material::default()).unwrap();
        assert_eq!(id.raw(), 1);
        let bad = Material {
            base_color: [f32::NAN; 4],
            base_color_texture: None,
        };
        assert!(s.add_material(bad).is_err());
    }
}
