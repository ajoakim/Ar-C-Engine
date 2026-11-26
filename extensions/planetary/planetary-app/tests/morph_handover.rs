//! PLAN 07.1-24: continuity of the geomorph at level handover, hex and cube.
//!
//! Every frame records the drawn cells and the morph factor shown for each (the same value the
//! renderer receives). Handover events are derived from consecutive frames:
//! - refine event: a drawn cell disappears and (some of) its children appear;
//! - coarsen event: children disappear and their parent appears.
//!
//! The vertices of a child at morph 1 lie on the parent mesh surface (plan 22), so the
//! displacement at a refine event is `m_parent_shown * |own - morph|` of the parent, bounded by
//! `m_parent_shown * max parent delta`. The tests assert that bound and, for a slow approach,
//! that `m_parent_shown` is exactly 0. All assertions are counted or computed work, never
//! durations.

use std::collections::{BTreeMap, BTreeSet};

use engine_sdk::{Engine, EngineConfig, NullBackend, Runtime};
use planetary_app::{AppSettings, DescentScript, PlanetaryApp, TopologyKind};
use planetary_core::lod::LodModel;
use planetary_core::{CubeSphere, HexSphere, PlanetTerrain, Topology};
use planetary_terrain::{TerrainConfig, TerrainReport};

use TopologyKind::{Cube, Hex};

struct Frame {
    /// Drawn cells (raw id) with their shown morph factor.
    cells: BTreeMap<u64, f64>,
    /// The drawn set equals the wanted set (the initial stream-in has finished).
    converged: bool,
    report: TerrainReport,
}

fn settings_for(kind: TopologyKind) -> AppSettings {
    let mut s = match kind {
        Hex => AppSettings::default(),
        Cube => AppSettings::cube_default(),
    };
    s.terrain.settle_each_frame = true;
    s
}

fn run_script(kind: TopologyKind, script: DescentScript, frames: u32) -> Vec<Frame> {
    let mut settings = settings_for(kind);
    settings.script = Some(script);
    let mut app = PlanetaryApp::new(settings).expect("app");
    let mut rt =
        Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).expect("runtime");
    rt.setup(&mut app).expect("setup");
    let mut out = Vec::with_capacity(frames as usize);
    for _ in 0..frames {
        rt.frame(&mut app, 1.0 / 60.0).expect("frame");
        let mut wanted = app.wanted_raw();
        wanted.sort_unstable();
        let mut drawn = app.drawn_raw();
        drawn.sort_unstable();
        let converged = wanted == drawn;
        let cells = app
            .drawn_raw()
            .into_iter()
            .zip(app.drawn_morphs())
            .collect();
        out.push(Frame {
            cells,
            converged,
            report: app.report().clone(),
        });
    }
    app.shutdown();
    rt.shutdown().expect("shutdown");
    out
}

fn radius(kind: TopologyKind) -> f64 {
    settings_for(kind).terrain.terrain.radius
}

/// Constant fraction of the altitude per frame (0.5 %), no lateral motion.
fn slow_script(kind: TopologyKind) -> (DescentScript, u32) {
    let start = 1.0e6f64;
    let end = 100.0f64;
    let rate = 0.005f64;
    let frames = ((end / start).ln() / (1.0 - rate).ln()).ceil() as u32;
    let _ = radius(kind);
    (
        DescentScript {
            start_altitude: start,
            end_altitude: end,
            descent_seconds: f64::from(frames) / 60.0,
            lateral_radians: 0.0,
            ..DescentScript::orbit_to_surface(radius(kind))
        },
        frames,
    )
}

fn is_flagged(r: &TerrainReport) -> bool {
    r.select.trimmed_splits > 0
        || r.select.node_budget_hit
        || r.select.leaf_budget_hit
        || r.select.retain_rejected > 0
        || r.retain_expired > 0
        || r.effective_coarsened > 0
        || r.stream.evicted > 0
        || r.destroyed > 0
}

#[derive(Default, Debug)]
struct Analysis {
    refine_events: usize,
    refine_with_parent_morph: usize,
    parent_morph_max: f64,
    parent_morphs: Vec<f64>,
    /// `m_parent * delta_parent_max / bound` over events with m_parent > 0 (<= 1 required).
    disp_ratios: Vec<f64>,
    child_not_one: Vec<(usize, u64, f64)>,
    parent_not_zero: Vec<(usize, u64, u8, f64)>,
    /// Refine events (parents with m > 0) before the first converged frame: the initial
    /// stream-in climbs one level per few frames, it is not driven by the camera.
    warmup_parent_morphing: usize,
    settled_from: usize,
    coarsen_events: usize,
    coarsen_children_below_one: Vec<(usize, u64, f64)>,
    coarsen_flagged_skipped: usize,
    /// (frame, level jump, replaced ancestor level)
    multi_level: Vec<(usize, u32, u8)>,
    gap_max_mismatch: usize,
    multi_count_mismatch: usize,
    defects: u32,
}

struct Ctx<T: Topology> {
    topo: T,
    terrain: PlanetTerrain,
    model: LodModel,
    delta: BTreeMap<u64, f64>,
}

impl<T: Topology> Ctx<T> {
    fn new(topo: T, cfg: &TerrainConfig) -> Self {
        let terrain =
            PlanetTerrain::new(cfg.world_seed, cfg.terrain, cfg.cells_per_edge).expect("terrain");
        let model = LodModel::for_topology(&terrain, cfg.kappa, topo.sample_angle0());
        Self {
            topo,
            terrain,
            model,
            delta: BTreeMap::new(),
        }
    }

    /// Largest vertex displacement between a cell's own surface and its morph target, and the
    /// largest vertex offset (for the f32 rounding allowance).
    fn delta_max(&mut self, raw: u64) -> f64 {
        if let Some(&d) = self.delta.get(&raw) {
            return d;
        }
        let c = self.topo.from_raw(raw).expect("valid cell");
        let mesh = self
            .topo
            .build_mesh(c, &self.terrain, &self.model, false, &|| false)
            .expect("mesh");
        let mut d = 0.0f64;
        let mut big = 0.0f64;
        for (v, m) in mesh.vertices.iter().zip(&mesh.morph) {
            let dx = f64::from(v.position[0]) - f64::from(m.position[0]);
            let dy = f64::from(v.position[1]) - f64::from(m.position[1]);
            let dz = f64::from(v.position[2]) - f64::from(m.position[2]);
            d = d.max((dx * dx + dy * dy + dz * dz).sqrt());
            for k in 0..3 {
                big = big.max(f64::from(v.position[k]).abs());
            }
        }
        // f32 rounding of the offsets: a few ulps of the largest offset.
        let d = d + 4.0 * f64::from(f32::EPSILON) * big;
        self.delta.insert(raw, d);
        d
    }

    fn analyse(&mut self, frames: &[Frame]) -> Analysis {
        let mut a = Analysis {
            settled_from: frames
                .iter()
                .position(|f| f.converged)
                .map_or(frames.len(), |i| i + 1),
            ..Analysis::default()
        };
        for f in 1..frames.len() {
            let (prev, now) = (&frames[f - 1], &frames[f]);
            a.defects += now.report.morph_first_frame_defects;
            let gone: BTreeSet<u64> = prev
                .cells
                .keys()
                .filter(|k| !now.cells.contains_key(k))
                .copied()
                .collect();
            let appeared: Vec<u64> = now
                .cells
                .keys()
                .filter(|k| !prev.cells.contains_key(k))
                .copied()
                .collect();
            let mut replaced: BTreeSet<u64> = BTreeSet::new();
            let mut deep: BTreeSet<u64> = BTreeSet::new();
            let mut gap_max = 0.0f64;
            let mut parents = Vec::new();
            for &raw in &appeared {
                let c = self.topo.from_raw(raw).expect("cell");
                parents.clear();
                self.topo.parents(c, &mut parents);
                let mut direct = false;
                for &p in &parents {
                    let pr = self.topo.raw(p);
                    if gone.contains(&pr) {
                        direct = true;
                        a.refine_events += 1;
                        replaced.insert(pr);
                        let m_child = now.cells[&raw];
                        if m_child != 1.0 {
                            a.child_not_one.push((f, raw, m_child));
                        }
                        let m_p = prev.cells[&pr];
                        if m_p != 0.0 {
                            if f < a.settled_from {
                                a.warmup_parent_morphing += 1;
                            } else {
                                a.parent_not_zero.push((f, pr, self.topo.level(p), m_p));
                            }
                        }
                    }
                }
                if !direct {
                    // Two or more levels deeper than a drawn ancestor that is gone?
                    let mut cur = self.topo.primary_parent(c);
                    let mut depth = 1u32;
                    while let Some(anc) = cur {
                        let ar = self.topo.raw(anc);
                        if prev.cells.contains_key(&ar) && !now.cells.contains_key(&ar) {
                            if depth >= 2 {
                                a.multi_level.push((f, depth, self.topo.level(anc)));
                                deep.insert(ar);
                                gap_max = gap_max.max(prev.cells[&ar]);
                            }
                            break;
                        }
                        cur = self.topo.primary_parent(anc);
                        depth += 1;
                    }
                }
                // Coarsening: this cell replaces children that were drawn.
                let mut kids = Vec::new();
                self.topo.children(c, &mut kids);
                let gone_kids: Vec<u64> = kids
                    .iter()
                    .map(|&k| self.topo.raw(k))
                    .filter(|k| gone.contains(k))
                    .collect();
                if !gone_kids.is_empty() {
                    if is_flagged(&now.report) {
                        a.coarsen_flagged_skipped += 1;
                    } else {
                        for k in gone_kids {
                            a.coarsen_events += 1;
                            let m = prev.cells[&k];
                            if m < 1.0 - 1e-9 {
                                a.coarsen_children_below_one.push((f, k, m));
                            }
                        }
                    }
                }
            }
            for &pr in &replaced {
                let m_p = prev.cells[&pr];
                gap_max = gap_max.max(m_p);
                a.parent_morphs.push(m_p);
                a.parent_morph_max = a.parent_morph_max.max(m_p);
                let p = self.topo.from_raw(pr).expect("cell");
                let level = self.topo.level(p);
                if m_p > 0.0 {
                    if f >= a.settled_from {
                        a.refine_with_parent_morph += 1;
                    }
                    let d = self.delta_max(pr);
                    // The interpolation target is the grandparent surface: the parent's own
                    // vertices cannot be further from it than the grandparent-level error.
                    let bound = if level == 0 {
                        0.0
                    } else {
                        self.model.geometric_error(level - 1) + self.model.residual(level - 1)
                    };
                    let tol = 1e-6 + 1e-9 * bound;
                    assert!(
                        m_p * d <= m_p * (bound + tol) + 1e-9,
                        "frame {f}: parent {pr:#x} level {level}: m {m_p} delta {d} bound {bound}"
                    );
                    a.disp_ratios
                        .push(if bound > 0.0 { d / bound } else { 0.0 });
                }
            }
            if (gap_max - now.report.handover_gap_max).abs() > 1e-12 {
                a.gap_max_mismatch += 1;
            }
            if u32::try_from(deep.len()).unwrap() != now.report.multi_level_handovers {
                a.multi_count_mismatch += 1;
            }
        }
        a
    }
}

fn analyse(kind: TopologyKind, frames: &[Frame]) -> Analysis {
    let cfg = settings_for(kind).terrain;
    match kind {
        Hex => Ctx::new(HexSphere::new(cfg.cells_per_edge).expect("hex"), &cfg).analyse(frames),
        Cube => Ctx::new(CubeSphere::new(cfg.cells_per_edge), &cfg).analyse(frames),
    }
}

fn percentile(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    let rank = ((v.len() as f64) * q).ceil() as usize;
    v[rank.clamp(1, v.len()) - 1]
}

fn report(label: &str, a: &mut Analysis) {
    println!(
        "{label}: refine pairs {} (parents with m>0: {}), parent m max {:.4} p95 {:.4}, \
         disp/bound max {:.4} p95 {:.4}, coarsen children {} (skipped flagged {}), \
         events with m_parent > 0 (all frames) {}, multi-level {} {:?}, first-frame defects {}, warm-up pairs with morphing parent {}          (settled from frame {})",
        a.refine_events,
        a.refine_with_parent_morph,
        a.parent_morph_max,
        percentile(&mut a.parent_morphs, 0.95),
        a.disp_ratios.iter().copied().fold(0.0, f64::max),
        percentile(&mut a.disp_ratios, 0.95),
        a.coarsen_events,
        a.coarsen_flagged_skipped,
        a.disp_ratios.len(),
        a.multi_level.len(),
        a.multi_level,
        a.defects,
        a.warmup_parent_morphing,
        a.settled_from,
    );
}

fn slow_approach(kind: TopologyKind) {
    let (script, frames) = slow_script(kind);
    let recorded = run_script(kind, script, frames);
    let mut a = analyse(kind, &recorded);
    report(&format!("slow approach {kind:?}"), &mut a);
    assert!(
        a.refine_events > 50,
        "only {} refine events",
        a.refine_events
    );
    assert!(a.child_not_one.is_empty(), "{:?}", a.child_not_one);
    assert!(
        a.parent_not_zero.is_empty(),
        "parents still morphing at a refine event: {:?}",
        a.parent_not_zero
    );
    // Displacement bound: m_parent == 0 at every event, so the vertex displacement is the f32
    // rounding of the offsets only (plan 22 puts the child morph targets on the parent surface).
    assert_eq!(
        a.refine_with_parent_morph, 0,
        "parents morphing at a refine event after the initial stream-in"
    );
    assert_eq!(a.defects, 0);
    assert_eq!(a.gap_max_mismatch, 0);
    assert_eq!(a.multi_count_mismatch, 0);
}

fn scripted_and_fast(kind: TopologyKind) {
    let r = radius(kind);
    for (label, script, frames) in [
        ("scripted descent", DescentScript::orbit_to_surface(r), 720),
        ("fast free-flight", DescentScript::fast_free_flight(r), 240),
    ] {
        let recorded = run_script(kind, script, frames);
        let mut a = analyse(kind, &recorded);
        report(&format!("{label} {kind:?}"), &mut a);
        assert!(a.refine_events > 20, "{label}: {} events", a.refine_events);
        assert!(a.child_not_one.is_empty(), "{label}: {:?}", a.child_not_one);
        assert_eq!(a.defects, 0, "{label}");
        assert_eq!(a.gap_max_mismatch, 0, "{label}");
        assert_eq!(a.multi_count_mismatch, 0, "{label}");
    }
}

fn ascent(kind: TopologyKind) {
    let recorded = run_script(kind, DescentScript::surface_to_orbit(radius(kind)), 720);
    let mut a = analyse(kind, &recorded);
    report(&format!("scripted ascent {kind:?}"), &mut a);
    assert!(a.coarsen_events > 20, "{} coarsen events", a.coarsen_events);
    assert!(
        a.coarsen_children_below_one.is_empty(),
        "children still below morph 1 when replaced: {:?}",
        a.coarsen_children_below_one
    );
    assert_eq!(a.defects, 0);
    assert_eq!(a.gap_max_mismatch, 0);
    assert_eq!(a.multi_count_mismatch, 0);
}

#[test]
fn hex_slow_approach_handover_is_continuous() {
    slow_approach(Hex);
}

#[test]
fn cube_slow_approach_handover_is_continuous() {
    slow_approach(Cube);
}

#[test]
fn hex_scripted_and_fast_handover_bounds() {
    scripted_and_fast(Hex);
}

#[test]
fn cube_scripted_and_fast_handover_bounds() {
    scripted_and_fast(Cube);
}

#[test]
fn hex_ascent_children_are_fully_morphed_when_replaced() {
    ascent(Hex);
}

#[test]
fn cube_ascent_children_are_fully_morphed_when_replaced() {
    ascent(Cube);
}
