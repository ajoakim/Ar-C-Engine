//! Mesh build: `build_hex_cell` (hexagon and pentagon, sector side 16 = `AppSettings` hex
//! default) and `build_patch` (cube patch, 32 cells per edge = `AppSettings::cube_default`).
//! Deterministic cells, built terrain and model are made outside the timed closure.
//! generator_version is an axis (ids suffixed `/gen{version}`) so v3/v2 is measured in one
//! session (11-RESEARCH F3).

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use engine_sdk::DVec3;
use planetary_core::hex::CellId;
use planetary_core::hex::adjacency::is_pentagon;
use planetary_core::hex::mesh::build_hex_cell;
use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
use planetary_core::mesh::build_patch;
use planetary_core::{CubeSphere, HexSphere, PlanetTerrain, TerrainParams, Topology, locate};

const SEED: u64 = 0x0007_5EED_2026_0001;

fn terrain(n: u32, version: u32) -> PlanetTerrain {
    let params = match version {
        1 => TerrainParams::default(),
        2 => TerrainParams::demo_v2(),
        3 => TerrainParams::demo_v3(),
        _ => unreachable!("unknown generator version"),
    };
    PlanetTerrain::new(SEED, params, n).expect("terrain")
}

fn bench_mesh(c: &mut Criterion) {
    let n_hex = 16;
    let n_cube = 32;
    let hexagon = CellId::level_iter(8)
        .find(|c| !is_pentagon(*c))
        .expect("hexagon");
    let pentagon = CellId::level_iter(8)
        .find(|c| is_pentagon(*c))
        .expect("pentagon");

    let mut g = c.benchmark_group("mesh_build");
    g.sample_size(20);
    for version in [1u32, 2, 3] {
        let t = terrain(n_hex, version);
        let hex = HexSphere::new(n_hex).expect("hex");
        let model = LodModel::for_topology(&t, DEFAULT_KAPPA, hex.sample_angle0());
        g.bench_function(format!("hex_cell/hexagon_l8_n16/gen{version}"), |b| {
            b.iter(|| build_hex_cell(black_box(hexagon), &t, &model, false, &|| false))
        });
        g.bench_function(format!("hex_cell/pentagon_l8_n16/gen{version}"), |b| {
            b.iter(|| build_hex_cell(black_box(pentagon), &t, &model, false, &|| false))
        });

        let tc = terrain(n_cube, version);
        let cube = CubeSphere::new(n_cube);
        let model_c = LodModel::for_topology(&tc, DEFAULT_KAPPA, cube.sample_angle0());
        for level in [4u8, 10] {
            let id = locate(DVec3::new(1.0, 0.3, 0.2), level);
            g.bench_function(format!("cube_patch/level{level}_n32/gen{version}"), |b| {
                b.iter(|| build_patch(&tc, &model_c, black_box(id)))
            });
        }
    }
    g.finish();
}

criterion_group!(benches, bench_mesh);
criterion_main!(benches);
