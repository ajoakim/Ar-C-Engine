//! SSE selection: orbit/surface, hysteresis, budgets, culling, determinism.

use std::collections::BTreeSet;

use engine_sdk::DVec3;
use planetary_core::lod::{
    HeightRange, LodModel, LodParams, SelectStats, View, cell_bound, is_balanced, is_partition,
    patch_footprint, reference_views, screen_space_error,
};
use planetary_core::topology::select::{Retained, retain_culled, select_cells};
use planetary_core::{CubeSphere, PatchId};

/// Cube-sphere selection through the generic `select_cells` (the `lod::select` wrapper is gone).
#[derive(Debug, PartialEq)]
struct Selection {
    split: BTreeSet<PatchId>,
    leaves: Vec<PatchId>,
    stats: SelectStats,
}

fn select(
    m: &LodModel,
    p: &LodParams,
    v: &View,
    prev: &BTreeSet<PatchId>,
    resident: &dyn Fn(PatchId) -> Option<HeightRange>,
) -> Selection {
    let s = select_cells(&CubeSphere::new(m.cells_per_edge), m, p, v, prev, resident);
    Selection {
        split: s.split,
        leaves: s.pieces.iter().map(|q| q.cell).collect(),
        stats: s.stats,
    }
}

const R: f64 = 6_371_000.0;
const RELIEF: f64 = 3800.0;

fn model() -> LodModel {
    LodModel {
        radius: R,
        height: HeightRange {
            min: -RELIEF,
            max: RELIEF,
        },
        residual: [0.0; 25],
        cells_per_edge: 32,
        sample_angle0: core::f64::consts::FRAC_PI_2 / 32.0,
        kappa: 0.25,
    }
}

fn view_at(eye: DVec3) -> View {
    View {
        eye,
        proj_scale: 720.0 / (2.0 * (1.0 / 3.0f64.sqrt())),
        frustum: None,
    }
}

fn x_eye(dist: f64) -> DVec3 {
    DVec3::new(dist, 0.0, 0.0)
}

fn sel(m: &LodModel, p: &LodParams, v: &View, prev: &BTreeSet<PatchId>) -> Selection {
    select(m, p, v, prev, &|_| None)
}

#[test]
fn orbit_is_coarse() {
    let s = sel(
        &model(),
        &LodParams::default(),
        &view_at(x_eye(2.0 * R)),
        &BTreeSet::new(),
    );
    assert!(s.leaves.len() <= 200);
    assert!(s.leaves.iter().all(|l| l.level() <= 4));
    assert!(is_partition(&s.leaves));
    assert!(is_balanced(&s.leaves));
}

#[test]
fn surface_reaches_max_level_within_budgets() {
    let p = LodParams::default();
    // Eye 3 m above the global top of the relief: the cell below has a (conservative) height
    // range up to the top, so refinement still converges to the maximum level there.
    for h in [3.0, 100.0, 1000.0] {
        let s = sel(
            &model(),
            &p,
            &view_at(x_eye(R + RELIEF + h)),
            &BTreeSet::new(),
        );
        let under = planetary_core::locate(DVec3::new(1.0, 0.0, 0.0), p.max_level);
        if h <= 3.0 {
            assert!(s.leaves.contains(&under), "{:?}", s.stats);
        }
        assert!(
            s.leaves.iter().any(|l| l.level() + 3 >= p.max_level),
            "h {h} {:?}",
            s.stats
        );
        assert!(
            s.stats.nodes_visited <= p.max_nodes_visited,
            "{:?}",
            s.stats
        );
        assert!(s.leaves.len() <= p.max_leaves, "{:?}", s.stats);
        assert!(is_partition(&s.leaves));
        assert!(is_balanced(&s.leaves));
    }
}

#[test]
fn hysteresis_band_has_zero_toggles() {
    let m = model();
    let p = LodParams::default();
    let level = 3u8;
    let id = planetary_core::locate(DVec3::new(1.0, 0.0, 0.0), level);
    let dir = planetary_core::patch_center_dir(id);
    let eye_at = |d: f64| {
        let s = R + m.height.max + d;
        DVec3::new(dir.x * s, dir.y * s, dir.z * s)
    };
    // Straight above the center, the distance is the gap above the cell's top.
    let probe = view_at(eye_at(1.0));
    let d_star = m.geometric_error(level) * probe.proj_scale / p.tau_px;
    let at = screen_space_error(&m, &view_at(eye_at(d_star)), id, m.height, p.min_distance);
    assert!((at - p.tau_px).abs() < 1e-6 * p.tau_px, "{at}");
    let mut prev = BTreeSet::new();
    let mut membership = Vec::new();
    for f in 0..100 {
        let k = if f % 2 == 0 { 0.95 } else { 1.05 };
        let s = sel(&m, &p, &view_at(eye_at(d_star * k)), &prev);
        membership.push(s.split.contains(&id));
        prev = s.split;
    }
    assert!(membership[0], "node must split at 0.95 d*");
    assert!(membership.iter().all(|&b| b), "toggled: {membership:?}");
}

#[test]
fn bound_covers_cell_vertices() {
    let m = model();
    let id = planetary_core::locate(DVec3::new(1.0, 0.0, 0.0), 3);
    let b = cell_bound(R, &patch_footprint(id), m.height);
    for d in planetary_core::patch_corner_dirs(id) {
        for h in [-RELIEF, RELIEF] {
            let p = DVec3::new(d.x * (R + h), d.y * (R + h), d.z * (R + h));
            assert!((p - b.center).length() <= b.radius);
        }
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
fn descent_is_balanced_partition_within_budgets() {
    let m = model();
    let p = LodParams::default();
    let mut prev = BTreeSet::new();
    let (far, near) = (2.0 * R, R + 3.0);
    let r1 = ratio_root(near / far, 199);
    let mut d = far;
    for f in 0..200 {
        let s = sel(&m, &p, &view_at(x_eye(d)), &prev);
        assert!(is_partition(&s.leaves), "frame {f}");
        assert!(is_balanced(&s.leaves), "frame {f}");
        assert!(s.leaves.len() <= p.max_leaves, "frame {f} {:?}", s.stats);
        assert!(s.stats.nodes_visited <= p.max_nodes_visited, "frame {f}");
        prev = s.split;
        d *= r1;
    }
}

#[test]
fn node_budget_keeps_invariants() {
    let p = LodParams {
        max_nodes_visited: 50,
        ..LodParams::default()
    };
    let s = sel(
        &model(),
        &p,
        &view_at(x_eye(R + RELIEF + 100.0)),
        &BTreeSet::new(),
    );
    assert!(s.stats.node_budget_hit, "{:?}", s.stats);
    assert!(s.stats.nodes_visited <= 50);
    assert!(is_partition(&s.leaves) && is_balanced(&s.leaves));
}

#[test]
fn leaf_budget_keeps_invariants() {
    let p = LodParams {
        max_leaves: 60,
        ..LodParams::default()
    };
    let s = sel(
        &model(),
        &p,
        &view_at(x_eye(R + RELIEF + 100.0)),
        &BTreeSet::new(),
    );
    assert!(s.stats.leaf_budget_hit, "{:?}", s.stats);
    assert!(s.leaves.len() <= 60, "{}", s.leaves.len());
    assert!(is_partition(&s.leaves) && is_balanced(&s.leaves));
}

#[test]
fn horizon_cells_never_split() {
    let m = model();
    let p = LodParams::default();
    let s = sel(&m, &p, &view_at(x_eye(R + 10_000.0)), &BTreeSet::new());
    assert!(s.stats.culled_horizon > 0, "{:?}", s.stats);
    // The antipodal hemisphere must stay coarse.
    for l in &s.leaves {
        let c = planetary_core::patch_center_dir(*l);
        if c.x < -0.5 {
            assert!(l.level() <= 4, "{l:?}");
        }
    }
}

#[test]
fn frustum_culls_behind_camera() {
    let m = model();
    let p = LodParams::default();
    let eye = x_eye(R + 100_000.0);
    // Looking away from the planet (+x): near plane at the eye, inward normals along +x.
    let planes = [
        [1.0, 0.0, 0.0, -eye.x],
        [0.0, 1.0, 0.0, 1e12],
        [0.0, -1.0, 0.0, 1e12],
        [0.0, 0.0, 1.0, 1e12],
        [0.0, 0.0, -1.0, 1e12],
        [-1.0, 0.0, 0.0, 1e12],
    ];
    let with = sel(&m, &p, &view_at(eye).with_frustum(planes), &BTreeSet::new());
    let without = sel(&m, &p, &view_at(eye), &BTreeSet::new());
    assert!(with.stats.culled_frustum > 0, "{:?}", with.stats);
    assert!(with.leaves.len() < without.leaves.len());
    // The cell under the camera is never culled: it keeps its level.
    let under = |s: &Selection| {
        s.leaves
            .iter()
            .find(|l| l.is_ancestor_of(planetary_core::locate(DVec3::new(1.0, 0.0, 0.0), 16)))
            .map(|l| l.level())
    };
    assert_eq!(under(&with), under(&without));
    assert!(is_partition(&with.leaves) && is_balanced(&with.leaves));
}

#[test]
fn reference_views_are_valid() {
    let m = model();
    let p = LodParams::default();
    for (name, eye) in reference_views(R, RELIEF) {
        let s = sel(&m, &p, &view_at(eye), &BTreeSet::new());
        assert!(is_partition(&s.leaves) && is_balanced(&s.leaves), "{name}");
        assert!(s.leaves.len() <= p.max_leaves, "{name}");
        assert!(s.stats.nodes_visited <= p.max_nodes_visited, "{name}");
    }
}

#[test]
fn deterministic() {
    let v = view_at(x_eye(R + 1000.0));
    let a = sel(&model(), &LodParams::default(), &v, &BTreeSet::new());
    let b = sel(&model(), &LodParams::default(), &v, &BTreeSet::new());
    assert_eq!(a, b);
}

#[test]
fn work_is_counted_and_bounded() {
    for d in [2.0 * R, R + 1000.0, R + 50.0] {
        let s = sel(
            &model(),
            &LodParams::default(),
            &view_at(x_eye(d)),
            &BTreeSet::new(),
        );
        assert!(s.stats.nodes_visited > 0 && s.stats.nodes_visited <= 8192);
        assert_eq!(s.stats.leaves, s.leaves.len());
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
    let s = sel(
        &model(),
        &LodParams::default(),
        &view_at(x_eye(2.0 * R)),
        &prev,
    );
    assert!(is_partition(&s.leaves) && is_balanced(&s.leaves));
}

/// Prints node/leaf/level numbers for the real terrain model (run with `--nocapture`); with
/// global ranges and with ranges measured from the terrain for every cell (an ideal resident set).
#[test]
fn reference_stats_report() {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use planetary_core::{PlanetTerrain, TerrainParams, vertex_dir};
    let t = PlanetTerrain::new(7, TerrainParams::default(), 32).expect("terrain");
    let m = LodModel::from_terrain(&t, planetary_core::lod::DEFAULT_KAPPA);
    let p = LodParams::default();
    let cache: RefCell<BTreeMap<PatchId, HeightRange>> = RefCell::new(BTreeMap::new());
    let measured = |id: PatchId| -> Option<HeightRange> {
        if id.level() < 6 {
            return None;
        }
        let mut c = cache.borrow_mut();
        Some(*c.entry(id).or_insert_with(|| {
            let mut r = HeightRange {
                min: f64::INFINITY,
                max: f64::NEG_INFINITY,
            };
            for j in 0..=32 {
                for i in 0..=32 {
                    let h = t.height_at_dir(vertex_dir(id, 32, i, j));
                    r.min = r.min.min(h);
                    r.max = r.max.max(h);
                }
            }
            r
        }))
    };
    for h in [3.0, 1000.0, 100_000.0] {
        let eye = x_eye(m.radius + m.height.max + h);
        let v = view_at(eye);
        let g = select(&m, &p, &v, &BTreeSet::new(), &|_| None);
        let top = |s: &Selection| s.leaves.iter().map(|l| l.level()).max().unwrap_or(0);
        eprintln!(
            "STATS h={h} global: nodes={} leaves={} max_level={} forced={} trimmed={} node_budget={} leaf_budget={}",
            g.stats.nodes_visited,
            g.leaves.len(),
            top(&g),
            g.stats.forced_splits,
            g.stats.trimmed_splits,
            g.stats.node_budget_hit,
            g.stats.leaf_budget_hit
        );
        if h <= 1000.0 {
            let r = select(&m, &p, &v, &BTreeSet::new(), &measured);
            eprintln!(
                "STATS h={h} resident: nodes={} leaves={} max_level={} forced={} trimmed={}",
                r.stats.nodes_visited,
                r.leaves.len(),
                top(&r),
                r.stats.forced_splits,
                r.stats.trimmed_splits
            );
        }
    }
}

/// 07.1-20: `retain_culled` keeps a cell that was split last frame and is culled this frame
/// split (as long as the pieces fit the budget), so it is not split again when it returns.
fn away_view(eye: DVec3) -> View {
    // Looking away from the planet: everything but the cell under the eye is culled.
    let planes = [
        [1.0, 0.0, 0.0, -eye.x],
        [0.0, 1.0, 0.0, 1e12],
        [0.0, -1.0, 0.0, 1e12],
        [0.0, 0.0, 1.0, 1e12],
        [0.0, 0.0, -1.0, 1e12],
        [-1.0, 0.0, 0.0, 1e12],
    ];
    view_at(eye).with_frustum(planes)
}

fn retain(
    m: &LodModel,
    p: &LodParams,
    v: &View,
    prev: &BTreeSet<PatchId>,
) -> (BTreeSet<PatchId>, Retained<PatchId>) {
    let topo = CubeSphere::new(m.cells_per_edge);
    let mut split = select_cells(&topo, m, p, v, prev, &|_| None).split;
    let r = retain_culled(&topo, m, p, v, prev, &|_| None, &mut split);
    (split, r)
}

fn leaves_of_split(m: &LodModel, split: &BTreeSet<PatchId>) -> Vec<PatchId> {
    let topo = CubeSphere::new(m.cells_per_edge);
    let mut pieces = Vec::new();
    planetary_core::Topology::pieces(&topo, split, &mut pieces);
    pieces.iter().map(|q| q.cell).collect()
}

#[test]
fn culled_previously_split_cells_stay_split() {
    let m = model();
    let p = LodParams::default();
    let eye = x_eye(R + 100_000.0);
    let first = sel(&m, &p, &view_at(eye), &BTreeSet::new());
    assert!(
        first.split.len() > 6,
        "the first frame must refine something: {}",
        first.split.len()
    );
    let away = away_view(eye);
    // select_cells itself still drops culled cells (retention is a separate step).
    let plain = sel(&m, &p, &away, &first.split);
    assert!(plain.stats.culled_frustum > 0, "{:?}", plain.stats);
    assert!(first.split.difference(&plain.split).count() > 0);
    let (split, r) = retain(&m, &p, &away, &first.split);
    let dropped: Vec<_> = first.split.difference(&split).collect();
    assert!(
        dropped.is_empty(),
        "{} previously split cells were dropped because they were culled: {dropped:?}",
        dropped.len()
    );
    assert!(!r.cells.is_empty());
    let leaves = leaves_of_split(&m, &split);
    assert!(is_partition(&leaves) && is_balanced(&leaves));
    assert!(leaves.len() <= p.max_leaves);
    // Deterministic.
    assert_eq!(
        (split.clone(), r.clone()),
        retain(&m, &p, &away, &first.split)
    );
    // No candidates: nothing to retain.
    let (_, none) = retain(&m, &p, &away, &BTreeSet::new());
    assert!(none.cells.is_empty() && none.rejected == 0);
}

/// The retained cells never push the selection over the piece budget: when they do not fit they
/// are dropped (what is visible keeps the budget).
#[test]
fn retained_culled_cells_respect_the_leaf_budget() {
    let m = model();
    let eye = x_eye(R + 100_000.0);
    let big = LodParams::default();
    let first = sel(&m, &big, &view_at(eye), &BTreeSet::new());
    let away = away_view(eye);
    let tight = LodParams {
        max_leaves: 6 + 3,
        ..big
    };
    let (split, r) = retain(&m, &tight, &away, &first.split);
    let leaves = leaves_of_split(&m, &split);
    assert!(leaves.len() <= tight.max_leaves, "{} pieces", leaves.len());
    assert!(r.rejected > 0, "{r:?}");
    assert!(is_partition(&leaves) && is_balanced(&leaves));
}

fn hex_model() -> LodModel {
    let hex = planetary_core::HexSphere::new(16).expect("hex");
    LodModel {
        sample_angle0: planetary_core::Topology::sample_angle0(&hex),
        cells_per_edge: 16,
        ..model()
    }
}

#[test]
fn refine_distance_is_the_selector_threshold() {
    use planetary_core::lod::refine_distance;
    let view = view_at(x_eye(2.0 * R));
    let tau = LodParams::default().tau_px;
    for m in [model(), hex_model()] {
        for level in 1..=16u8 {
            let d = refine_distance(&m, view.proj_scale, tau, level);
            let rho = m.geometric_error(level) * view.proj_scale / d;
            assert!((rho - tau).abs() <= 1e-9 * tau, "level {level}: {rho}");
        }
    }
    // Same construction as the selector test: the real screen-space error at the distance.
    let m = model();
    let id = planetary_core::locate(DVec3::new(1.0, 0.0, 0.0), 5);
    let dir = planetary_core::patch_center_dir(id);
    let d = refine_distance(&m, view.proj_scale, tau, 5);
    let s = R + m.height.max + d;
    let eye = DVec3::new(dir.x * s, dir.y * s, dir.z * s);
    let at = screen_space_error(&m, &view_at(eye), id, m.height, 1.0);
    assert!((at - tau).abs() < 1e-6 * tau, "{at}");
    assert_eq!(refine_distance(&m, 1.0, 0.0, 3), f64::INFINITY);
}

#[test]
fn morph_factor_shape_and_guards() {
    use planetary_core::lod::morph_factor;
    assert_eq!(morph_factor(10.0, 10.0, 5.0), 1.0);
    assert_eq!(morph_factor(11.0, 10.0, 5.0), 1.0);
    assert_eq!(morph_factor(5.0, 10.0, 5.0), 0.0);
    assert_eq!(morph_factor(1.0, 10.0, 5.0), 0.0);
    assert_eq!(morph_factor(7.5, 10.0, 5.0), 0.5);
    let mut prev = 1.0;
    for i in (0..=200).rev() {
        let f = morph_factor(f64::from(i) * 0.1, 10.0, 5.0);
        assert!((0.0..=1.0).contains(&f) && f <= prev);
        prev = f;
    }
    assert_eq!(morph_factor(7.0, 5.0, 10.0), 0.0);
    assert_eq!(morph_factor(7.0, 5.0, 5.0), 0.0);
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(morph_factor(bad, 10.0, 5.0), 0.0);
        assert_eq!(morph_factor(7.0, bad, 5.0), 0.0);
        assert_eq!(morph_factor(7.0, 10.0, bad), 0.0);
    }
}

#[test]
fn morph_chain_parent_finished_where_children_start() {
    use planetary_core::lod::{morph_factor, refine_distance};
    let view = view_at(x_eye(2.0 * R));
    let tau = LodParams::default().tau_px;
    for m in [model(), hex_model()] {
        for l in 2..=16u8 {
            let split = refine_distance(&m, view.proj_scale, tau, l - 1);
            let parent_end = 1.25 * split;
            assert!(parent_end >= split);
            // Parent (level l-1) is fully morphed at its split distance; the child (level l)
            // starts at 1 there; each level's ramp is non-degenerate.
            let p_start = refine_distance(&m, view.proj_scale, tau, l - 2);
            assert_eq!(morph_factor(split, p_start, parent_end), 0.0);
            let c_end = 1.25 * refine_distance(&m, view.proj_scale, tau, l);
            assert_eq!(morph_factor(split, split, c_end), 1.0);
            assert!(c_end < split, "level {l}: ramp must be non-degenerate");
        }
    }
}
