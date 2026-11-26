//! Per-frame GPU hook and mesh destruction tests on NullBackend (SDK API only).
#![allow(clippy::result_large_err)]

use engine_sdk::prelude::*;
use engine_sdk::{Engine, EngineConfig, MeshHandle, NullBackend, Runtime};

fn lit_vertex() -> VertexPosNormalUv {
    VertexPosNormalUv {
        position: [0.0; 3],
        normal: [0.0, 1.0, 0.0],
        uv: [0.0; 2],
    }
}

fn runtime(backend: NullBackend) -> Runtime<NullBackend> {
    Runtime::new(Engine::new(EngineConfig::default()), backend).expect("runtime")
}

#[derive(Default)]
struct Scripted {
    calls: u32,
    mesh: Option<MeshHandle>,
    indexed: bool,
    create_on: u32,
    destroy_on: Option<u32>,
    fail: bool,
    second_destroy: Option<bool>,
    exit: bool,
}

impl App for Scripted {
    fn setup(&mut self, _r: &mut Resources<'_>, _s: &mut SceneState) -> Result<(), SdkError> {
        Ok(())
    }

    fn update(&mut self, _dt: f64, _s: &mut SceneState) -> AppControl {
        if self.exit {
            AppControl::Exit
        } else {
            AppControl::Continue
        }
    }

    fn gpu_frame(&mut self, res: &mut Resources<'_>, _s: &mut SceneState) -> Result<(), SdkError> {
        let n = self.calls;
        self.calls += 1;
        if self.fail {
            return Err(SdkError::App("boom".into()));
        }
        if n == self.create_on && self.mesh.is_none() {
            let m = if self.indexed {
                res.create_lit_mesh("m", &[lit_vertex(); 3], &[0, 1, 2])?
            } else {
                let v = VertexPosColor {
                    position: [0.0; 3],
                    color: [1.0; 3],
                };
                res.create_mesh("m", &[v, v, v])?
            };
            self.mesh = Some(m);
        }
        if Some(n) == self.destroy_on {
            let m = self.mesh.take().expect("mesh");
            res.destroy_mesh(m)?;
            self.second_destroy = Some(res.destroy_mesh(m).is_err());
        }
        Ok(())
    }
}

#[test]
fn default_hook_is_noop() {
    struct Plain;
    impl App for Plain {
        fn setup(&mut self, _: &mut Resources<'_>, _: &mut SceneState) -> Result<(), SdkError> {
            Ok(())
        }
        fn update(&mut self, _: f64, _: &mut SceneState) -> AppControl {
            AppControl::Continue
        }
    }
    let mut rt = runtime(NullBackend::new());
    let mut app = Plain;
    rt.setup(&mut app).unwrap();
    for _ in 0..3 {
        assert_eq!(
            rt.frame(&mut app, 1.0 / 60.0).unwrap(),
            AppControl::Continue
        );
    }
    assert_eq!(rt.backend().buffer_count(), 0);
}

#[test]
fn hook_called_once_per_continue_frame_not_on_exit() {
    let mut rt = runtime(NullBackend::new());
    let mut app = Scripted {
        create_on: u32::MAX,
        ..Scripted::default()
    };
    rt.setup(&mut app).unwrap();
    for _ in 0..4 {
        rt.frame(&mut app, 1.0 / 60.0).unwrap();
    }
    assert_eq!(app.calls, 4);
    app.exit = true;
    assert_eq!(rt.frame(&mut app, 1.0 / 60.0).unwrap(), AppControl::Exit);
    assert_eq!(app.calls, 4);
}

#[test]
fn create_then_destroy_indexed_mesh() {
    let mut rt = runtime(NullBackend::new());
    let mut app = Scripted {
        indexed: true,
        create_on: 0,
        destroy_on: Some(2),
        ..Scripted::default()
    };
    rt.setup(&mut app).unwrap();
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    assert_eq!(rt.backend().buffer_count(), 2);
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    assert_eq!(rt.backend().buffer_count(), 2);
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    assert_eq!(rt.backend().buffer_count(), 0);
}

#[test]
fn create_then_destroy_non_indexed_mesh() {
    let mut rt = runtime(NullBackend::new());
    let mut app = Scripted {
        indexed: false,
        create_on: 0,
        destroy_on: Some(1),
        ..Scripted::default()
    };
    rt.setup(&mut app).unwrap();
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    assert_eq!(rt.backend().buffer_count(), 1);
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    assert_eq!(rt.backend().buffer_count(), 0);
}

#[test]
fn double_destroy_is_error_not_panic() {
    let mut rt = runtime(NullBackend::new());
    let mut app = Scripted {
        indexed: true,
        create_on: 0,
        destroy_on: Some(1),
        ..Scripted::default()
    };
    rt.setup(&mut app).unwrap();
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    assert_eq!(app.second_destroy, Some(true));
}

#[test]
fn gpu_frame_error_propagates() {
    let mut rt = runtime(NullBackend::new());
    let mut app = Scripted {
        fail: true,
        ..Scripted::default()
    };
    rt.setup(&mut app).unwrap();
    let err = rt.frame(&mut app, 1.0 / 60.0).unwrap_err();
    assert!(matches!(err, SdkError::App(ref m) if m == "boom"));
}

#[test]
fn hook_runs_with_zero_size_surface() {
    let mut rt = runtime(NullBackend::with_size(0, 0));
    let mut app = Scripted {
        create_on: u32::MAX,
        ..Scripted::default()
    };
    rt.setup(&mut app).unwrap();
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    assert_eq!(app.calls, 2);
}
