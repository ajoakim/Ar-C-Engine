//! Backend-generic frame driver shared by `run_app` and headless tests.

use engine_core::{Engine, EnginePhase};
use engine_platform::InputEvent;
use engine_render::{
    CompiledGraph, PipelineDesc, PipelineId, RenderBackend, RenderError, RenderGraph, ShaderId,
    VertexLayout, extract,
};

use crate::app::{App, AppControl, Resources, SceneState};
use crate::error::SdkError;

/// Owns the engine, backend and scene and runs the per-frame path.
///
/// The backend is owned here and is never registered as an engine module.
pub struct Runtime<B: RenderBackend> {
    engine: Engine,
    backend: B,
    scene: SceneState,
    compiled: CompiledGraph,
    pipeline: PipelineId,
    frame_index: u64,
}

impl<B: RenderBackend> Runtime<B> {
    /// Start the engine (if needed) and create the default pipeline and graph.
    ///
    /// # Errors
    /// Fails if the engine cannot reach `Running` or the backend rejects the pipeline.
    pub fn new(mut engine: Engine, mut backend: B) -> Result<Self, SdkError> {
        if engine.phase() == EnginePhase::Initialize {
            engine.initialize()?;
            engine.load()?;
            engine.start()?;
        }
        if engine.phase() != EnginePhase::Running {
            return Err(SdkError::App("engine not running".into()));
        }
        let pipeline = backend.create_pipeline(&PipelineDesc {
            label: "unlit_vertex_color",
            shader: ShaderId::UnlitVertexColor,
            vertex_layout: VertexLayout::PosColor,
        })?;
        let compiled = RenderGraph::main_p0()
            .compile()
            .map_err(RenderError::from)?;
        Ok(Self {
            engine,
            backend,
            scene: SceneState::new(),
            compiled,
            pipeline,
            frame_index: 0,
        })
    }

    /// Run the app's setup.
    ///
    /// # Errors
    /// Propagates the app's error.
    pub fn setup(&mut self, app: &mut dyn App) -> Result<(), SdkError> {
        app.setup(
            &mut Resources {
                backend: &mut self.backend,
            },
            &mut self.scene,
        )
    }

    /// Deliver one input event (resizes also reach the backend).
    pub fn handle_input(&mut self, app: &mut dyn App, event: &InputEvent) {
        if let InputEvent::Resized { width, height } = *event {
            self.backend.resize(width, height);
        }
        app.input(event, &mut self.scene);
    }

    /// One frame: update -> engine tick -> extract -> graph -> submit.
    ///
    /// # Errors
    /// Propagates engine, math and render errors.
    pub fn frame(&mut self, app: &mut dyn App, dt: f64) -> Result<AppControl, SdkError> {
        let control = app.update(dt, &mut self.scene);
        if control == AppControl::Exit {
            return Ok(AppControl::Exit);
        }
        self.engine.tick(dt)?;
        let (w, h) = self.backend.size();
        if w == 0 || h == 0 {
            return Ok(AppControl::Continue);
        }
        let span = engine_diagnostics::tracing::info_span!("render_frame");
        let _guard = span.enter();
        let scene = extract(
            &self.scene.frames,
            &self.scene.camera,
            w as f32 / h as f32,
            &self.scene.items,
            self.frame_index,
        )?;
        let packet =
            self.compiled
                .build_frame_packet(&scene, self.pipeline, self.scene.clear_color)?;
        self.backend.submit(&packet)?;
        self.frame_index += 1;
        Ok(AppControl::Continue)
    }

    /// Shut the engine down.
    ///
    /// # Errors
    /// Propagates lifecycle errors.
    pub fn shutdown(&mut self) -> Result<(), SdkError> {
        self.engine.shutdown()?;
        Ok(())
    }

    /// The backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// The engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// The scene.
    pub fn scene(&self) -> &SceneState {
        &self.scene
    }
}
