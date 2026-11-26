//! Headless smoke tests of the planetary app on the SDK runtime (no wall-clock time).

use engine_sdk::{
    AppControl, DVec3, Engine, EngineConfig, InputEvent, KeyCode, NullBackend, Runtime,
};
use planetary_app::{AppSettings, DescentScript, PlanetaryApp, TopologyKind};

const DT: f64 = 1.0 / 60.0;

fn settings(kind: TopologyKind, scripted: bool) -> AppSettings {
    let mut s = AppSettings {
        topology: kind,
        ..AppSettings::default()
    };
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
    app.camera().altitude(&|d| app.surface_radius_at(d))
}

fn scripted_descent_draws_terrain(kind: TopologyKind) {
    let mut app = PlanetaryApp::new(settings(kind, true)).expect("app");
    let mut rt = runtime();
    rt.setup(&mut app).expect("setup");
    for _ in 0..60 {
        rt.frame(&mut app, DT).expect("frame");
    }
    assert!(draws(&rt) > 0);
    assert!(app.report().drawn_leaves > 0);
    app.shutdown();
}

fn scripted_descent_loses_altitude(kind: TopologyKind) {
    let mut app = PlanetaryApp::new(settings(kind, true)).expect("app");
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

fn resize_is_accepted(kind: TopologyKind) {
    let mut app = PlanetaryApp::new(settings(kind, true)).expect("app");
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

fn escape_exits_unscripted_app(kind: TopologyKind) {
    let mut app = PlanetaryApp::new(settings(kind, false)).expect("app");
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

fn holding_w_moves_the_camera(kind: TopologyKind) {
    let mut app = PlanetaryApp::new(settings(kind, false)).expect("app");
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

#[test]
fn invalid_hex_cells_per_edge_is_an_error() {
    for n in [0, 1, 3, 66] {
        let mut s = AppSettings::default();
        s.terrain.cells_per_edge = n;
        assert!(PlanetaryApp::new(s).is_err(), "n = {n}");
    }
}

macro_rules! both {
    ($($name:ident),*) => {
        mod hex {
            use super::*;
            $(#[test] fn $name() { super::$name(TopologyKind::Hex); })*
        }
        mod cube {
            use super::*;
            $(#[test] fn $name() { super::$name(TopologyKind::Cube); })*
        }
    };
}

both!(
    scripted_descent_draws_terrain,
    scripted_descent_loses_altitude,
    resize_is_accepted,
    escape_exits_unscripted_app,
    holding_w_moves_the_camera
);
