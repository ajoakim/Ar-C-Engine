//! The per-frame hook the overlay hand-off rides on (headless, NullBackend, SDK API only).
#![allow(clippy::result_large_err)]

use engine_sdk::prelude::*;
use engine_sdk::{Engine, EngineConfig, NullBackend, Runtime};

struct Idle;

impl App for Idle {
    fn setup(&mut self, _res: &mut Resources<'_>, _scene: &mut SceneState) -> Result<(), SdkError> {
        Ok(())
    }

    fn update(&mut self, _dt: f64, _scene: &mut SceneState) -> AppControl {
        AppControl::Continue
    }

    #[cfg(feature = "egui-overlay")]
    fn overlay_frame(&mut self) -> Option<engine_sdk::DiagnosticsSnapshot> {
        Some(engine_sdk::DiagnosticsSnapshot::default())
    }
}

fn runtime() -> Runtime<NullBackend> {
    Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).expect("runtime")
}

#[test]
fn frame_with_runs_hook_once_per_submitted_frame() {
    let mut rt = runtime();
    let mut calls = 0;
    rt.frame_with(&mut Idle, 0.016, |_, _| calls += 1)
        .expect("frame");
    assert_eq!(calls, 1);
    assert_eq!(rt.backend().frames().len(), 1);
    rt.frame(&mut Idle, 0.016).expect("plain frame");
    assert_eq!(rt.backend().frames().len(), 2);
}

#[cfg(feature = "egui-overlay")]
#[test]
fn overlay_frame_defaults_to_none_and_is_visible_to_the_hook() {
    struct Plain;
    impl App for Plain {
        fn setup(
            &mut self,
            _res: &mut Resources<'_>,
            _scene: &mut SceneState,
        ) -> Result<(), SdkError> {
            Ok(())
        }
        fn update(&mut self, _dt: f64, _scene: &mut SceneState) -> AppControl {
            AppControl::Continue
        }
    }
    assert!(Plain.overlay_frame().is_none());
    let mut rt = runtime();
    let mut seen = false;
    rt.frame_with(&mut Idle, 0.016, |_, app| {
        seen = app.overlay_frame().is_some();
    })
    .expect("frame");
    assert!(seen);
}
