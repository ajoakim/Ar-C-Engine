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
