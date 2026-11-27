//! F3 toggles the read-only overlay hand-off (headless, no GPU, SDK API only).
#![cfg(feature = "debug-overlay")]

use engine_sdk::{
    App, AppControl, Engine, EngineConfig, InputEvent, KeyCode, NullBackend, Runtime, SceneState,
};
use planetary_app::{AppSettings, PlanetaryApp};

const DT: f64 = 1.0 / 60.0;

fn app() -> PlanetaryApp {
    let mut s = AppSettings::default();
    s.terrain.cells_per_edge = 8;
    s.terrain.lod.max_level = 6;
    s.terrain.settle_each_frame = true;
    PlanetaryApp::new(s).expect("app")
}

fn f3(repeat: bool) -> InputEvent {
    InputEvent::KeyPressed {
        key: KeyCode::F3,
        repeat,
    }
}

#[test]
fn f3_toggles_overlay_and_hides_it_when_off() {
    let mut app = app();
    let mut scene = SceneState::new();
    assert!(!app.overlay_visible());
    assert!(app.overlay_frame().is_none());
    app.input(&f3(false), &mut scene);
    assert!(app.overlay_visible());
    let snap = app.overlay_frame().expect("snapshot when visible");
    assert!(snap.frame_ms.is_finite() && snap.prepare_ms.is_finite());
    // Key repeat does not flip it; a second press hides it again.
    app.input(&f3(true), &mut scene);
    assert!(app.overlay_visible());
    app.input(&f3(false), &mut scene);
    assert!(!app.overlay_visible());
    assert!(app.overlay_frame().is_none());
    // Other keys leave it alone.
    app.input(
        &InputEvent::KeyPressed {
            key: KeyCode::W,
            repeat: false,
        },
        &mut scene,
    );
    assert!(!app.overlay_visible());
}

#[test]
fn overlay_frame_reaches_the_runtime_hook_headless() {
    let mut rt =
        Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).expect("runtime");
    let mut app = app();
    rt.setup(&mut app).expect("setup");
    rt.handle_input(&mut app, &f3(false));
    let mut shown = 0;
    for _ in 0..3 {
        let c = rt
            .frame_with(&mut app, DT, |_, a| {
                shown += usize::from(a.overlay_frame().is_some());
            })
            .expect("frame");
        assert_eq!(c, AppControl::Continue);
    }
    assert_eq!(shown, 3);
}
