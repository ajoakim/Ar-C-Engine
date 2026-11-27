//! `select_cells` for the hex and cube topologies (QA-01), at the production defaults
//! (`LodParams::default()`, hex sector side 16, cube 32 cells per edge, as in `AppSettings`).
//!
//! Views: orbit (2 R from the centre) and low altitude (1 km above the surface). Each is run
//! cold (empty previous split set) and warm (previous split set from the same view, the
//! steady-state per-frame case). Inputs are built outside the timed closure; no timing is
//! asserted.

use std::collections::BTreeSet;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use engine_sdk::DVec3;
use planetary_core::lod::{DEFAULT_KAPPA, LodModel, LodParams, View};
use planetary_core::topology::select::select_cells;
use planetary_core::{CubeSphere, HexSphere, PlanetTerrain, TerrainParams, Topology};

const SEED: u64 = 0x0007_5EED_2026_0001;
const R: f64 = 6_371_000.0;

fn view_at(eye: DVec3) -> View {
    View {
        eye,
        proj_scale: 720.0 / (2.0 * (1.0 / 3.0f64.sqrt())),
        frustum: None,
    }
}

fn model_for<T: Topology>(topo: &T, n: u32) -> LodModel {
    let terrain = PlanetTerrain::new(SEED, TerrainParams::default(), n).expect("terrain");
    LodModel::for_topology(&terrain, DEFAULT_KAPPA, topo.sample_angle0())
}

fn bench_topology<T: Topology>(c: &mut Criterion, topo: &T, n: u32, low_eye: DVec3) {
    let model = model_for(topo, n);
    let params = LodParams::default();
    let views = [
        ("orbit", view_at(DVec3::new(2.0 * R, 0.0, 0.0))),
        ("low_1km", view_at(low_eye)),
    ];
    let mut g = c.benchmark_group(format!("select_cells/{}", topo.name()));
    g.sample_size(20);
    for (label, view) in &views {
        let empty = BTreeSet::new();
        g.bench_function(format!("{label}/cold"), |b| {
            b.iter(|| select_cells(topo, &model, &params, black_box(view), &empty, &|_| None))
        });
        let warm = select_cells(topo, &model, &params, view, &empty, &|_| None).split;
        g.bench_function(format!("{label}/warm"), |b| {
            b.iter(|| select_cells(topo, &model, &params, black_box(view), &warm, &|_| None))
        });
    }
    g.finish();
}

fn benches_select(c: &mut Criterion) {
    let hex = HexSphere::new(16).expect("hex");
    let hex_eye = {
        let ctr = hex.footprint(hex.roots()[0]).center;
        let l = (ctr.x * ctr.x + ctr.y * ctr.y + ctr.z * ctr.z).sqrt();
        let s = (R + 1000.0) / l;
        DVec3::new(ctr.x * s, ctr.y * s, ctr.z * s)
    };
    bench_topology(c, &hex, 16, hex_eye);

    let cube = CubeSphere::new(32);
    bench_topology(c, &cube, 32, DVec3::new(R + 1000.0, 0.0, 0.0));
}

criterion_group!(benches, benches_select);
criterion_main!(benches);
