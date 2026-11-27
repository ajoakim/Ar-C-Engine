//! 07.1-27: reusing the previous selection (`always_reselect = false`) gives exactly the result of
//! running `select_cells` every frame (`always_reselect = true`), on every frame of the scripted
//! descent, the fast free-flight and a seeded randomised flight, for hex and cube.
//!
//! Both apps run on the simulated clock with workers settled every frame, so the two runs see the
//! same residency and the same adaptive caps; the only allowed differences are the
//! `reselect_skipped` flag and the `range_lookups` probe counter (0 on a reused frame).

use engine_sdk::{DQuat, DVec3, Engine, EngineConfig, NullBackend, Runtime};
use planetary_app::{AppSettings, DescentScript, PlanetaryApp, SimClock, TopologyKind};

const DT: f64 = 1.0 / 30.0;

fn settings(kind: TopologyKind, always: bool) -> AppSettings {
    let mut s = match kind {
        TopologyKind::Hex => AppSettings::default(),
        TopologyKind::Cube => AppSettings::cube_default(),
    };
    s.terrain.cells_per_edge = 8;
    s.terrain.lod.max_level = 10;
    s.terrain.settle_each_frame = true;
    s.terrain.uploads_per_frame = 32;
    s.terrain.max_requests_per_frame = 64;
    s.terrain.stream.max_starts_per_frame = 64;
    s.terrain.always_reselect = always;
    s.adaptive_streaming = true;
    s
}

fn runtime() -> Runtime<NullBackend> {
    Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).expect("runtime")
}

/// A pose for each frame of a path (`None` leaves the app's own script in charge).
type Pose = Option<(DVec3, DQuat)>;

/// Per-frame state compared between the two runs.
#[derive(Debug, PartialEq)]
struct Snap {
    wanted_raw: Vec<u64>,
    wanted_levels: Vec<u8>,
    drawn_raw: Vec<u64>,
    wanted_split: Vec<u64>,
    report: planetary_terrain::TerrainReport,
}

fn snap(app: &PlanetaryApp) -> (Snap, bool) {
    let mut report = app.report().clone();
    let skipped = report.reselect_skipped;
    report.reselect_skipped = false;
    // Probes of the held table are what a reused selection saves; every other field must match.
    report.range_lookups = 0;
    (
        Snap {
            wanted_raw: app.wanted_raw(),
            wanted_levels: app.wanted_levels(),
            drawn_raw: app.drawn_raw(),
            wanted_split: app.wanted_split_raw(),
            report,
        },
        skipped,
    )
}

/// Runs both variants frame by frame over `poses` (or `frames` frames of the script) and asserts
/// exact equality on every frame. Returns the per-frame skip flags of the skipping run.
fn compare(
    kind: TopologyKind,
    script: Option<DescentScript>,
    poses: Option<&[Pose]>,
    frames: usize,
) -> Vec<bool> {
    let mut a_set = settings(kind, true);
    let mut b_set = settings(kind, false);
    a_set.script = script;
    b_set.script = script;
    let mut a = PlanetaryApp::with_clock(a_set, Box::new(SimClock::new(1_000))).expect("app a");
    let mut b = PlanetaryApp::with_clock(b_set, Box::new(SimClock::new(1_000))).expect("app b");
    let (mut ra, mut rb) = (runtime(), runtime());
    ra.setup(&mut a).expect("setup a");
    rb.setup(&mut b).expect("setup b");
    let mut skips = Vec::new();
    for f in 0..frames {
        if let Some(p) = poses {
            a.set_pose_override(p[f]);
            b.set_pose_override(p[f]);
        }
        ra.frame(&mut a, DT).expect("frame a");
        rb.frame(&mut b, DT).expect("frame b");
        let (sa, skipped_a) = snap(&a);
        let (sb, skipped_b) = snap(&b);
        assert!(!skipped_a, "{kind:?} frame {f}: always_reselect skipped");
        let ctx = format!("{kind:?} frame {f}: skipping changed");
        assert_eq!(sa.wanted_split, sb.wanted_split, "{ctx} the wanted split");
        assert_eq!(sa.wanted_raw, sb.wanted_raw, "{ctx} the wanted pieces");
        assert_eq!(
            sa.wanted_levels, sb.wanted_levels,
            "{ctx} the wanted levels"
        );
        assert_eq!(sa.drawn_raw, sb.drawn_raw, "{ctx} the drawn set");
        assert_eq!(sa.report, sb.report, "{ctx} the report");
        skips.push(skipped_b);
    }
    a.shutdown();
    b.shutdown();
    skips
}

fn script_frames(s: &DescentScript, hover: usize) -> usize {
    (s.descent_seconds / DT).ceil() as usize + hover
}

fn scripted(kind: TopologyKind, fast: bool) -> Vec<bool> {
    let r = settings(kind, true).terrain.terrain.radius;
    let s = if fast {
        DescentScript::fast_free_flight(r)
    } else {
        DescentScript::orbit_to_surface(r)
    };
    let n = script_frames(&s, 120);
    let skips = compare(kind, Some(s), None, n);
    let descent = n - 120;
    // Hover: once converged the selection is a fixed point and the camera is still.
    let hover = &skips[descent + 20..];
    let skipped = hover.iter().filter(|&&x| x).count();
    println!(
        "{kind:?} fast={fast}: {} of {} frames skipped, {skipped} of {} hover frames",
        skips.iter().filter(|&&x| x).count(),
        skips.len(),
        hover.len()
    );
    skips
}

#[test]
fn scripted_descent_hex() {
    let skips = scripted(TopologyKind::Hex, false);
    assert!(
        skips.iter().any(|&s| s),
        "the skip never fired in the hover"
    );
}

#[test]
fn scripted_descent_cube() {
    let skips = scripted(TopologyKind::Cube, false);
    assert!(
        skips.iter().any(|&s| s),
        "the skip never fired in the hover"
    );
}

#[test]
fn fast_free_flight_hex() {
    let skips = scripted(TopologyKind::Hex, true);
    assert!(
        skips.iter().any(|&s| s),
        "the skip never fired in the hover"
    );
}

#[test]
fn fast_free_flight_cube() {
    let skips = scripted(TopologyKind::Cube, true);
    assert!(
        skips.iter().any(|&s| s),
        "the skip never fired in the hover"
    );
}

// ---------------------------------------------------------------------------------------------
// Seeded randomised free flight.

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn unit_f(state: &mut u64) -> f64 {
    (splitmix(state) >> 11) as f64 / (1u64 << 53) as f64
}

fn unit_vec(state: &mut u64) -> DVec3 {
    loop {
        let v = DVec3::new(
            unit_f(state) * 2.0 - 1.0,
            unit_f(state) * 2.0 - 1.0,
            unit_f(state) * 2.0 - 1.0,
        );
        let l = v.length();
        if l > 0.1 && l <= 1.0 {
            return v / l;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Stationary,
    TinyMove,
    Rotate,
    SharpTurn,
    Teleport,
    Move,
}

/// One frame of the random path and what the frame is for.
struct Step {
    pose: DVec3,
    orient: DQuat,
    kind: Kind,
    /// First frame of the segment (the frame the change happens on).
    first: bool,
}

fn look(fwd: DVec3, roll: f64) -> DQuat {
    DQuat::from_axis_angle(fwd, roll) * DQuat::from_rotation_arc(DVec3::NEG_Z, fwd)
}

fn random_path(radius: f64, seed: u64, frames: usize) -> Vec<Step> {
    let mut st = seed;
    let altitudes = [2.0e5, 2.0e4, 3.0e3, 4.0e2];
    let mut dir = unit_vec(&mut st);
    let mut alt = altitudes[2];
    let mut fwd = (-dir + unit_vec(&mut st) * 0.4).normalize();
    let mut roll = 0.0f64;
    let mut out: Vec<Step> = Vec::new();
    let schedule = [
        (Kind::Stationary, 170),
        (Kind::TinyMove, 40),
        (Kind::Stationary, 60),
        (Kind::Rotate, 40),
        (Kind::Stationary, 60),
        (Kind::SharpTurn, 30),
        (Kind::Teleport, 170),
        (Kind::Move, 30),
        (Kind::Stationary, 60),
        (Kind::Teleport, 120),
        (Kind::Rotate, 30),
    ];
    'outer: loop {
        for (kind, n) in schedule {
            for i in 0..n {
                if out.len() >= frames {
                    break 'outer;
                }
                let first = i == 0;
                match kind {
                    Kind::Stationary => {}
                    Kind::TinyMove => {
                        let side = fwd.cross(dir).normalize();
                        alt += 2.0e-4 * (1.0 + unit_f(&mut st));
                        dir = (dir + side * (3.0e-4 / (radius + alt))).normalize();
                    }
                    Kind::Rotate => {
                        roll += 1.0e-4;
                        fwd = (DQuat::from_axis_angle(dir, 2.0e-4) * fwd).normalize();
                    }
                    Kind::SharpTurn => {
                        if first {
                            let axis = dir;
                            fwd = (DQuat::from_axis_angle(axis, core::f64::consts::FRAC_PI_2)
                                * fwd)
                                .normalize();
                            roll += 0.7;
                        }
                    }
                    Kind::Teleport => {
                        if first {
                            dir = unit_vec(&mut st);
                            alt = altitudes[usize::try_from(splitmix(&mut st) % 4).expect("i")];
                            fwd = (-dir + unit_vec(&mut st) * 0.5).normalize();
                        }
                    }
                    Kind::Move => {
                        let side = fwd.cross(dir).normalize();
                        alt = (alt * 0.97).max(50.0);
                        dir = (dir + side * (4.0 / (radius + alt))).normalize();
                    }
                }
                out.push(Step {
                    pose: dir * (radius + alt),
                    orient: look(fwd, roll),
                    kind,
                    first,
                });
            }
        }
    }
    out
}

fn random_flight(kind: TopologyKind, seed: u64) {
    let radius = settings(kind, true).terrain.terrain.radius;
    let steps = random_path(radius, seed, 600);
    let poses: Vec<Pose> = steps.iter().map(|s| Some((s.pose, s.orient))).collect();
    let skips = compare(kind, None, Some(&poses), steps.len());
    // The frame a teleport happens on can never reuse the old selection.
    for (i, s) in steps.iter().enumerate() {
        if s.kind == Kind::Teleport && s.first {
            assert!(!skips[i], "{kind:?}: skipped on the teleport frame {i}");
        }
    }
    // Stationary and small-motion frames, after the first 120 frames of each segment have let the
    // streaming converge (a segment shorter than that counts its tail).
    let mut calm = 0usize;
    let mut calm_skipped = 0usize;
    for (i, s) in steps.iter().enumerate() {
        if matches!(s.kind, Kind::Stationary | Kind::TinyMove | Kind::Rotate) {
            calm += 1;
            calm_skipped += usize::from(skips[i]);
        }
    }
    let total = skips.iter().filter(|&&x| x).count();
    println!(
        "{kind:?} seed {seed}: {total} of {} frames skipped; calm {calm_skipped} of {calm}",
        skips.len()
    );
    assert!(
        calm_skipped * 5 >= calm,
        "{kind:?}: only {calm_skipped} of {calm} stationary / small-motion frames skipped (< 20 %)"
    );
}

#[test]
fn randomised_flight_hex() {
    random_flight(TopologyKind::Hex, 0x07_1027);
}

#[test]
fn randomised_flight_cube() {
    random_flight(TopologyKind::Cube, 0x07_1028);
}

#[test]
fn randomised_flight_hex_second_seed() {
    random_flight(TopologyKind::Hex, 0xC0FFEE);
}
