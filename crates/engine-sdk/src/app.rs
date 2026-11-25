//! The application-facing trait and scene state.

use engine_math::{FrameRegistry, WorldPos};
use engine_platform::InputEvent;
use engine_render::{
    BufferDesc, BufferUsage, Camera, MeshHandle, RenderBackend, RenderItem, VertexPosColor,
    encode_vertices,
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
        }
    }
}

impl Default for SceneState {
    fn default() -> Self {
        Self::new()
    }
}

/// GPU resource creation available during setup.
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
        Ok(MeshHandle {
            vertex_buffer,
            vertex_count,
        })
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
}
