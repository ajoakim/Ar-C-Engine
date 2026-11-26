//! SSE selection: orbit/surface, hysteresis, monotone approach, leaf cap, determinism.

use std::collections::BTreeSet;

use engine_sdk::DVec3;
use planetary_core::PatchId;
use planetary_core::lod::{
    LodModel, LodParams, View, is_balanced, is_partition, screen_space_error, select,
};

const R: f64 = 6_371_000.0;
const RELIEF: f64 = 3800.0;

fn model() -> LodModel {
    LodModel {
        radius: R,
        max_relief: RELIEF,
        cells_per_edge: 32,
        kappa: 0.25,
    }
}

fn view_at(eye: DVec3) -> View {
    View {
        eye,
        proj_scale: 720.0 / (2.0 * (1.0 / 3.0f64.sqrt())),
    }
}

fn x_eye(dist: f64) -> DVec3 {
    DVec3::new(dist, 0.0, 0.0)
}

#[test]
fn orbit_is_coarse() {
    let sel = select(
        &model(),
        &LodParams::default(),
        &view_at(x_eye(2.0 * R)),
        &BTreeSet::new(),
    );
    assert!(sel.leaves.len() <= 200);
    assert!(sel.leaves.iter().all(|l| l.level() <= 4));
    assert!(is_partition(&sel.leaves));
    assert!(is_balanced(&sel.leaves));
    assert_eq!(sel.stats.tau_steps, 0);
}

#[test]
fn surface_is_fine_and_bounded() {
    let p = LodParams::default();
    let sel = select(
        &model(),
        &p,
        &view_at(x_eye(R + RELIEF + 100.0)),
        &BTreeSet::new(),
    );
    assert!(
        sel.leaves.iter().any(|l| l.level() == p.max_level) || sel.stats.tau_steps > 0,
        "{:?}",
        sel.stats
    );
    // Inside the relief-inflated bounds the distance clamps to min_distance, so the leaf cap is
    // best-effort: bounded tau escalation, then the (documented) final tau is accepted.
    assert!(sel.leaves.len() <= p.max_leaves || sel.stats.tau_steps == p.max_tau_steps);
    assert!(is_partition(&sel.leaves));
    assert!(is_balanced(&sel.leaves));
}

#[test]
fn hysteresis_band_has_zero_toggles() {
    let m = model();
    let p = LodParams::default();
    let level = 3u8;
    let id = planetary_core::locate(DVec3::new(1.0, 0.0, 0.0), level);
    let b = m.node_bound(id);
    let dir = planetary_core::patch_center_dir(id);
    let eye_at = |d: f64| {
        let s = R + b.radius + d;
        DVec3::new(dir.x * s, dir.y * s, dir.z * s)
    };
    // rho = eps * proj / d with d the distance beyond the bound.
    let probe = view_at(eye_at(1.0));
    let d_star = m.geometric_error(level) * probe.proj_scale / p.tau_px;
    let at = screen_space_error(&m, &view_at(eye_at(d_star)), id, p.min_distance);
    assert!((at - p.tau_px).abs() < 1e-6 * p.tau_px, "{at}");
    let mut prev = BTreeSet::new();
    let mut membership = Vec::new();
    for f in 0..100 {
        let k = if f % 2 == 0 { 0.95 } else { 1.05 };
        let sel = select(&m, &p, &view_at(eye_at(d_star * k)), &prev);
        membership.push(sel.split.contains(&id));
        prev = sel.split;
    }
    assert!(membership[0], "node must split at 0.95 d*");
    assert!(membership.iter().all(|&b| b), "toggled: {membership:?}");
}

#[test]
fn monotone_approach_never_unsplits() {
    let m = model();
    let p = LodParams::default();
    let (far, near) = (3.0 * R, R + RELIEF + 50.0);
    let mut prev = BTreeSet::new();
    let mut prev_steps = 0u32;
    for f in 0..300 {
        let t = f64::from(f) / 299.0;
        // geometric interpolation without pow: exponentiation by repeated ratio
        let mut d = far;
        let ratio = near / far;
        let steps = f;
        let r1 = ratio_root(ratio, 299);
        for _ in 0..steps {
            d *= r1;
        }
        let _ = t;
        let sel = select(&m, &p, &view_at(x_eye(d)), &prev);
        // Tau escalation (leaf cap) legitimately coarsens; monotonicity applies while it is idle.
        if sel.stats.tau_steps == 0 && prev_steps == 0 {
            assert!(prev.is_subset(&sel.split), "frame {f} removed a split");
        }
        prev_steps = sel.stats.tau_steps;
        prev = sel.split;
    }
}

/// n-th root by bisection (no libm).
fn ratio_root(x: f64, n: u32) -> f64 {
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        let mut v = 1.0;
        for _ in 0..n {
            v *= mid;
        }
        if v < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[test]
fn leaf_cap_escalates_tau() {
    let p = LodParams {
        max_leaves: 60,
        ..LodParams::default()
    };
    let sel = select(
        &model(),
        &p,
        &view_at(x_eye(R + RELIEF + 100.0)),
        &BTreeSet::new(),
    );
    assert!(sel.stats.tau_steps >= 1);
    assert!(sel.stats.tau_eff > p.tau_px);
    assert!(sel.leaves.len() <= 60 || sel.stats.tau_steps == p.max_tau_steps);
    assert!(is_partition(&sel.leaves));
}

#[test]
fn deterministic() {
    let v = view_at(x_eye(R + RELIEF + 1000.0));
    let a = select(&model(), &LodParams::default(), &v, &BTreeSet::new());
    let b = select(&model(), &LodParams::default(), &v, &BTreeSet::new());
    assert_eq!(a, b);
}

#[test]
fn work_is_counted_and_bounded() {
    for d in [2.0 * R, R + RELIEF + 1000.0, R + RELIEF + 50.0] {
        let sel = select(
            &model(),
            &LodParams::default(),
            &view_at(x_eye(d)),
            &BTreeSet::new(),
        );
        assert!(sel.stats.nodes_visited > 0 && sel.stats.nodes_visited <= 50_000);
        assert_eq!(sel.stats.leaves, sel.leaves.len());
    }
}

#[test]
fn view_new_rejects_bad_input() {
    let ok = DVec3::new(1.0, 2.0, 3.0);
    assert!(View::new(ok, 720, 0.5).is_some());
    assert!(View::new(DVec3::new(f64::NAN, 0.0, 0.0), 720, 0.5).is_none());
    assert!(View::new(ok, 0, 0.5).is_none());
    assert!(View::new(ok, 720, 0.0).is_none());
    assert!(View::new(ok, 720, -1.0).is_none());
    assert!(View::new(ok, 720, f64::INFINITY).is_none());
}

#[test]
fn prev_split_unused_ids_are_harmless() {
    // A stale prev set containing unrelated nodes must not break partition/balance.
    let mut prev = BTreeSet::new();
    prev.insert(PatchId::root(planetary_core::Face::NegZ));
    let sel = select(
        &model(),
        &LodParams::default(),
        &view_at(x_eye(2.0 * R)),
        &prev,
    );
    assert!(is_partition(&sel.leaves) && is_balanced(&sel.leaves));
}
