//! 6.4e6 m precision and rebase integration tests (public API only).

use engine_math::{DAffine3, DVec3, FrameRegistry, MathError};
use engine_world::{Transform, World, WorldError};

fn at(x: f64, y: f64, z: f64) -> Transform {
    Transform::from_translation(DVec3::new(x, y, z))
}

fn close(a: DVec3, b: DVec3, tol: f64) -> bool {
    (a.x - b.x).abs() <= tol && (a.y - b.y).abs() <= tol && (a.z - b.z).abs() <= tol
}

#[test]
fn precision_mm_child_under_planet_radius_root() {
    let mut frames = FrameRegistry::new();
    let planet = frames
        .add_frame("planet", frames.root(), DAffine3::IDENTITY)
        .unwrap();
    let mut w = World::new();
    let r = w.spawn(planet, at(6.4e6, 0.0, 0.0)).unwrap();
    let c = w.spawn_child(r, at(0.001, 0.0, 0.0)).unwrap();
    w.update_transforms();
    let rp = w.world_transform(r).unwrap().position;
    let cp = w.world_transform(c).unwrap().position;
    let d = frames.delta(rp, cp).unwrap();
    assert!(close(d, DVec3::new(0.001, 0.0, 0.0), 1e-9), "{d:?}");
}

#[test]
fn precision_rebase_preserves_world_positions() {
    let mut frames = FrameRegistry::new();
    let root_frame = frames.root();
    let local = frames
        .add_frame(
            "local",
            root_frame,
            DAffine3::from_translation(DVec3::new(100.0, 200.0, 300.0)),
        )
        .unwrap();
    let mut w = World::new();
    let r = w.spawn(local, at(6.4e6, 1.0, -2.0)).unwrap();
    let c = w.spawn_child(r, at(0.25, 0.0, 0.0)).unwrap();
    w.update_transforms();
    let before_r = w.world_position_in(r, &frames, root_frame).unwrap().local();
    let before_c = w.world_position_in(c, &frames, root_frame).unwrap().local();

    w.rebase(&mut frames, local, DVec3::new(6.4e6, 0.0, 0.0))
        .unwrap();
    assert_eq!(w.world_transform(r), Err(WorldError::DirtyTransform(r)));
    w.update_transforms();
    let after_r = w.world_position_in(r, &frames, root_frame).unwrap().local();
    let after_c = w.world_position_in(c, &frames, root_frame).unwrap().local();
    assert!(close(before_r, after_r, 1e-8), "{before_r:?} {after_r:?}");
    assert!(close(before_c, after_c, 1e-8), "{before_c:?} {after_c:?}");
    assert!(close(
        w.local(r).unwrap().translation,
        DVec3::new(0.0, 1.0, -2.0),
        1e-8
    ));
}

#[test]
fn precision_rebase_root_frame_rejected() {
    let mut frames = FrameRegistry::new();
    let root_frame = frames.root();
    let mut w = World::new();
    w.spawn(root_frame, at(1.0, 2.0, 3.0)).unwrap();
    w.update_transforms();
    let res = w.rebase(&mut frames, root_frame, DVec3::new(1.0, 0.0, 0.0));
    assert_eq!(res, Err(WorldError::Math(MathError::RootFrame)));
    assert_eq!(w.update_transforms(), 0);
}

#[test]
fn precision_rebase_non_finite_rejected() {
    let mut frames = FrameRegistry::new();
    let f = frames
        .add_frame("f", frames.root(), DAffine3::IDENTITY)
        .unwrap();
    let mut w = World::new();
    let r = w.spawn(f, at(5.0, 6.0, 7.0)).unwrap();
    w.update_transforms();
    let res = w.rebase(&mut frames, f, DVec3::new(f64::NAN, 0.0, 0.0));
    assert!(res.is_err());
    assert_eq!(w.local(r).unwrap().translation, DVec3::new(5.0, 6.0, 7.0));
    assert_eq!(w.update_transforms(), 0);
}
