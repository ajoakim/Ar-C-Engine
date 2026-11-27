//! App-level 6DOF free-flight tests (headless, SDK API only).

use engine_sdk::{
    DVec3, Engine, EngineConfig, InputEvent, KeyCode, MouseButton, NullBackend, Runtime,
};
use planetary_app::{AppSettings, DEFAULT_ROLL_RATE, DescentScript, FlightMode, PlanetaryApp};

const DT: f64 = 1.0 / 60.0;

fn settings() -> AppSettings {
    let mut s = AppSettings::default();
    s.terrain.cells_per_edge = 8;
    s.terrain.lod.max_level = 6;
    s.terrain.settle_each_frame = true;
    s
}

fn setup(s: AppSettings) -> (PlanetaryApp, Runtime<NullBackend>) {
    let mut rt =
        Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).expect("runtime");
    let mut app = PlanetaryApp::new(s).expect("app");
    rt.setup(&mut app).expect("setup");
    (app, rt)
}

fn press(rt: &mut Runtime<NullBackend>, app: &mut PlanetaryApp, key: KeyCode, repeat: bool) {
    rt.handle_input(app, &InputEvent::KeyPressed { key, repeat });
}

fn release(rt: &mut Runtime<NullBackend>, app: &mut PlanetaryApp, key: KeyCode) {
    rt.handle_input(app, &InputEvent::KeyReleased { key });
}

fn frames(rt: &mut Runtime<NullBackend>, app: &mut PlanetaryApp, n: usize) {
    for _ in 0..n {
        rt.frame(app, DT).expect("frame");
    }
}

fn up(app: &PlanetaryApp) -> DVec3 {
    app.camera().orientation() * DVec3::Y
}

fn right(app: &PlanetaryApp) -> DVec3 {
    app.camera().orientation() * DVec3::X
}

/// Signed angle from `a` to `b` about `axis`.
fn signed_angle(a: DVec3, b: DVec3, axis: DVec3) -> f64 {
    axis.dot(a.cross(b)).atan2(a.dot(b))
}

#[test]
fn free_is_the_default_mode() {
    let (app, _rt) = setup(settings());
    assert_eq!(app.flight_mode(), FlightMode::Free);
}

#[test]
fn z_and_x_roll_about_the_view_axis() {
    let n = 30;
    let expected = DEFAULT_ROLL_RATE * n as f64 * DT;
    let mut signs = Vec::new();
    for key in [KeyCode::Z, KeyCode::X] {
        let (mut app, mut rt) = setup(settings());
        frames(&mut rt, &mut app, 1);
        let (f0, u0) = (app.camera().forward(), up(&app));
        press(&mut rt, &mut app, key, false);
        frames(&mut rt, &mut app, n);
        let ang = signed_angle(u0, up(&app), f0);
        assert!(
            (ang.abs() - expected).abs() < 1e-6,
            "{key:?}: {ang} vs {expected}"
        );
        assert!((app.camera().forward() - f0).length() < 1e-9);
        signs.push(ang.signum());
    }
    assert!(signs[0] * signs[1] < 0.0, "Z and X must roll opposite ways");
}

#[test]
fn a_and_d_strafe_along_camera_right() {
    for (key, sign) in [(KeyCode::D, 1.0), (KeyCode::A, -1.0)] {
        let (mut app, mut rt) = setup(settings());
        frames(&mut rt, &mut app, 1);
        let (p0, r0) = (app.camera().position(), right(&app));
        press(&mut rt, &mut app, key, false);
        frames(&mut rt, &mut app, 30);
        let d = app.camera().position() - p0;
        assert!(d.length() > 0.0);
        assert!(d.normalize().dot(r0) * sign > 0.999, "{key:?}");
    }
}

#[test]
fn vertical_thrust_follows_camera_up_not_planet_up() {
    let (mut app, mut rt) = setup(settings());
    frames(&mut rt, &mut app, 1);
    press(&mut rt, &mut app, KeyCode::Z, false);
    frames(&mut rt, &mut app, 78); // about 90 degrees of roll
    release(&mut rt, &mut app, KeyCode::Z);
    frames(&mut rt, &mut app, 1);
    let (p0, u0) = (app.camera().position(), up(&app));
    let radial = p0.normalize();
    assert!(u0.dot(radial).abs() < 0.1, "camera up must be off radial");
    press(&mut rt, &mut app, KeyCode::E, false);
    frames(&mut rt, &mut app, 30);
    let d = app.camera().position() - p0;
    assert!(d.length() > 0.0);
    let dn = d.normalize();
    assert!(dn.dot(u0) > 0.999, "moves along camera up");
    assert!(dn.dot(radial).abs() < 0.1, "not along planet radial");
}

#[test]
fn f_toggles_and_levels_the_horizon() {
    let (mut app, mut rt) = setup(settings());
    frames(&mut rt, &mut app, 1);
    // Tilt the view away from the radial first so that roll tilts the horizon.
    rt.handle_input(
        &mut app,
        &InputEvent::MouseButtonPressed(MouseButton::Right),
    );
    for (x, y) in [(100.0, 100.0), (180.0, 330.0)] {
        rt.handle_input(&mut app, &InputEvent::CursorMoved { x, y });
    }
    rt.handle_input(
        &mut app,
        &InputEvent::MouseButtonReleased(MouseButton::Right),
    );
    frames(&mut rt, &mut app, 1);
    press(&mut rt, &mut app, KeyCode::Z, false);
    frames(&mut rt, &mut app, 40);
    release(&mut rt, &mut app, KeyCode::Z);
    frames(&mut rt, &mut app, 1);
    assert!(right(&app).dot(app.camera().position().normalize()).abs() > 0.1);

    press(&mut rt, &mut app, KeyCode::F, false);
    release(&mut rt, &mut app, KeyCode::F);
    assert_eq!(app.flight_mode(), FlightMode::PlanetAligned);
    frames(&mut rt, &mut app, 1);
    let radial = app.camera().position().normalize();
    assert!(right(&app).dot(radial).abs() < 1e-9);

    // Key repeat does not toggle.
    press(&mut rt, &mut app, KeyCode::F, true);
    assert_eq!(app.flight_mode(), FlightMode::PlanetAligned);
    // A second real press returns to free flight.
    press(&mut rt, &mut app, KeyCode::F, false);
    assert_eq!(app.flight_mode(), FlightMode::Free);
}

fn scripted_run(extra_keys: bool) -> (DVec3, [f64; 4], FlightMode) {
    let mut s = settings();
    s.script = Some(DescentScript::orbit_to_surface(s.terrain.terrain.radius));
    let (mut app, mut rt) = setup(s);
    if extra_keys {
        for k in [KeyCode::Z, KeyCode::X, KeyCode::F, KeyCode::D, KeyCode::E] {
            press(&mut rt, &mut app, k, false);
        }
    }
    frames(&mut rt, &mut app, 20);
    let q = app.camera().orientation();
    (
        app.camera().position(),
        [q.x, q.y, q.z, q.w],
        app.flight_mode(),
    )
}

#[test]
fn scripted_flight_ignores_free_flight_input() {
    let (p_a, q_a, m_a) = scripted_run(false);
    let (p_b, q_b, m_b) = scripted_run(true);
    assert_eq!(
        p_a.to_array().map(f64::to_bits),
        p_b.to_array().map(f64::to_bits)
    );
    assert_eq!(q_a.map(f64::to_bits), q_b.map(f64::to_bits));
    assert_eq!(m_a, FlightMode::Free);
    assert_eq!(m_b, FlightMode::Free);
}

#[test]
fn orientation_stays_normalised_over_a_long_mixed_sequence() {
    let (mut app, mut rt) = setup(settings());
    rt.handle_input(
        &mut app,
        &InputEvent::MouseButtonPressed(MouseButton::Right),
    );
    let keys = [KeyCode::Z, KeyCode::W, KeyCode::D, KeyCode::E, KeyCode::X];
    let mut x = 100.0;
    for i in 0..600usize {
        let k = keys[i % keys.len()];
        if i % 7 == 0 {
            press(&mut rt, &mut app, k, false);
        } else if i % 11 == 0 {
            release(&mut rt, &mut app, k);
        }
        if i == 300 {
            press(&mut rt, &mut app, KeyCode::F, false);
        }
        x += 3.0;
        rt.handle_input(
            &mut app,
            &InputEvent::CursorMoved {
                x,
                y: 50.0 + f64::from(i32::try_from(i % 13).unwrap()),
            },
        );
        frames(&mut rt, &mut app, 1);
        let q = app.camera().orientation();
        assert!((q.length() - 1.0).abs() < 1e-12, "step {i}");
    }
}
