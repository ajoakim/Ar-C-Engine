//! 07.1-27: the slack recorded by `select_cells` is sound (a view change it allows never changes
//! the selection), has teeth (changes far beyond it do change the selection and are refused), and
//! its predicate refuses everything it cannot prove.

use std::collections::BTreeSet;

use engine_sdk::{DQuat, DVec3};
use planetary_core::lod::{
    HeightRange, LodModel, LodParams, SelectSlack, View, cell_distance, patch_footprint,
};
use planetary_core::topology::select::{CellSelection, select_cells};
use planetary_core::{CubeSphere, Face, HexSphere, PatchId, Topology};

const R: f64 = 6_371_000.0;
const TAN_HALF: f64 = 0.577_350_269_189_625_8;
const PROJ: f64 = 720.0 / (2.0 * TAN_HALF);

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

/// World-space frustum planes (inward normals) for a camera at `eye` looking along `fwd`.
fn planes(eye: DVec3, fwd: DVec3, up_hint: DVec3, near: f64) -> [[f64; 4]; 6] {
    let f = fwd.normalize();
    let right = f.cross(up_hint).normalize();
    let up = right.cross(f).normalize();
    let aspect = 16.0 / 9.0;
    let tx = TAN_HALF * aspect;
    let ty = TAN_HALF;
    let side = |n: DVec3| {
        let n = n.normalize();
        [n.x, n.y, n.z, -n.dot(eye)]
    };
    [
        side(right + f * tx),
        side(-right + f * tx),
        side(up + f * ty),
        side(-up + f * ty),
        [f.x, f.y, f.z, -f.dot(eye) - near],
        [-f.x, -f.y, -f.z, f.dot(eye) + 1.0e8],
    ]
}

fn model_for(t: &impl Topology) -> LodModel {
    LodModel {
        radius: R,
        height: HeightRange {
            min: -3800.0,
            max: 3800.0,
        },
        residual: [0.0; 25],
        cells_per_edge: 16,
        sample_angle0: t.sample_angle0(),
        kappa: 0.25,
    }
}

fn params(max_level: u8) -> LodParams {
    LodParams {
        max_level,
        ..LodParams::default()
    }
}

/// A camera at `altitude` above the direction `dir`, looking toward the ground ahead of it.
fn camera(dir: DVec3, altitude: f64, tilt: f64) -> (DVec3, DVec3, DVec3) {
    let eye = dir * (R + altitude);
    let side = dir.cross(DVec3::Y).normalize();
    let fwd = (-dir * (1.0 - tilt) + side * tilt).normalize();
    (eye, fwd, dir)
}

fn same<C: Copy + Ord>(a: &CellSelection<C>, b: &CellSelection<C>) -> bool {
    a.split == b.split && a.stats == b.stats && a.pieces == b.pieces
}

struct Tally {
    allowed: usize,
    differing: usize,
    refused: usize,
    clean: usize,
}

fn run_scenarios<T: Topology>(topo: &T, max_level: u8, seed: u64) -> Tally {
    let model = model_for(topo);
    let p = params(max_level);
    let mut st = seed;
    let mut tally = Tally {
        allowed: 0,
        differing: 0,
        refused: 0,
        clean: 0,
    };
    let altitudes = [4.0e5, 3.0e4, 2.0e3, 150.0, 12.0];
    for (case, &alt) in altitudes.iter().enumerate() {
        let dir = unit_vec(&mut st);
        let tilt = 0.2 + 0.3 * unit_f(&mut st);
        let (eye, fwd, up) = camera(dir, alt, tilt);
        let near = (alt * 0.1).clamp(0.5, 1.0e4);
        let view0 = View {
            eye,
            proj_scale: PROJ,
            frustum: None,
        }
        .with_frustum(planes(eye, fwd, up, near));
        let none = |_| None;
        // Run twice so `prev_split` is the fixed point of the hysteresis.
        let first = select_cells(topo, &model, &p, &view0, &BTreeSet::new(), &none);
        let prev = first.split.clone();
        let base = select_cells(topo, &model, &p, &view0, &prev, &none);
        let s = base.stats;
        if s.node_budget_hit || s.leaf_budget_hit || s.trimmed_splits > 0 {
            continue;
        }
        tally.clean += 1;
        let slack = base.slack;
        assert!(
            slack.margin_eye >= 0.0 && slack.margin_plane >= 0.0,
            "case {case}"
        );
        let de_limit = slack.eye_cap.min(slack.margin_eye);
        let alpha_limit = if slack.d_plane_max > 0.0 {
            slack.margin_plane / slack.d_plane_max
        } else {
            0.0
        };
        println!(
            "case {case} alt {alt}: margin_eye {:.4} m, margin_plane {:.4}, d_plane_max {:.0}, eye_cap {:.1}, leaves {}",
            slack.margin_eye, slack.margin_plane, slack.d_plane_max, slack.eye_cap, s.leaves
        );
        for k in 0..160 {
            // Fractions below, around and far above the slack.
            let frac = [0.001, 0.01, 0.05, 0.2, 0.5, 0.8, 1.5, 4.0, 40.0, 4000.0][k % 10];
            let delta = unit_vec(&mut st) * (de_limit * frac * unit_f(&mut st));
            let angle = alpha_limit * frac * unit_f(&mut st);
            let rot = DQuat::from_axis_angle(unit_vec(&mut st), angle);
            let eye1 = eye + delta;
            let (fwd1, up1) = if k % 3 == 0 {
                (fwd, up)
            } else {
                (rot * fwd, rot * up)
            };
            let view1 = View {
                eye: eye1,
                proj_scale: PROJ,
                frustum: None,
            }
            .with_frustum(planes(eye1, fwd1, up1, near));
            let allowed = slack.allows(&view0, &view1);
            let re = select_cells(topo, &model, &p, &view1, &prev, &none);
            let eq = same(&base, &re);
            if allowed {
                tally.allowed += 1;
                assert!(
                    eq,
                    "case {case} k {k} frac {frac}: an allowed change altered the selection"
                );
            } else {
                tally.refused += 1;
                if !eq {
                    tally.differing += 1;
                }
            }
        }
    }
    tally
}

#[test]
fn cube_slack_is_sound_and_has_teeth() {
    let t = CubeSphere::new(16);
    let tally = run_scenarios(&t, 11, 0x07_1027);
    println!(
        "cube: clean {} allowed {} refused {} differing(refused) {}",
        tally.clean, tally.allowed, tally.refused, tally.differing
    );
    assert!(tally.clean >= 3);
    assert!(tally.allowed >= 100, "the skip predicate is vacuous");
    assert!(
        tally.differing > 0,
        "far moves never changed the selection: the test has no teeth"
    );
}

#[test]
fn hex_slack_is_sound_and_has_teeth() {
    let t = HexSphere::new(16).expect("hex");
    let tally = run_scenarios(&t, 11, 0x07_1028);
    println!(
        "hex: clean {} allowed {} refused {} differing(refused) {}",
        tally.clean, tally.allowed, tally.refused, tally.differing
    );
    assert!(tally.clean >= 3);
    assert!(tally.allowed >= 100, "the skip predicate is vacuous");
    assert!(
        tally.differing > 0,
        "far moves never changed the selection: the test has no teeth"
    );
}

#[test]
fn cell_distance_is_one_lipschitz_in_the_eye() {
    let mut st = 5;
    let range = HeightRange {
        min: -500.0,
        max: 2500.0,
    };
    for _ in 0..3000 {
        let level = u8::try_from(splitmix(&mut st) % 9).expect("level");
        let n = 1u64 << level;
        let x = u32::try_from(splitmix(&mut st) % n).expect("x");
        let y = u32::try_from(splitmix(&mut st) % n).expect("y");
        let face = Face::ALL[usize::try_from(splitmix(&mut st) % 6).expect("face")];
        let id = PatchId::new(face, level, x, y).expect("id");
        let fp = patch_footprint(id);
        let alt = 10.0 + 1.0e6 * unit_f(&mut st).powi(3);
        let e0 = unit_vec(&mut st) * (R + alt);
        let step = 1000.0 * unit_f(&mut st).powi(4);
        let e1 = e0 + unit_vec(&mut st) * step;
        let d0 = cell_distance(R, e0, &fp, range);
        let d1 = cell_distance(R, e1, &fp, range);
        let moved = (e1 - e0).length();
        assert!(
            (d0 - d1).abs() <= moved + 2.0e-2,
            "|{d0} - {d1}| > {moved} (level {level})"
        );
    }
}

#[test]
fn identical_views_always_pass_and_unprovable_ones_never() {
    let slack = SelectSlack::default();
    let eye = DVec3::new(R + 100.0, 0.0, 0.0);
    let v = View {
        eye,
        proj_scale: PROJ,
        frustum: None,
    };
    assert!(
        slack.allows(&v, &v),
        "an identical view reuses the pure result"
    );
    let good = SelectSlack {
        margin_eye: 100.0,
        margin_plane: 100.0,
        d_plane_max: 1.0e6,
        eye_cap: 50.0,
    };
    let moved = View {
        eye: eye + DVec3::new(0.0, 1.0, 0.0),
        ..v
    };
    assert!(good.allows(&v, &moved));
    assert!(!slack.allows(&v, &moved), "default slack allows no motion");
    // Beyond the margin, beyond the cap, non-finite, changed projection, frustum presence.
    let far = View {
        eye: eye + DVec3::new(0.0, 99.99, 0.0),
        ..v
    };
    assert!(!good.allows(&v, &far));
    let teleport = View {
        eye: eye + DVec3::new(0.0, 60.0, 0.0),
        ..v
    };
    let wide = SelectSlack {
        margin_eye: 1.0e9,
        ..good
    };
    assert!(!wide.allows(&v, &teleport), "past eye_cap");
    let nan = View {
        eye: DVec3::new(f64::NAN, 0.0, 0.0),
        ..v
    };
    assert!(!good.allows(&v, &nan));
    assert!(!good.allows(&nan, &nan));
    let zoom = View {
        proj_scale: PROJ * 1.000_000_1,
        ..v
    };
    assert!(!good.allows(&v, &zoom));
    let framed = v.with_frustum(planes(eye, -eye, DVec3::Y, 1.0));
    assert!(!good.allows(&v, &framed));
    assert!(!good.allows(&framed, &v));
}

#[test]
fn orientation_delta_properties() {
    let eye = DVec3::new(R + 100.0, 0.0, 0.0);
    let base = View {
        eye,
        proj_scale: PROJ,
        frustum: None,
    };
    assert_eq!(base.orientation_delta(&base), Some(0.0));
    let a = base.with_frustum(planes(eye, -DVec3::X, DVec3::Y, 1.0));
    let tilted = (-DVec3::X + DVec3::Y * 0.01).normalize();
    let b = base.with_frustum(planes(eye, tilted, DVec3::Y, 1.0));
    assert_eq!(a.orientation_delta(&a), Some(0.0));
    let ab = a.orientation_delta(&b).expect("both frustums");
    let ba = b.orientation_delta(&a).expect("both frustums");
    assert!(ab > 0.0 && ab.is_finite() && ab < 0.02);
    assert!((ab - ba).abs() < 1e-15, "symmetric");
    assert_eq!(a.orientation_delta(&base), None);
    assert_eq!(base.orientation_delta(&a), None);
    assert!(a.same_projection(&b));
    let zoomed = View {
        proj_scale: PROJ + 1.0,
        ..a
    };
    assert!(!a.same_projection(&zoomed));
}
