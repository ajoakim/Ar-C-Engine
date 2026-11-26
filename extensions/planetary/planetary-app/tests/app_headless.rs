//! Headless smoke tests of the planetary app on the SDK runtime (no wall-clock time).

use engine_sdk::{
    AppControl, DVec3, Engine, EngineConfig, InputEvent, KeyCode, NullBackend, Runtime,
};
use planetary_app::{AppSettings, DescentScript, PlanetaryApp};

const DT: f64 = 1.0 / 60.0;

fn settings(scripted: bool) -> AppSettings {
    let mut s = AppSettings::default();
    s.terrain.cells_per_edge = 8;
    s.terrain.lod.max_level = 10;
    s.terrain.settle_each_frame = true;
    if scripted {
        s.script = Some(DescentScript::orbit_to_surface(s.terrain.terrain.radius));
    }
    s
}

fn runtime() -> Runtime<NullBackend> {
    Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).expect("runtime")
}

fn draws(rt: &Runtime<NullBackend>) -> usize {
    rt.backend()
        .last_frame()
        .map_or(0, |f| f.passes.iter().map(|p| p.draws.len()).sum())
}

fn altitude(app: &PlanetaryApp) -> f64 {
    app.camera()
        .altitude(&|d| app.terrain().surface_radius_at(d))
}

#[test]
fn scripted_descent_draws_terrain() {
    let mut app = PlanetaryApp::new(settings(true)).expect("app");
    let mut rt = runtime();
    rt.setup(&mut app).expect("setup");
    for _ in 0..60 {
        rt.frame(&mut app, DT).expect("frame");
    }
    assert!(draws(&rt) > 0);
    assert!(app.report().drawn_leaves > 0);
    app.shutdown();
}

#[test]
fn scripted_descent_loses_altitude() {
    let mut app = PlanetaryApp::new(settings(true)).expect("app");
    let mut rt = runtime();
    rt.setup(&mut app).expect("setup");
    rt.frame(&mut app, DT).expect("frame");
    let first = altitude(&app);
    for _ in 1..300 {
        rt.frame(&mut app, DT).expect("frame");
    }
    let later = altitude(&app);
    assert!(later < first, "{later} !< {first}");
    assert!(app.simulated_time() > 4.9);
    app.shutdown();
}

#[test]
fn resize_is_accepted() {
    let mut app = PlanetaryApp::new(settings(true)).expect("app");
    let mut rt = runtime();
    rt.setup(&mut app).expect("setup");
    rt.handle_input(
        &mut app,
        &InputEvent::Resized {
            width: 800,
            height: 600,
        },
    );
    assert_eq!(rt.frame(&mut app, DT).expect("frame"), AppControl::Continue);
    app.shutdown();
}

#[test]
fn escape_exits_unscripted_app() {
    let mut app = PlanetaryApp::new(settings(false)).expect("app");
    let mut rt = runtime();
    rt.setup(&mut app).expect("setup");
    assert_eq!(rt.frame(&mut app, DT).expect("frame"), AppControl::Continue);
    rt.handle_input(
        &mut app,
        &InputEvent::KeyPressed {
            key: KeyCode::Escape,
            repeat: false,
        },
    );
    assert_eq!(rt.frame(&mut app, DT).expect("frame"), AppControl::Exit);
    app.shutdown();
}

#[test]
fn holding_w_moves_the_camera() {
    let mut app = PlanetaryApp::new(settings(false)).expect("app");
    let mut rt = runtime();
    rt.setup(&mut app).expect("setup");
    let before: DVec3 = app.camera().position();
    rt.handle_input(
        &mut app,
        &InputEvent::KeyPressed {
            key: KeyCode::W,
            repeat: false,
        },
    );
    for _ in 0..60 {
        rt.frame(&mut app, DT).expect("frame");
    }
    assert_ne!(app.camera().position(), before);
    app.shutdown();
}
