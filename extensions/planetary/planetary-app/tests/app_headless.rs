//! Headless smoke tests of the planetary app on the SDK runtime (no wall-clock time).

use engine_sdk::{
    AppControl, DVec3, Engine, EngineConfig, InputEvent, KeyCode, NullBackend, Runtime,
};
use planetary_app::{
    AppSettings, DescentScript, PlanetaryApp, TopologyKind, haze_for_altitude,
    sun_direction_for_target,
};
use planetary_core::TerrainParams;

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

fn frame_sets_light_fog_and_clear_colour(kind: TopologyKind) {
    let mut app = PlanetaryApp::new(settings(kind, true)).expect("app");
    let mut rt = runtime();
    rt.setup(&mut app).expect("setup");
    for _ in 0..3 {
        rt.frame(&mut app, DT).expect("frame");
    }
    let alt = altitude(&app);
    let want = haze_for_altitude(alt);
    let scene = rt.scene();
    assert_eq!(scene.fog, want.fog);
    assert_eq!(scene.clear_color, want.clear_color);
    let radius = TerrainParams::default().radius;
    let target = DescentScript::orbit_to_surface(radius);
    assert_eq!(
        scene.light.direction,
        sun_direction_for_target(
            target
                .pose_at(target.descent_seconds, &|_| radius)
                .0
                .normalize()
        )
    );
    assert!(scene.light.validate().is_ok());
    assert!(scene.fog.validate().is_ok());
    app.shutdown();
}

fn haze_off_keeps_default_fog(kind: TopologyKind) {
    let mut s = settings(kind, true);
    s.haze = false;
    let mut app = PlanetaryApp::new(s).expect("app");
    let mut rt = runtime();
    rt.setup(&mut app).expect("setup");
    for _ in 0..3 {
        rt.frame(&mut app, DT).expect("frame");
    }
    assert_eq!(rt.scene().fog, engine_sdk::DistanceFog::default());
    assert_eq!(
        rt.scene().clear_color,
        engine_sdk::SceneState::new().clear_color
    );
    assert!(rt.scene().light.validate().is_ok());
    app.shutdown();
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
    holding_w_moves_the_camera,
    frame_sets_light_fog_and_clear_colour,
    haze_off_keeps_default_fog
);

#[test]
fn window_title_is_set_only_on_throttle_frames() {
    let mut app = PlanetaryApp::new(settings(TopologyKind::Hex, true)).expect("app");
    let mut rt = runtime();
    rt.setup(&mut app).expect("setup");
    let mut titled = Vec::new();
    for i in 0..31_u64 {
        rt.frame(&mut app, DT).expect("frame");
        if let Some(t) = rt.take_window_title() {
            assert!(t.starts_with("alt "), "{t}");
            assert!(t.contains(" | hex L"), "{t}");
            titled.push(i);
        }
    }
    assert_eq!(titled, vec![0, 15, 30]);
}
