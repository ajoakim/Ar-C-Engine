//! Headless SDK frame tests on NullBackend (SDK API only).
#![allow(clippy::result_large_err)]

use engine_sdk::prelude::*;
use engine_sdk::{
    Engine, EngineConfig, EnginePhase, MeshHandle, NullBackend, RenderBackend, RenderError, Runtime,
};

#[derive(Default)]
struct TriangleApp {
    mesh: Option<MeshHandle>,
    inputs: Vec<InputEvent>,
    exit: bool,
}

impl App for TriangleApp {
    fn setup(&mut self, res: &mut Resources<'_>, scene: &mut SceneState) -> Result<(), SdkError> {
        let v = |p: [f32; 3], c: [f32; 3]| VertexPosColor {
            position: p,
            color: c,
        };
        let mesh = res.create_mesh(
            "tri",
            &[
                v([-0.5, -0.4, 0.0], [1.0, 0.0, 0.0]),
                v([0.5, -0.4, 0.0], [0.0, 1.0, 0.0]),
                v([0.0, 0.5, 0.0], [0.0, 0.0, 1.0]),
            ],
        )?;
        self.mesh = Some(mesh);
        let root = scene.frames.root();
        scene.camera = Camera::new(WorldPos::from_xyz(root, 0.0, 6.4e6, 2.0)?);
        scene.items.push(RenderItem::new(
            mesh,
            WorldPos::from_xyz(root, 0.0, 6.4e6, 0.0)?,
        ));
        Ok(())
    }

    fn input(&mut self, event: &InputEvent, _scene: &mut SceneState) {
        self.inputs.push(*event);
    }

    fn update(&mut self, _dt: f64, _scene: &mut SceneState) -> AppControl {
        if self.exit {
            AppControl::Exit
        } else {
            AppControl::Continue
        }
    }
}

fn runtime() -> Runtime<NullBackend> {
    Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).unwrap()
}

#[test]
fn headless_frame_renders_triangle_at_planetary_distance() {
    let mut rt = runtime();
    let mut app = TriangleApp::default();
    rt.setup(&mut app).unwrap();
    let mut last_index = None;
    for _ in 0..3 {
        assert_eq!(rt.frame(&mut app, 0.016).unwrap(), AppControl::Continue);
        let idx = rt.engine().last_frame_stats().unwrap().frame_index;
        if let Some(prev) = last_index {
            assert!(idx > prev);
        }
        last_index = Some(idx);
    }
    let frames = rt.backend().frames();
    assert_eq!(frames.len(), 3);
    for f in frames {
        assert_eq!(f.passes.len(), 1);
        assert_eq!(f.passes[0].name, "main");
        assert_eq!(f.passes[0].draws.len(), 1);
        assert!(f.passes[0].draws[0].mvp.is_finite());
    }
}

#[test]
fn headless_resize_input_reaches_backend() {
    let mut rt = runtime();
    let mut app = TriangleApp::default();
    let ev = InputEvent::Resized {
        width: 800,
        height: 600,
    };
    rt.handle_input(&mut app, &ev);
    assert_eq!(rt.backend().size(), (800, 600));
    assert_eq!(app.inputs, vec![ev]);
}

#[test]
fn headless_zero_size_skips_extract() {
    let mut rt = runtime();
    let mut app = TriangleApp::default();
    rt.setup(&mut app).unwrap();
    rt.handle_input(
        &mut app,
        &InputEvent::Resized {
            width: 0,
            height: 0,
        },
    );
    assert_eq!(rt.frame(&mut app, 0.016).unwrap(), AppControl::Continue);
    assert_eq!(rt.backend().frames().len(), 0);
}

#[test]
fn headless_exit_control_propagates() {
    let mut rt = runtime();
    let mut app = TriangleApp {
        exit: true,
        ..Default::default()
    };
    rt.setup(&mut app).unwrap();
    assert_eq!(rt.frame(&mut app, 0.016).unwrap(), AppControl::Exit);
    assert_eq!(rt.backend().frames().len(), 0);
}

#[test]
fn headless_runtime_starts_engine() {
    let mut rt = runtime();
    assert_eq!(rt.engine().phase(), EnginePhase::Running);
    rt.shutdown().unwrap();
    assert_eq!(rt.engine().phase(), EnginePhase::Shutdown);
}

#[test]
fn headless_invalid_mesh_is_error() {
    struct Bad;
    impl App for Bad {
        fn setup(&mut self, r: &mut Resources<'_>, _s: &mut SceneState) -> Result<(), SdkError> {
            r.create_mesh("empty", &[]).map(|_| ())
        }
        fn update(&mut self, _dt: f64, _s: &mut SceneState) -> AppControl {
            AppControl::Continue
        }
    }
    let mut rt = runtime();
    let err = rt.setup(&mut Bad).unwrap_err();
    assert!(matches!(err, SdkError::Render(RenderError::EmptyBuffer)));
}

#[test]
fn startup_info_reaches_setup_before_the_first_frame() {
    use engine_sdk::{
        App, AppControl, Engine, EngineConfig, InputEvent, NullBackend, Resources, Runtime,
        SceneState, SdkError, StartupInfo,
    };

    struct Probe(Option<StartupInfo>);
    impl App for Probe {
        fn setup(
            &mut self,
            _r: &mut Resources<'_>,
            scene: &mut SceneState,
        ) -> Result<(), SdkError> {
            self.0 = Some(scene.startup.clone());
            Ok(())
        }
        fn input(&mut self, _e: &InputEvent, _s: &mut SceneState) {}
        fn update(&mut self, _dt: f64, _s: &mut SceneState) -> AppControl {
            AppControl::Continue
        }
    }

    let info = StartupInfo {
        adapter: "test (null)".into(),
        present: None,
        monitor_refresh_mhz: Some(59_940),
    };
    let mut rt = Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).unwrap();
    rt.set_startup_info(info.clone());
    assert_eq!(rt.scene().startup, info);
    let mut app = Probe(None);
    rt.setup(&mut app).unwrap();
    assert_eq!(app.0, Some(info));
    assert_eq!(rt.take_window_title(), None);
}
