//! 07.1-20: absolute flicker invariants of the drawn cut on hex and cube.
//!
//! The production configuration at the real defaults (async workers, adaptive caps, a simulated
//! 1/30 s frame, `wait_for_workers` pacing, no wall clock) with the churn counters on. Every
//! expectation is an absolute zero; none of them is a measured bound.

use std::sync::OnceLock;

use planetary_app::stream_bench::{StreamRun, run_stream};
use planetary_app::{AppSettings, DescentScript, SimClock, TopologyKind};
use planetary_terrain::{TerrainConfig, count_overlaps};

const DT: f64 = 1.0 / 30.0;
/// Hover frames after the descent ends.
const HOVER_FRAMES: u64 = 300;
const SIM_CLOCK_STEP_NS: u64 = 100_000;
/// Index of each counter in `StreamFrame::churn`.
const LEVEL_CHANGES: usize = 0;
const APPEARED: usize = 1;
const DISAPPEARED: usize = 2;
const CULL_TOGGLES: usize = 3;
const OVERLAP: usize = 4;
const FLIPFLOPS: usize = 5;
const FLIPFLOPS_UNEXPLAINED: usize = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Flight {
    Scripted,
    Fast,
}

fn execute(kind: TopologyKind, flight: Flight, retain_frames: u32) -> StreamRun {
    let mut settings = match kind {
        TopologyKind::Hex => AppSettings::default(),
        TopologyKind::Cube => AppSettings::cube_default(),
    };
    settings.terrain = TerrainConfig {
        diagnostics: true,
        retain_frames,
        ..settings.terrain
    };
    settings.adaptive_streaming = true;
    let r = settings.terrain.terrain.radius;
    let script = match flight {
        Flight::Scripted => DescentScript::orbit_to_surface(r),
        Flight::Fast => DescentScript::fast_free_flight(r),
    };
    settings.script = Some(script);
    let descent = (script.descent_seconds / DT).ceil() as u64;
    let mut run = run_stream(
        settings,
        Box::new(SimClock::new(SIM_CLOCK_STEP_NS)),
        descent + HOVER_FRAMES,
        DT,
        &mut |app| app.wait_for_workers(),
    )
    .expect("stream run");
    run.descent_frames = descent as usize;
    run
}

fn shared(kind: TopologyKind, flight: Flight) -> &'static StreamRun {
    static CELLS: [OnceLock<StreamRun>; 4] = [
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
    ];
    let i = match (kind, flight) {
        (TopologyKind::Hex, Flight::Scripted) => 0,
        (TopologyKind::Hex, Flight::Fast) => 1,
        (TopologyKind::Cube, Flight::Scripted) => 2,
        (TopologyKind::Cube, Flight::Fast) => 3,
    };
    CELLS[i].get_or_init(|| execute(kind, flight, TerrainConfig::default().retain_frames))
}

fn all() -> [(TopologyKind, Flight); 4] {
    [
        (TopologyKind::Hex, Flight::Scripted),
        (TopologyKind::Hex, Flight::Fast),
        (TopologyKind::Cube, Flight::Scripted),
        (TopologyKind::Cube, Flight::Fast),
    ]
}

#[test]
fn drawn_pieces_never_overlap() {
    for (kind, flight) in all() {
        let run = shared(kind, flight);
        for r in &run.frames {
            assert_eq!(
                r.churn[OVERLAP], 0,
                "{kind:?} {flight:?} frame {}: overlapping drawn pieces",
                r.frame
            );
        }
    }
}

#[test]
fn frozen_camera_has_zero_churn_after_convergence() {
    for (kind, flight) in all() {
        let run = shared(kind, flight);
        let conv = run
            .frames
            .iter()
            .enumerate()
            .skip(run.descent_frames)
            .find(|(_, r)| r.converged)
            .map(|(i, _)| i)
            .unwrap_or_else(|| panic!("{kind:?} {flight:?}: never converged within the hover"));
        let tail = &run.frames[conv + 1..];
        assert!(
            tail.len() >= 30,
            "{kind:?} {flight:?}: only {} frames after convergence at {conv}",
            tail.len()
        );
        for r in tail {
            for (name, idx) in [
                ("level_changes", LEVEL_CHANGES),
                ("drawn_appeared", APPEARED),
                ("drawn_disappeared", DISAPPEARED),
                ("cull_toggles", CULL_TOGGLES),
                ("overlap_pairs", OVERLAP),
            ] {
                assert_eq!(
                    r.churn[idx], 0,
                    "{kind:?} {flight:?} frame {} (converged at {conv}): {name}",
                    r.frame
                );
            }
        }
    }
}

#[test]
fn monotone_descent_has_no_unexplained_flipflops() {
    for (kind, flight) in all() {
        let run = shared(kind, flight);
        for r in run.frames.iter().take(run.descent_frames) {
            assert_eq!(
                r.churn[FLIPFLOPS_UNEXPLAINED], 0,
                "{kind:?} {flight:?} frame {}: {} flip-flops, trimmed {}, node budget {}, leaf budget {}, coarsened {}, destroyed {}",
                r.frame, r.churn[FLIPFLOPS], r.sel[0], r.sel[1], r.sel[2], r.sel[3], r.destroyed
            );
        }
    }
}

/// Negative control: the counter can fire. A parent drawn together with its child overlaps.
#[test]
fn overlap_counter_fires_on_parent_and_child() {
    use planetary_core::{CubeSphere, Piece, Topology};
    use std::collections::BTreeSet;
    let topo = CubeSphere::new(8);
    let root = topo.roots()[0];
    let mut kids = Vec::new();
    topo.children(root, &mut kids);
    let pieces = [
        Piece {
            cell: root,
            variant: 0,
        },
        Piece {
            cell: kids[0],
            variant: 0,
        },
    ];
    assert_eq!(count_overlaps(&topo, &pieces, &BTreeSet::new()), 1);
    // The same pair with the root split (as a real cut would have it) still counts: a drawn cell
    // must not be split.
    let split: BTreeSet<_> = [root].into_iter().collect();
    assert!(count_overlaps(&topo, &pieces, &split) >= 1);
    // A proper cut (children only, root split) does not.
    let ok: Vec<Piece<_>> = kids
        .iter()
        .map(|&cell| Piece { cell, variant: 0 })
        .collect();
    assert_eq!(count_overlaps(&topo, &ok, &split), 0);
}

/// Row E measurement (report only, no bound): the fraction of seeded directions whose palette
/// class (colour segment of the height index, mask row) differs between a linear interpolation of
/// samples spaced one level apart. A coarse mesh interpolates `(height index, mask)` between its
/// vertices, a fine mesh between closer ones; a class change at the handover is a colour pop that
/// morphing cannot remove.
#[test]
fn colour_class_pop_fraction_by_level() {
    use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
    use planetary_core::{CubeSphere, PlanetTerrain, TerrainParams, Topology};
    use planetary_terrain::DEMO_SEED;

    let params = TerrainParams::default();
    let cells = 32u32;
    let terrain = PlanetTerrain::new(DEMO_SEED, params, cells).expect("terrain");
    let topo = CubeSphere::new(cells);
    let model = LodModel::for_topology(&terrain, DEFAULT_KAPPA, topo.sample_angle0());
    let span = params.max_relief() - params.sea_level;
    // Colour segment of palette x (0..64): sand, green, brown, grey, white (palette.rs BANDS).
    let seg = |h_idx: f64| -> u32 {
        let x = (h_idx.clamp(0.0, 1.0) * 63.0).floor() as u32;
        match x {
            0..=2 => 0,
            3..=19 => 1,
            20..=39 => 2,
            40..=55 => 3,
            _ => 4,
        }
    };
    let row = |m: f64| -> u32 { (m.clamp(0.0, 1.0) * 3.0).round() as u32 };
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) as f64 / u64::MAX as f64
    };
    println!(
        "level | n | segment differs | mask row differs | any class differs | palette x differs"
    );
    let n = 4000usize;
    for level in 6u8..=14 {
        let (mut seg_d, mut row_d, mut any_d, mut x_d) = (0usize, 0usize, 0usize, 0usize);
        for _ in 0..n {
            let z = 2.0 * next() - 1.0;
            let a = std::f64::consts::TAU * next();
            let r = (1.0 - z * z).sqrt();
            let d = engine_sdk::DVec3::new(r * a.cos(), r * a.sin(), z);
            let t = next();
            let axis = d
                .cross(engine_sdk::DVec3::Z)
                .normalize_or(engine_sdk::DVec3::X);
            let interp = |spacing_m: f64| -> (f64, f64) {
                let ang = spacing_m / model.radius;
                let p0 = (d - axis * (t * ang)).normalize();
                let p1 = (d + axis * ((1.0 - t) * ang)).normalize();
                let (s0, s1) = (terrain.sample(p0), terrain.sample(p1));
                (
                    ((1.0 - t) * s0.height + t * s1.height - params.sea_level) / span,
                    (1.0 - t) * s0.mask + t * s1.mask,
                )
            };
            let (hc, mc) = interp(model.cell_size(level - 1));
            let (hf, mf) = interp(model.cell_size(level));
            let (sc, sf) = (seg(hc), seg(hf));
            let (rc, rf) = (row(mc), row(mf));
            seg_d += usize::from(sc != sf);
            row_d += usize::from(rc != rf);
            any_d += usize::from(sc != sf || rc != rf);
            let xc = (hc.clamp(0.0, 1.0) * 63.0).floor() as u32;
            let xf = (hf.clamp(0.0, 1.0) * 63.0).floor() as u32;
            x_d += usize::from(xc != xf);
        }
        let f = |k: usize| k as f64 / n as f64;
        println!(
            "{level} | {n} | {:.4} | {:.4} | {:.4} | {:.4}",
            f(seg_d),
            f(row_d),
            f(any_d),
            f(x_d)
        );
    }
}

/// Negative control for the fix: with the culled-cell retention switched off (the behaviour
/// before 07.1-20) the monotone hex descent does flip-flop without any budget or trim
/// explanation, so the invariant above can fail and the retention is what keeps it at zero.
#[test]
fn without_retention_the_flipflop_counter_fires() {
    let run = execute(TopologyKind::Hex, Flight::Scripted, 0);
    let unexplained: u32 = run
        .frames
        .iter()
        .take(run.descent_frames)
        .map(|r| r.churn[FLIPFLOPS_UNEXPLAINED])
        .sum();
    println!("retention off: {unexplained} unexplained flip-flops during the descent");
    assert!(unexplained > 0);
}
