//! Free-flight controller: drift, parity, roll axis, camera-relative thrust and levelling.

use std::f64::consts::FRAC_PI_2;

use engine_sdk::{DQuat, DVec3, InputEvent, KeyCode, MouseButton};
use planetary_app::free_flight::{level_orientation, rotate_local};
use planetary_app::{DEFAULT_ROLL_RATE, FlightMode, FreeFlight};
use planetary_terrain::{CameraParams, PlanetaryCamera};

const R: f64 = 6_371_000.0;
const DT: f64 = 1.0 / 60.0;

fn flat(_: DVec3) -> f64 {
    R
}

fn start_pos() -> DVec3 {
    DVec3::new(0.0, 0.0, R + 1.0e4)
}

fn pose() -> DQuat {
    (DQuat::from_axis_angle(DVec3::Y, 0.7)
        * DQuat::from_axis_angle(DVec3::X, -0.4)
        * DQuat::from_axis_angle(DVec3::Z, 0.2))
    .normalize()
}

fn press(f: &mut FreeFlight, key: KeyCode) {
    f.handle_input(&InputEvent::KeyPressed { key, repeat: false });
}

fn release(f: &mut FreeFlight, key: KeyCode) {
    f.handle_input(&InputEvent::KeyReleased { key });
}

fn quat_close(a: DQuat, b: DQuat, tol: f64) -> bool {
    let s = if a.dot(b) < 0.0 { -1.0 } else { 1.0 };
    (a - b * s).length() < tol
}

fn assert_orthonormal(q: DQuat) {
    assert!((q.length() - 1.0).abs() < 1e-12, "|q| = {}", q.length());
    let r = q * DVec3::X;
    let u = q * DVec3::Y;
    let f = q * DVec3::NEG_Z;
    assert!(r.dot(u).abs() < 1e-12);
    assert!(r.dot(f).abs() < 1e-12);
    assert!(u.dot(f).abs() < 1e-12);
    assert!((r.length() - 1.0).abs() < 1e-12);
    assert!((u.length() - 1.0).abs() < 1e-12);
    assert!((f.length() - 1.0).abs() < 1e-12);
    assert!((r.cross(u) + f).length() < 1e-12);
}

#[test]
fn normalise_guard_non_unit_inputs() {
    let q = pose();
    let a = rotate_local(q * 1.001, 0.0, 0.0, 0.0);
    assert!((a.length() - 1.0).abs() < 1e-12);
    let b = rotate_local(q * 0.999, 0.013, -0.021, 0.034);
    assert!((b.length() - 1.0).abs() < 1e-12);
    let c = rotate_local(q * 1.01, 0.0, 0.0, 0.0);
    assert!((c.length() - 1.0).abs() < 1e-12);
}

#[test]
fn drift_pure_rotate_local_100000_steps() {
    let mut q = pose();
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut steps = 0_u32;
    for _ in 0..100_000 {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let a = ((seed >> 33) % 2001) as f64 / 1000.0 - 1.0;
        let b = ((seed >> 20) % 2001) as f64 / 1000.0 - 1.0;
        let c = ((seed >> 7) % 2001) as f64 / 1000.0 - 1.0;
        q = rotate_local(q, a * 0.05, b * 0.05, c * 0.05);
        steps += 1;
    }
    assert_eq!(steps, 100_000);
    assert_orthonormal(q);
}

#[test]
fn drift_end_to_end_100000_steps() {
    let mut f = FreeFlight::new(start_pos(), pose(), CameraParams::default()).unwrap();
    f.handle_input(&InputEvent::MouseButtonPressed(MouseButton::Right));
    let mut seed: u64 = 12345;
    let (mut cx, mut cy) = (500.0_f64, 400.0_f64);
    let mut steps = 0_u32;
    let keys = [
        KeyCode::Z,
        KeyCode::X,
        KeyCode::W,
        KeyCode::S,
        KeyCode::A,
        KeyCode::D,
        KeyCode::E,
        KeyCode::Q,
    ];
    for i in 0..100_000_u32 {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let k = keys[((seed >> 40) % 8) as usize];
        if (seed >> 32) & 1 == 0 {
            press(&mut f, k);
        } else {
            release(&mut f, k);
        }
        cx += (((seed >> 20) % 61) as f64 - 30.0) * (1.0 + f64::from(i % 7));
        cy += (((seed >> 8) % 41) as f64 - 20.0) * (1.0 + f64::from(i % 5));
        f.handle_input(&InputEvent::CursorMoved { x: cx, y: cy });
        f.update(DT, &flat, 0.0);
        steps += 1;
    }
    assert_eq!(steps, 100_000);
    assert_eq!(f.mode(), FlightMode::Free);
    assert!(f.position().is_finite());
    assert_orthonormal(f.orientation());
}

#[test]
fn roll_reversibility_and_known_angle() {
    let q0 = pose();
    let n = 1000;
    let theta = 1.3;
    let mut q = q0;
    for _ in 0..n {
        q = rotate_local(q, 0.0, 0.0, theta / f64::from(n));
    }
    let mid = q;
    assert!(!quat_close(mid, q0, 1e-3));
    for _ in 0..n {
        q = rotate_local(q, 0.0, 0.0, -theta / f64::from(n));
    }
    assert!(quat_close(q, q0, 1e-9));

    // Single FreeFlight: hold Z then X for the same number of frames.
    let mut f = FreeFlight::new(start_pos(), q0, CameraParams::default()).unwrap();
    press(&mut f, KeyCode::Z);
    for _ in 0..120 {
        f.update(DT, &flat, 0.0);
    }
    assert!(!quat_close(f.orientation(), q0, 1e-3));
    release(&mut f, KeyCode::Z);
    press(&mut f, KeyCode::X);
    for _ in 0..120 {
        f.update(DT, &flat, 0.0);
    }
    assert!(quat_close(f.orientation(), q0, 1e-9));

    // Known angle about the view axis.
    let r = rotate_local(q0, 0.0, 0.0, 0.5);
    assert!(((q0 * DVec3::NEG_Z) - (r * DVec3::NEG_Z)).length() < 1e-12);
    let up0 = q0 * DVec3::Y;
    let up1 = r * DVec3::Y;
    assert!((up0.dot(up1).acos() - 0.5).abs() < 1e-9);
}

/// Scripted run through both controllers; returns the pair for comparison.
fn run_parity(alt: f64) -> (PlanetaryCamera, FreeFlight) {
    let params = CameraParams::default();
    let pos = DVec3::new(0.0, 0.0, R + alt);
    let mut cam = PlanetaryCamera::new(pos, params).unwrap();
    let mut ff = FreeFlight::new(pos, cam.orientation(), params).unwrap();
    ff.set_mode(FlightMode::PlanetAligned);
    let down = |key| InputEvent::KeyPressed { key, repeat: false };
    let up = |key| InputEvent::KeyReleased { key };
    let mut cx = 300.0_f64;
    let mut cy = 300.0_f64;
    for frame in 0..600_u32 {
        let mut events: Vec<InputEvent> = Vec::new();
        match frame {
            0 => {
                events.push(down(KeyCode::W));
                events.push(InputEvent::MouseButtonPressed(MouseButton::Right));
            }
            50 => events.push(down(KeyCode::D)),
            100 => events.push(down(KeyCode::E)),
            150 => events.push(down(KeyCode::ShiftLeft)),
            250 => {
                events.push(up(KeyCode::E));
                events.push(down(KeyCode::Q));
            }
            350 => events.push(up(KeyCode::W)),
            400 => events.push(up(KeyCode::ShiftLeft)),
            450 => events.push(up(KeyCode::D)),
            _ => {}
        }
        if frame > 0 {
            cx += f64::from((frame * 7) % 23) - 11.0;
            // Large downward pulses exercise the near-surface pitch clamp.
            cy += if frame % 97 < 30 { 40.0 } else { -25.0 };
            events.push(InputEvent::CursorMoved { x: cx, y: cy });
        }
        for e in &events {
            cam.handle_input(e);
            ff.handle_input(e);
        }
        cam.update(DT, &flat, 0.0);
        ff.update(DT, &flat, 0.0);
    }
    (cam, ff)
}

#[test]
fn aligned_parity_with_planetary_camera() {
    for alt in [1.0e4, 3.0e5] {
        let (cam, ff) = run_parity(alt);
        assert!(
            (cam.position() - ff.position()).length() < 1e-9,
            "alt {alt}"
        );
        assert!(
            cam.orientation().dot(ff.orientation()).abs() >= 1.0 - 1e-9,
            "alt {alt}"
        );
        // The run moved the camera and rotated the view (the comparison is not vacuous).
        assert!((cam.position() - DVec3::new(0.0, 0.0, R + alt)).length() > 1.0);
    }
}

#[test]
fn free_speed_scales_with_altitude_and_boost() {
    let params = CameraParams {
        min_clearance: 0.0,
        ..CameraParams::default()
    };
    let tangent = DQuat::from_axis_angle(DVec3::Y, FRAC_PI_2);
    let step = |alt: f64, boost: bool| -> f64 {
        let mut f = FreeFlight::new(DVec3::new(0.0, 0.0, R + alt), tangent, params).unwrap();
        press(&mut f, KeyCode::W);
        if boost {
            press(&mut f, KeyCode::ShiftLeft);
        }
        let p0 = f.position();
        f.update(0.01, &flat, 0.0);
        (f.position() - p0).length() / 0.01
    };
    let expect =
        |alt: f64| (alt * params.speed_per_altitude).clamp(params.min_speed, params.max_speed);
    for alt in [1.0, 1.0e4, 1.0e9] {
        assert!((step(alt, false) - expect(alt)).abs() < 1e-3, "alt {alt}");
    }
    assert!((step(1.0, false) - params.min_speed).abs() < 1e-6);
    assert!((step(1.0e9, false) - params.max_speed).abs() < 1e-3);
    let mid = step(1.0e4, false);
    assert!(mid > step(1.0, false) && mid < step(1.0e9, false));
    assert!((step(1.0e4, true) / mid - params.boost).abs() < 1e-9);
}

#[test]
fn roll_leaves_forward_and_rotates_up_by_rate() {
    let mut f = FreeFlight::new(start_pos(), pose(), CameraParams::default()).unwrap();
    assert_eq!(f.roll_rate(), DEFAULT_ROLL_RATE);
    let (f0, u0) = (f.forward(), f.orientation() * DVec3::Y);
    press(&mut f, KeyCode::Z);
    f.update(DT, &flat, 0.0);
    let u1 = f.orientation() * DVec3::Y;
    assert!((f.forward() - f0).length() < 1e-12);
    assert!((u0.dot(u1).acos() - DEFAULT_ROLL_RATE * DT).abs() < 1e-9);
}

#[test]
fn thrust_is_camera_relative_after_quarter_roll() {
    // Camera up is the planet up; the camera looks along +Y (tangent).
    let q = DQuat::from_axis_angle(DVec3::X, FRAC_PI_2);
    assert!(((q * DVec3::Y) - DVec3::Z).length() < 1e-12);
    let rolled = rotate_local(q, 0.0, 0.0, FRAC_PI_2);
    let planet_up = DVec3::Z;
    // After the roll the camera up is horizontal and the camera right is radial.
    assert!((rolled * DVec3::Y).dot(planet_up).abs() < 1e-12);
    assert!((rolled * DVec3::X).dot(planet_up).abs() > 1.0 - 1e-12);

    for (key, axis, radial) in [(KeyCode::E, DVec3::Y, false), (KeyCode::D, DVec3::X, true)] {
        let mut f = FreeFlight::new(start_pos(), rolled, CameraParams::default()).unwrap();
        let p0 = f.position();
        press(&mut f, key);
        f.update(0.1, &flat, 0.0);
        let dir = (f.position() - p0).normalize();
        assert!((dir - rolled * axis).length() < 1e-9);
        let along_up = dir.dot(planet_up).abs();
        if radial {
            assert!(along_up > 1.0 - 1e-9);
        } else {
            assert!(along_up < 1e-9);
        }
    }
}

#[test]
fn toggle_levels_horizon_and_back_is_identity() {
    let pos = DVec3::new(2.0e6, -3.0e6, R + 4.0e5);
    let mut f = FreeFlight::new(pos, pose(), CameraParams::default()).unwrap();
    let up = pos.normalize();
    let f0 = f.forward();
    let before = f.orientation();
    assert!(
        (before * DVec3::X).dot(up).abs() > 1e-3,
        "pose must be rolled"
    );
    assert_eq!(f.toggle_mode(), FlightMode::PlanetAligned);
    let q = f.orientation();
    assert!((q * DVec3::X).dot(up).abs() < 1e-12);
    assert!((f.forward() - f0).length() < 1e-12);
    assert!((q * DVec3::Y).dot(up) > 0.0);
    assert_eq!(q, level_orientation(before, up));
    assert_eq!(f.toggle_mode(), FlightMode::Free);
    assert_eq!(f.orientation().to_array(), q.to_array());
    assert_eq!(f.position(), pos);
}

#[test]
fn aligned_mode_ignores_roll_keys() {
    let mut f = FreeFlight::new(start_pos(), pose(), CameraParams::default()).unwrap();
    f.toggle_mode();
    let q = f.orientation();
    press(&mut f, KeyCode::Z);
    for _ in 0..30 {
        f.update(DT, &flat, 0.0);
    }
    assert!(quat_close(f.orientation(), q, 1e-12));
}

#[test]
fn non_finite_inputs_never_corrupt_pose() {
    let mut f = FreeFlight::new(start_pos(), pose(), CameraParams::default()).unwrap();
    f.handle_input(&InputEvent::MouseButtonPressed(MouseButton::Right));
    f.handle_input(&InputEvent::CursorMoved { x: 1.0, y: 1.0 });
    f.handle_input(&InputEvent::CursorMoved {
        x: f64::NAN,
        y: 5.0,
    });
    f.handle_input(&InputEvent::CursorMoved { x: 3.0, y: 4.0 });
    press(&mut f, KeyCode::W);
    f.update(f64::NAN, &flat, 0.0);
    f.update(f64::INFINITY, &flat, 0.0);
    f.update(0.0, &flat, 0.0);
    assert_eq!(f.position(), start_pos());
    f.update(DT, &flat, 0.0);
    assert!(f.position().is_finite());
    assert_orthonormal(f.orientation());
    f.handle_input(&InputEvent::Focused(false));
    let p = f.position();
    f.update(DT, &flat, 0.0);
    assert_eq!(f.position(), p);
    assert_orthonormal(f.orientation());
}
