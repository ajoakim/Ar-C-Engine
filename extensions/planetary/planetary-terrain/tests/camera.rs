use engine_sdk::{Camera, DQuat, DVec3, FrameRegistry, InputEvent, KeyCode, WorldPos};
use planetary_core::{PlanetTerrain, TerrainParams};
use planetary_terrain::{CameraParams, PlanetaryCamera};

const SEED: u64 = 0x0007_5EED_2026_0001;

struct Fixture {
    terrain: PlanetTerrain,
    radius: f64,
}

fn fixture() -> Fixture {
    let params = TerrainParams::default();
    let radius = params.radius;
    Fixture {
        terrain: PlanetTerrain::new(SEED, params, 8).expect("terrain"),
        radius,
    }
}

fn press(cam: &mut PlanetaryCamera, key: KeyCode) {
    cam.handle_input(&InputEvent::KeyPressed { key, repeat: false });
}

fn step_len(alt: f64, boost: bool, f: &Fixture) -> f64 {
    let surf = |d: DVec3| f.radius + f.terrain.height_at_dir(d);
    let dir = DVec3::new(1.0, 0.0, 0.0);
    let mut cam = PlanetaryCamera::new(dir * (surf(dir) + alt), CameraParams::default()).unwrap();
    press(&mut cam, KeyCode::W);
    if boost {
        press(&mut cam, KeyCode::ShiftLeft);
    }
    let p0 = cam.position();
    cam.update(1.0 / 60.0, &surf, 0.0);
    (cam.position() - p0).length()
}

#[test]
fn speed_scales_with_altitude() {
    let f = fixture();
    let ratio = step_len(1.0e6, false, &f) / step_len(1.0e3, false, &f);
    assert!((ratio / 1000.0 - 1.0).abs() < 0.01, "ratio {ratio}");
}

#[test]
fn speed_is_clamped_and_boosted() {
    let f = fixture();
    let p = CameraParams::default();
    let dt = 1.0 / 60.0;
    let s = step_len(5.0e7, false, &f);
    assert!((s - p.max_speed * dt).abs() < 1e-6 * p.max_speed * dt);
    let b = step_len(1.0e3, true, &f) / step_len(1.0e3, false, &f);
    assert!((b - p.boost).abs() < 1e-6, "boost {b}");
}

#[test]
fn parallel_transport_keeps_view_angle() {
    let f = fixture();
    let surf = |d: DVec3| f.radius + f.terrain.height_at_dir(d);
    let mut cam = PlanetaryCamera::new(
        DVec3::new(f.radius + 1.0e4, 0.0, 0.0),
        CameraParams::default(),
    )
    .unwrap();
    // Tilt the view to a tangent look direction.
    let up = cam.position().normalize();
    let tangent = DVec3::Z;
    let q = DQuat::from_rotation_arc(DVec3::NEG_Z, tangent);
    assert!(cam.set_pose(cam.position(), q));
    let angle0 = cam.forward().dot(up).clamp(-1.0, 1.0).acos();
    press(&mut cam, KeyCode::D);
    for _ in 0..2000 {
        cam.update(0.1, &surf, 0.0);
        let u = cam.position().normalize();
        let a = cam.forward().dot(u).clamp(-1.0, 1.0).acos();
        assert!((a - angle0).abs() < 1e-6, "angle drift {}", a - angle0);
        let o = cam.orientation();
        assert!(o.x.is_finite() && o.y.is_finite() && o.z.is_finite() && o.w.is_finite());
    }
}

#[test]
fn pole_start_is_finite() {
    let f = fixture();
    let surf = |d: DVec3| f.radius + f.terrain.height_at_dir(d);
    let mut cam = PlanetaryCamera::new(
        DVec3::new(0.0, f.radius + 1.0e4, 0.0),
        CameraParams::default(),
    )
    .unwrap();
    press(&mut cam, KeyCode::W);
    press(&mut cam, KeyCode::D);
    for _ in 0..100 {
        cam.update(0.1, &surf, 0.0);
    }
    let o = cam.orientation();
    assert!(o.x.is_finite() && o.y.is_finite() && o.z.is_finite() && o.w.is_finite());
    assert!(cam.position().is_finite());
}

#[test]
fn clearance_is_respected() {
    let f = fixture();
    let surf = |d: DVec3| f.radius + f.terrain.height_at_dir(d);
    let p = CameraParams::default();
    let dir = DVec3::new(0.3, 0.5, 0.8).normalize();
    let mut cam = PlanetaryCamera::new(dir * (surf(dir) + 100.0), p).unwrap();
    press(&mut cam, KeyCode::Q);
    for _ in 0..3000 {
        cam.update(1.0 / 60.0, &surf, 0.5);
        assert!(cam.altitude(&surf) >= p.min_clearance + 0.5 - 1e-6);
    }
}

#[test]
fn near_plane_and_projection() {
    assert_eq!(PlanetaryCamera::near_plane(2.0), 0.05);
    assert_eq!(PlanetaryCamera::near_plane(1000.0), 10.0);
    assert_eq!(PlanetaryCamera::near_plane(1.0e7), 1.0e4);
    let f = fixture();
    let frames = FrameRegistry::new();
    let root = frames.root();
    for alt in [2.0, 1.0e7] {
        let pos = DVec3::new(f.radius + alt, 0.0, 0.0);
        let cam = PlanetaryCamera::new(pos, CameraParams::default()).unwrap();
        let mut c = Camera::new(WorldPos::from_xyz(root, 0.0, 0.0, 0.0).unwrap());
        cam.apply_to(&mut c, root, alt).unwrap();
        let m = c.projection(16.0 / 9.0).expect("projection");
        assert!(m.to_cols_array().iter().all(|v| v.is_finite()));
    }
}

#[test]
fn apply_to_matches_pose() {
    let frames = FrameRegistry::new();
    let root = frames.root();
    let pos = DVec3::new(7.0e6, 123.5, -42.25);
    let cam = PlanetaryCamera::new(pos, CameraParams::default()).unwrap();
    let mut c = Camera::new(WorldPos::from_xyz(root, 0.0, 0.0, 0.0).unwrap());
    cam.apply_to(&mut c, root, 1.0e5).unwrap();
    let p = c.position.local();
    assert_eq!(p, pos);
    let q = cam.orientation().as_quat();
    assert!((c.orientation.dot(q.normalize()) - 1.0).abs() < 1e-6);
    assert!((c.orientation.length() - 1.0).abs() < 1e-6);
}

#[test]
fn invalid_dt_and_pose_rejected() {
    let f = fixture();
    let surf = |d: DVec3| f.radius + f.terrain.height_at_dir(d);
    let mut cam = PlanetaryCamera::new(
        DVec3::new(f.radius + 1.0e4, 0.0, 0.0),
        CameraParams::default(),
    )
    .unwrap();
    press(&mut cam, KeyCode::W);
    let (p, o) = (cam.position(), cam.orientation());
    for dt in [f64::NAN, 0.0, -1.0] {
        cam.update(dt, &surf, 0.0);
        assert_eq!(cam.position(), p);
        assert_eq!(cam.orientation(), o);
    }
    assert!(!cam.set_pose(DVec3::new(f64::NAN, 0.0, 0.0), o));
    assert_eq!(cam.position(), p);
    assert!(PlanetaryCamera::new(DVec3::ZERO, CameraParams::default()).is_none());
}

fn release(cam: &mut PlanetaryCamera, key: KeyCode) {
    cam.handle_input(&InputEvent::KeyReleased { key });
}

fn cam_at(alt: f64, f: &Fixture) -> PlanetaryCamera {
    let dir = DVec3::new(1.0, 0.0, 0.0);
    let surf = f.radius + f.terrain.height_at_dir(dir);
    PlanetaryCamera::new(dir * (surf + alt), CameraParams::default()).unwrap()
}

fn surf_fn(f: &Fixture) -> impl Fn(DVec3) -> f64 + '_ {
    move |d| f.radius + f.terrain.height_at_dir(d)
}

#[test]
fn key_press_and_release_move_forward_then_stop() {
    let f = fixture();
    let s = surf_fn(&f);
    let mut cam = cam_at(1.0e4, &f);
    press(&mut cam, KeyCode::W);
    let (p0, fwd) = (cam.position(), cam.forward());
    cam.update(0.1, &s, 0.0);
    assert!((cam.position() - p0).dot(fwd) > 0.0);
    release(&mut cam, KeyCode::W);
    let p1 = cam.position();
    cam.update(0.1, &s, 0.0);
    assert_eq!(cam.position(), p1);
}

#[test]
fn arrows_mirror_wasd() {
    let f = fixture();
    let s = surf_fn(&f);
    for (a, b) in [
        (KeyCode::W, KeyCode::Up),
        (KeyCode::S, KeyCode::Down),
        (KeyCode::A, KeyCode::Left),
        (KeyCode::D, KeyCode::Right),
    ] {
        let (mut c1, mut c2) = (cam_at(1.0e4, &f), cam_at(1.0e4, &f));
        press(&mut c1, a);
        press(&mut c2, b);
        c1.update(0.1, &s, 0.0);
        c2.update(0.1, &s, 0.0);
        assert_eq!(c1.position(), c2.position());
    }
}

#[test]
fn vertical_keys_change_radius() {
    let f = fixture();
    let s = surf_fn(&f);
    for key in [KeyCode::E, KeyCode::Space] {
        let mut c = cam_at(1.0e4, &f);
        let r0 = c.position().length();
        press(&mut c, key);
        c.update(0.1, &s, 0.0);
        assert!(c.position().length() > r0);
    }
    for key in [KeyCode::Q, KeyCode::ControlLeft] {
        let mut c = cam_at(1.0e4, &f);
        let r0 = c.position().length();
        press(&mut c, key);
        c.update(0.1, &s, 0.0);
        assert!(c.position().length() < r0);
    }
}

#[test]
fn repeat_events_change_nothing() {
    let f = fixture();
    let s = surf_fn(&f);
    let mut c = cam_at(1.0e4, &f);
    c.handle_input(&InputEvent::KeyPressed {
        key: KeyCode::W,
        repeat: true,
    });
    let p = c.position();
    c.update(0.1, &s, 0.0);
    assert_eq!(c.position(), p);
}

fn cursor(c: &mut PlanetaryCamera, x: f64, y: f64) {
    c.handle_input(&InputEvent::CursorMoved { x, y });
}

#[test]
fn mouse_look_requires_right_button() {
    let f = fixture();
    let s = surf_fn(&f);
    let mut c = cam_at(1.0e4, &f);
    let o = c.orientation();
    cursor(&mut c, 100.0, 100.0);
    cursor(&mut c, 110.0, 100.0);
    c.update(0.1, &s, 0.0);
    assert_eq!(c.orientation(), o);

    c.handle_input(&InputEvent::MouseButtonPressed(
        engine_sdk::MouseButton::Right,
    ));
    cursor(&mut c, 100.0, 100.0); // first move only records
    c.update(0.1, &s, 0.0);
    assert_eq!(c.orientation(), o);
    cursor(&mut c, 110.0, 100.0);
    c.update(0.1, &s, 0.0);
    let o2 = c.orientation();
    assert_ne!(o2, o);

    c.handle_input(&InputEvent::MouseButtonReleased(
        engine_sdk::MouseButton::Right,
    ));
    cursor(&mut c, 200.0, 100.0);
    c.update(0.1, &s, 0.0);
    assert_eq!(c.orientation(), o2);
}

#[test]
fn pitch_is_clamped_near_surface() {
    let f = fixture();
    let s = surf_fn(&f);
    let mut c = cam_at(1.0e3, &f);
    c.handle_input(&InputEvent::MouseButtonPressed(
        engine_sdk::MouseButton::Right,
    ));
    cursor(&mut c, 0.0, 0.0);
    cursor(&mut c, 0.0, -1.0e6);
    c.update(0.1, &s, 0.0);
    let up = c.position().normalize();
    let pitch = c.forward().dot(up).clamp(-1.0, 1.0).asin();
    assert!(pitch <= CameraParams::default().max_pitch + 1e-9);
}

#[test]
fn focus_loss_clears_state_and_escape_exits() {
    let f = fixture();
    let s = surf_fn(&f);
    let mut c = cam_at(1.0e4, &f);
    press(&mut c, KeyCode::W);
    c.handle_input(&InputEvent::Focused(false));
    let p = c.position();
    c.update(0.1, &s, 0.0);
    assert_eq!(c.position(), p);
    assert!(!c.exit_requested());
    press(&mut c, KeyCode::Escape);
    assert!(c.exit_requested());
}

#[test]
fn irrelevant_events_are_ignored() {
    let f = fixture();
    let s = surf_fn(&f);
    let mut c = cam_at(1.0e4, &f);
    let (p, o) = (c.position(), c.orientation());
    press(&mut c, KeyCode::Other);
    press(&mut c, KeyCode::Enter);
    c.handle_input(&InputEvent::MouseWheel {
        lines_x: 1.0,
        lines_y: 2.0,
    });
    c.handle_input(&InputEvent::Resized {
        width: 10,
        height: 10,
    });
    c.handle_input(&InputEvent::CloseRequested);
    c.handle_input(&InputEvent::CursorMoved {
        x: f64::NAN,
        y: 0.0,
    });
    c.update(0.1, &s, 0.0);
    assert_eq!(c.position(), p);
    assert_eq!(c.orientation(), o);
}

fn plane_dist(p: &[f64; 4], x: DVec3) -> f64 {
    p[0] * x.x + p[1] * x.y + p[2] * x.z + p[3]
}

#[test]
fn frustum_planes_contain_view_axis() {
    let pos = DVec3::new(6_372_000.0, 10.0, -20.0);
    let cam = PlanetaryCamera::new(pos, CameraParams::default()).unwrap();
    let planes = cam.frustum_planes(16.0 / 9.0, 1.0);
    let f = cam.forward();
    for (i, p) in planes.iter().enumerate() {
        assert!(plane_dist(p, pos) >= -1e-9 || i == 0, "eye plane {i}");
        if i < 5 {
            let n = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((n - 1.0).abs() < 1e-12, "plane {i} norm {n}");
        }
        assert!(plane_dist(p, pos + f * 10.0) > 0.0, "front {i}");
    }
    assert!(plane_dist(&planes[0], pos) < 0.0, "eye is in front of near");
    let behind = pos - f * 10.0;
    assert!(planes.iter().any(|p| plane_dist(p, behind) < 0.0));
    // A point far to the side is outside the left/right planes.
    let r = cam.orientation() * DVec3::X;
    assert!(
        planes
            .iter()
            .any(|p| plane_dist(p, pos + f * 10.0 + r * 1000.0) < 0.0)
    );
    assert!(
        planes
            .iter()
            .any(|p| plane_dist(p, pos + f * 10.0 - r * 1000.0) < 0.0)
    );
}
