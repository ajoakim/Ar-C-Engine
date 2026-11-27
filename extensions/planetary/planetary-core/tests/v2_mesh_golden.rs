//! v2 (`TerrainParams::demo_v2`) mesh byte golden: hex cells (payload v4, positions, normals, uv,
//! morph targets) and cube patches (positions, normals, uv, indices). Recorded in plan 11-01 from
//! the unmodified v2 code so later edits to the helpers shared by v1/v2/v3 cannot silently shift
//! v2 output (TERR-02).

use engine_sdk::DVec3;
use engine_sdk::procedural::Fnv1a64;
use planetary_core::hex::CellId;
use planetary_core::hex::adjacency::is_pentagon;
use planetary_core::hex::mesh::build_hex_cell;
use planetary_core::lod::{DEFAULT_KAPPA, LodModel};
use planetary_core::mesh::PatchMesh;
use planetary_core::{
    CubeSphere, HexSphere, PlanetTerrain, TerrainParams, Topology, build_patch,
    encode_cell_payload, locate,
};

const SEED: u64 = 0x0007_5EED_2026_0001;

// Recorded plan 11-01 from unmodified v2 code; Windows computed; Linux confirmed by CI.
const GOLDEN_V2_HEX_HEXAGON: u64 = 0xd5fa_dc4a_d97d_1a7a;
const GOLDEN_V2_HEX_PENTAGON: u64 = 0x934f_5991_7ac6_5daa;
const GOLDEN_V2_CUBE_L4: u64 = 0x018d_5b5c_0c91_e7be;
const GOLDEN_V2_CUBE_L10: u64 = 0xbe19_670e_7a02_a316;

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = Fnv1a64::new();
    h.write(bytes);
    h.finish()
}

fn hex_hash(params: TerrainParams, pentagon: bool) -> u64 {
    let n = 16;
    let t = PlanetTerrain::new(SEED, params, n).expect("terrain");
    let hex = HexSphere::new(n).expect("hex");
    let model = LodModel::for_topology(&t, DEFAULT_KAPPA, hex.sample_angle0());
    let cell = CellId::level_iter(8)
        .find(|c| is_pentagon(*c) == pentagon)
        .expect("cell");
    let mesh = build_hex_cell(cell, &t, &model, false, &|| false).expect("mesh");
    fnv(&encode_cell_payload(&mesh))
}

fn patch_bytes(mesh: &PatchMesh) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&mesh.id.raw().to_le_bytes());
    for f in [mesh.origin.x, mesh.origin.y, mesh.origin.z] {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out.extend_from_slice(&mesh.bound_radius.to_le_bytes());
    for f in [mesh.skirt_depth, mesh.height_min, mesh.height_max] {
        out.extend_from_slice(&f.to_le_bytes());
    }
    for v in &mesh.vertices {
        for f in v.position.iter().chain(&v.normal).chain(&v.uv) {
            out.extend_from_slice(&f.to_le_bytes());
        }
    }
    for i in &mesh.indices {
        out.extend_from_slice(&i.to_le_bytes());
    }
    out
}

fn cube_hash(params: TerrainParams, level: u8) -> u64 {
    let n = 32;
    let t = PlanetTerrain::new(SEED, params, n).expect("terrain");
    let cube = CubeSphere::new(n);
    let model = LodModel::for_topology(&t, DEFAULT_KAPPA, cube.sample_angle0());
    let id = locate(DVec3::new(1.0, 0.3, 0.2), level);
    fnv(&patch_bytes(&build_patch(&t, &model, id)))
}

#[test]
fn v2_hex_cells_match_golden() {
    let h = hex_hash(TerrainParams::demo_v2(), false);
    let p = hex_hash(TerrainParams::demo_v2(), true);
    eprintln!("GOLDEN_V2_HEX_HEXAGON = {h:#018x}");
    eprintln!("GOLDEN_V2_HEX_PENTAGON = {p:#018x}");
    assert_eq!(h, GOLDEN_V2_HEX_HEXAGON);
    assert_eq!(p, GOLDEN_V2_HEX_PENTAGON);
}

#[test]
fn v2_cube_patches_match_golden() {
    let a = cube_hash(TerrainParams::demo_v2(), 4);
    let b = cube_hash(TerrainParams::demo_v2(), 10);
    eprintln!("GOLDEN_V2_CUBE_L4 = {a:#018x}");
    eprintln!("GOLDEN_V2_CUBE_L10 = {b:#018x}");
    assert_eq!(a, GOLDEN_V2_CUBE_L4);
    assert_eq!(b, GOLDEN_V2_CUBE_L10);
}

#[test]
fn hash_is_deterministic_and_version_aware() {
    assert_eq!(
        hex_hash(TerrainParams::demo_v2(), false),
        hex_hash(TerrainParams::demo_v2(), false)
    );
    assert_ne!(
        hex_hash(TerrainParams::demo_v2(), false),
        hex_hash(TerrainParams::default(), false)
    );
    assert_eq!(
        cube_hash(TerrainParams::demo_v2(), 4),
        cube_hash(TerrainParams::demo_v2(), 4)
    );
    assert_ne!(
        cube_hash(TerrainParams::demo_v2(), 4),
        cube_hash(TerrainParams::default(), 4)
    );
}
