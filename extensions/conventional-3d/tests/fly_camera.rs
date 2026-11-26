use conventional_3d::FlyCamera;
use engine_sdk::{Camera, FrameRegistry, InputEvent, KeyCode, MouseButton, WorldPos};

fn cam_at(x: f64, y: f64, z: f64) -> Camera {
    let frames = FrameRegistry::new();
    Camera::new(WorldPos::from_xyz(frames.root(), x, y, z).unwrap())
}

fn press(f: &mut FlyCamera, key: KeyCode) {
    f.handle_input(&InputEvent::KeyPressed { key, repeat: false });
}

fn release(f: &mut FlyCamera, key: KeyCode) {
    f.handle_input(&InputEvent::KeyReleased { key });
}

fn moved(c: &Camera, start: glam_free::P) -> (f64, f64, f64) {
    let l = c.position.local();
    (l.x - start.0, l.y - start.1, l.z - start.2)
}

mod glam_free {
    pub type P = (f64, f64, f64);
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn fly_forward_w() {
    let mut f = FlyCamera::new();
    let mut c = cam_at(0.0, 0.0, 0.0);
    press(&mut f, KeyCode::W);
    f.update(1.0, &mut c).unwrap();
    let (x, y, z) = moved(&c, (0.0, 0.0, 0.0));
    assert!(near(x, 0.0) && near(y, 0.0) && near(z, -5.0));
    release(&mut f, KeyCode::W);
    f.update(1.0, &mut c).unwrap();
    let (_, _, z) = moved(&c, (0.0, 0.0, 0.0));
    assert!(near(z, -5.0));
}

#[test]
fn fly_boost_shift() {
    let mut f = FlyCamera::new();
    let mut c = cam_at(0.0, 0.0, 0.0);
    press(&mut f, KeyCode::W);
    press(&mut f, KeyCode::ShiftLeft);
    f.update(1.0, &mut c).unwrap();
    assert!(near(moved(&c, (0.0, 0.0, 0.0)).2, -50.0));
}

#[test]
fn fly_diagonal_normalized() {
    let mut f = FlyCamera::new();
    let mut c = cam_at(0.0, 0.0, 0.0);
    press(&mut f, KeyCode::W);
    press(&mut f, KeyCode::D);
    f.update(1.0, &mut c).unwrap();
    let (x, _, z) = moved(&c, (0.0, 0.0, 0.0));
    assert!(((x * x + z * z).sqrt() - 5.0).abs() < 1e-6);
}

#[test]
fn fly_vertical_keys() {
    for (key, sign) in [
        (KeyCode::E, 1.0),
        (KeyCode::Space, 1.0),
        (KeyCode::Q, -1.0),
        (KeyCode::ControlLeft, -1.0),
    ] {
        let mut f = FlyCamera::new();
        let mut c = cam_at(0.0, 0.0, 0.0);
        press(&mut f, key);
        f.update(1.0, &mut c).unwrap();
        assert!(near(moved(&c, (0.0, 0.0, 0.0)).1, 5.0 * sign));
    }
}

#[test]
fn fly_mouse_look_requires_right_button() {
    let mut f = FlyCamera::new();
    f.handle_input(&InputEvent::CursorMoved { x: 50.0, y: 50.0 });
    f.handle_input(&InputEvent::CursorMoved { x: 90.0, y: 50.0 });
    assert_eq!(f.yaw, 0.0);
    f.handle_input(&InputEvent::MouseButtonPressed(MouseButton::Right));
    f.handle_input(&InputEvent::CursorMoved { x: 100.0, y: 100.0 });
    assert_eq!(f.yaw, 0.0);
    f.handle_input(&InputEvent::CursorMoved { x: 200.0, y: 100.0 });
    assert!((f.yaw - (-100.0 * f.sensitivity)).abs() < 1e-6);
    f.handle_input(&InputEvent::MouseButtonReleased(MouseButton::Right));
    let yaw = f.yaw;
    f.handle_input(&InputEvent::CursorMoved { x: 300.0, y: 100.0 });
    assert_eq!(f.yaw, yaw);
}

#[test]
fn fly_pitch_clamped() {
    let mut f = FlyCamera::new();
    f.handle_input(&InputEvent::MouseButtonPressed(MouseButton::Right));
    f.handle_input(&InputEvent::CursorMoved {
        x: 0.0,
        y: 100000.0,
    });
    f.handle_input(&InputEvent::CursorMoved { x: 0.0, y: 0.0 });
    assert!((f.pitch - 89f32.to_radians()).abs() < 1e-5);
    f.handle_input(&InputEvent::CursorMoved {
        x: 0.0,
        y: -100000.0,
    });
    assert!(f.pitch <= FlyCamera::MAX_PITCH);
    f.handle_input(&InputEvent::CursorMoved {
        x: 0.0,
        y: 100000.0,
    });
    f.handle_input(&InputEvent::CursorMoved {
        x: 0.0,
        y: -200000.0,
    });
    assert!(f.pitch >= -FlyCamera::MAX_PITCH);
}

#[test]
fn fly_focus_loss_clears_state() {
    let mut f = FlyCamera::new();
    let mut c = cam_at(0.0, 0.0, 0.0);
    press(&mut f, KeyCode::W);
    f.handle_input(&InputEvent::MouseButtonPressed(MouseButton::Right));
    f.handle_input(&InputEvent::Focused(false));
    f.update(1.0, &mut c).unwrap();
    assert!(near(moved(&c, (0.0, 0.0, 0.0)).2, 0.0));
    f.handle_input(&InputEvent::CursorMoved { x: 0.0, y: 0.0 });
    f.handle_input(&InputEvent::CursorMoved { x: 50.0, y: 0.0 });
    assert_eq!(f.yaw, 0.0);
}

#[test]
fn fly_orientation_applied() {
    let mut f = FlyCamera::new();
    f.yaw = std::f32::consts::FRAC_PI_2;
    let mut c = cam_at(0.0, 0.0, 0.0);
    press(&mut f, KeyCode::W);
    f.update(1.0, &mut c).unwrap();
    let (x, _, z) = moved(&c, (0.0, 0.0, 0.0));
    assert!((x + 5.0).abs() < 1e-5 && z.abs() < 1e-5);
}

#[test]
fn fly_ignores_bad_dt() {
    let mut f = FlyCamera::new();
    let mut c = cam_at(1.0, 2.0, 3.0);
    press(&mut f, KeyCode::W);
    assert!(f.update(f64::NAN, &mut c).is_ok());
    assert!(f.update(-1.0, &mut c).is_ok());
    assert_eq!(moved(&c, (1.0, 2.0, 3.0)), (0.0, 0.0, 0.0));
}

#[test]
fn fly_far_from_origin() {
    let mut f = FlyCamera::new();
    let mut c = cam_at(6.4e6, 0.0, 0.0);
    press(&mut f, KeyCode::W);
    f.update(0.016, &mut c).unwrap();
    assert!(near(moved(&c, (6.4e6, 0.0, 0.0)).2, -0.08));
}

#[test]
fn fly_ignores_non_finite_cursor() {
    let mut f = FlyCamera::new();
    f.handle_input(&InputEvent::MouseButtonPressed(MouseButton::Right));
    f.handle_input(&InputEvent::CursorMoved { x: 0.0, y: 0.0 });
    f.handle_input(&InputEvent::CursorMoved {
        x: f64::NAN,
        y: 0.0,
    });
    assert!(f.yaw.is_finite() && f.pitch.is_finite());
}
