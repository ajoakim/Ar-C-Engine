//! Docs/ADR coverage: documentation and layer table cannot drift from the workspace (WS-02, WS-04).

use cargo_metadata::MetadataCommand;
use std::path::{Path, PathBuf};
use xtask::Rule;
use xtask::layers::LAYERS;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn require(file: &str, text: &str, lit: &str) {
    assert!(
        text.contains(lit),
        "{file} is missing required literal {lit:?}"
    );
}

fn packages() -> Vec<(String, PathBuf)> {
    let md = MetadataCommand::new()
        .manifest_path(repo_root().join("Cargo.toml"))
        .no_deps()
        .exec()
        .expect("metadata");
    md.workspace_packages()
        .into_iter()
        .map(|p| {
            (
                p.name.to_string(),
                p.manifest_path.as_std_path().to_path_buf(),
            )
        })
        .collect()
}

#[test]
fn docs_cover_all_crates() {
    let file = "docs/architecture/dependency-direction.md";
    let doc = read(file);
    for (name, _) in packages() {
        require(file, &doc, &format!("`{name}`"));
    }
    const ALL: [Rule; 6] = [
        Rule::CoreDependsOnExtension,
        Rule::WgpuOutsideAllowedCrate,
        Rule::WinitOutsideAllowedCrate,
        Rule::ExtensionCrossDependency,
        Rule::TierViolation,
        Rule::UnlistedCrate,
    ];
    for r in ALL {
        require(file, &doc, r.id());
    }
}

#[test]
fn canary_documented() {
    let file = "docs/architecture/dependency-direction.md";
    let doc = read(file);
    require(file, &doc, "canary-planetary-reachable");
    require(file, &doc, "check-canary");
}

#[test]
fn adr_conventions_present() {
    let f0 = "docs/architecture/adr/0000-template.md";
    let t = read(f0);
    for lit in [
        "- Status:",
        "## Context",
        "## Decision",
        "## Alternatives Considered",
        "## Consequences",
        "## Evidence",
    ] {
        require(f0, &t, lit);
    }
    let f1 = "docs/architecture/adr/0001-conventions.md";
    let c = read(f1);
    for lit in [
        "Status: accepted",
        "meters",
        "right-handed",
        "Y-up",
        "-Z forward",
        "reversed-Z",
        "Depth32Float",
        "[0, 1]",
    ] {
        require(f1, &c, lit);
    }
}

#[test]
fn layer_table_matches_workspace() {
    let pkgs = packages();
    for l in LAYERS {
        assert!(
            pkgs.iter().any(|(n, _)| n == l.name),
            "layer table lists `{}` which is not a workspace package",
            l.name
        );
    }
    let root = repo_root().canonicalize().expect("root");
    for (name, manifest) in pkgs {
        let manifest = manifest.canonicalize().unwrap_or(manifest);
        let rel = manifest.strip_prefix(&root).unwrap_or(&manifest);
        let first = rel
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().into_owned());
        if matches!(first.as_deref(), Some("crates" | "extensions")) {
            assert!(
                LAYERS.iter().any(|l| l.name == name),
                "workspace package `{name}` is missing from the layer table"
            );
        }
    }
}

#[test]
fn adr_0008_determinism_present() {
    let f = "docs/architecture/adr/0008-deterministic-streaming-and-procedural-generation.md";
    let t = read(f);
    for lit in [
        "Status: accepted",
        "splitmix64",
        "fnv1a64",
        "HASH_VERSION",
        "bit-exact",
        "mul_add",
        "QueueFull",
        "not for security",
    ] {
        require(f, &t, lit);
    }
}

#[test]
fn adr_0009_gpu_hook_present() {
    let f = "docs/architecture/adr/0009-sdk-per-frame-gpu-hook.md";
    let t = read(f);
    for lit in [
        "Status: accepted",
        "gpu_frame",
        "destroy_mesh",
        "default no-op",
    ] {
        require(f, &t, lit);
    }
}

#[test]
fn adr_0010_lod_technique_present() {
    let f = "docs/architecture/adr/0010-terrain-lod-skirts-and-shared-border-sampling.md";
    let t = read(f);
    for lit in [
        "skirts",
        "shared-border",
        "2:1",
        "effective cut",
        "## Evidence",
        "T(s)",
        "Status: accepted",
    ] {
        require(f, &t, lit);
    }
}

#[test]
fn adr_0011_hex_topology_present() {
    let f = "docs/architecture/adr/0011-hex-grid-planet-topology.md";
    let t = read(f);
    for lit in [
        "Status: accepted",
        "half-cell",
        "pentagon",
        "gnomonic",
        "Topology",
        "## Evidence",
    ] {
        require(f, &t, lit);
    }
}

#[test]
fn adr_0012_lod_selection_present() {
    let f = "docs/architecture/adr/0012-terrain-lod-selection-tight-bounds-culling-budgets.md";
    let t = read(f);
    for lit in [
        "Status: accepted",
        "horizon",
        "frustum",
        "node-visit budget",
        "max_level 16",
        "## Evidence",
    ] {
        require(f, &t, lit);
    }
}

#[test]
fn adr_0013_index_range_present() {
    let f = "docs/architecture/adr/0013-shared-index-buffers-and-index-range-draws.md";
    let t = read(f);
    for lit in [
        "Status: accepted",
        "with_index_range",
        "draw_indexed",
        "shared",
        "## Evidence",
    ] {
        require(f, &t, lit);
    }
}

#[test]
fn adr_0014_vertex_morph_present() {
    let f = "docs/architecture/adr/0014-vertex-morph-attribute-geomorphing.md";
    let t = read(f);
    for lit in [
        "Status: accepted",
        "morph_position",
        "morph_uv",
        "parent",
        "skirt",
        "## Evidence",
    ] {
        require(f, &t, lit);
    }
}

#[test]
fn adr_0015_scale_contrast_present() {
    let f = "docs/architecture/adr/0015-scale-contrast-and-distance-haze.md";
    let t = read(f);
    for lit in [
        "Status: accepted",
        "generator_version",
        "DistanceFog",
        "PLAN-08",
        "## Evidence",
    ] {
        require(f, &t, lit);
    }
}

#[test]
fn adr_0016_present_mode_present() {
    let f = "docs/architecture/adr/0016-present-mode-setting-and-window-readout.md";
    let t = read(f);
    for lit in [
        "Status: accepted",
        "PresentModeSetting",
        "Fifo",
        "## Measurements",
        "## Evidence",
    ] {
        require(f, &t, lit);
    }
}

#[test]
fn adr_0017_present() {
    let f = "docs/architecture/adr/0017-no-ecs-in-p0.md";
    let t = read(f);
    for lit in [
        "Status: accepted",
        "hecs",
        "slotmap",
        "## Measurements",
        "## Evidence",
        "ecs-bench",
        "bevy_ecs",
        "Revisit",
    ] {
        require(f, &t, lit);
    }
    let readme = read("docs/architecture/adr/README.md");
    require("docs/architecture/adr/README.md", &readme, "[0017]");
}

#[test]
fn adr_0018_present() {
    let f = "docs/architecture/adr/0018-debug-overlay-placement-and-font-licences.md";
    let t = read(f);
    for lit in [
        "Status: accepted",
        "egui-wgpu",
        "engine-render-wgpu",
        "OFL-1.1",
        "Ubuntu-font-1.0",
        "egui-winit",
        "F3",
        "## Alternatives Considered",
        "## Evidence",
    ] {
        require(f, &t, lit);
    }
    let readme = read("docs/architecture/adr/README.md");
    require("docs/architecture/adr/README.md", &readme, "[0018]");
}

#[test]
fn adr_0019_present() {
    let f = "docs/architecture/adr/0019-minimal-atmosphere-limb-ring.md";
    let t = read(f);
    for lit in [
        "Status: accepted",
        "limb",
        "r_in",
        "planetary-app",
        "## Evidence",
        "fallback",
        "supersede",
        "## Deviation from CONTEXT",
        "shader pass",
        "ShaderId",
        "REPLACE",
    ] {
        require(f, &t, lit);
    }
    let readme = read("docs/architecture/adr/README.md");
    require("docs/architecture/adr/README.md", &readme, "[0019]");
    require(
        "docs/architecture/adr/README.md",
        &readme,
        "haze superseded by 0019",
    );
    let f15 = "docs/architecture/adr/0015-scale-contrast-and-distance-haze.md";
    let t15 = read(f15);
    let status = t15
        .lines()
        .find(|l| l.starts_with("- Status:"))
        .expect("0015 status line");
    assert!(
        status.contains("Status: accepted") && status.contains("ADR-0019"),
        "0015 status line must keep `accepted` and cite ADR-0019: {status}"
    );
    let limits = read("docs/limitations.md");
    for lit in ["limb ring", "scattering", "alpha blend"] {
        require("docs/limitations.md", &limits, lit);
    }
}

#[test]
fn dependency_direction_documents_egui_wgpu_rule() {
    let f = "docs/architecture/dependency-direction.md";
    let t = read(f);
    for lit in ["egui-wgpu", "ADR-0018"] {
        require(f, &t, lit);
    }
}

#[test]
fn phase8_amendments_cite_evidence() {
    let marker = "Amendment (Phase 8)";
    let f5 = "docs/architecture/adr/0005-rendering-api-wgpu-behind-trait.md";
    let t5 = read(f5);
    for lit in [
        marker,
        "image_diff",
        "WARP",
        "plan 11",
        "llvmpipe",
        "46042013890",
    ] {
        require(f5, &t5, lit);
    }
    let f6 = "docs/architecture/adr/0006-spatial-index-hash-grid-first.md";
    let t6 = read(f6);
    for lit in [marker, "HashGridIndex", "spatial.rs", "Linear / HashGrid"] {
        require(f6, &t6, lit);
    }
    let f10 = "docs/architecture/adr/0010-terrain-lod-skirts-and-shared-border-sampling.md";
    let t10 = read(f10);
    for lit in [
        marker,
        "45947523739",
        "08-CI-EVIDENCE",
        "ADR-0014",
        "LOD transition",
    ] {
        require(f10, &t10, lit);
    }
    let readme = read("docs/architecture/adr/README.md");
    require("docs/architecture/adr/README.md", &readme, "LOD transition");
}

#[test]
fn architecture_overview_and_sdk_usage_present() {
    let fo = "docs/architecture-overview.md";
    let overview = read(fo);
    for lit in [
        "## Layers",
        "engine-core",
        "planetary",
        "dependency-direction.md",
        "ADR-0002",
        "ADR-0018",
        "ecs-bench",
        "debug-ui",
    ] {
        require(fo, &overview, lit);
    }
    // The published overview must stand alone: it never cites the untracked design document.
    assert!(
        !overview.contains("HLD"),
        "{fo} must not depend on or cite HLD.md"
    );
    // Every ADR in the index is reachable from the overview.
    for n in 1..=18 {
        require(fo, &overview, &format!("ADR-{n:04}"));
    }

    let fs = "docs/sdk-usage.md";
    let sdk = read(fs);
    for lit in [
        "## Running the examples",
        "triangle",
        "basic_scene",
        "planetary_flight",
        "gpu_frame",
        "F3",
        "frame_with",
        "overlay_frame",
        "egui-overlay",
        "debug-overlay",
        "PresentConfig",
        "ARC_REQUIRE_ADAPTER",
        "ARC_UPDATE_GOLDENS",
    ] {
        require(fs, &sdk, lit);
    }
    assert!(
        !sdk.contains("HLD"),
        "{fs} must not depend on or cite HLD.md"
    );
}

/// Names the SDK guide relies on must still exist in the source (drift guard).
#[test]
fn sdk_usage_names_exist_in_source() {
    let app = read("crates/engine-sdk/src/app.rs");
    for lit in ["fn gpu_frame", "fn overlay_frame", "pub trait App"] {
        require("crates/engine-sdk/src/app.rs", &app, lit);
    }
    let rt = read("crates/engine-sdk/src/runtime.rs");
    require("crates/engine-sdk/src/runtime.rs", &rt, "pub fn frame_with");
    let run = read("crates/engine-sdk/src/run.rs");
    require("crates/engine-sdk/src/run.rs", &run, "pub fn run_app");
    let flight = read("extensions/planetary/planetary-app/examples/planetary_flight.rs");
    let doc = read("docs/sdk-usage.md");
    // Every flag named in the example's usage string is in the guide's flag list.
    let usage = flight
        .lines()
        .find(|l| l.starts_with("const USAGE"))
        .expect("USAGE const");
    for flag in usage.split("--").skip(1) {
        let name: String = flag
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        require("docs/sdk-usage.md", &doc, &format!("--{name}"));
    }
}

#[test]
fn assumptions_and_limitations_present() {
    let fa = "docs/assumptions.md";
    let assumptions = read(fa);
    for lit in [
        "## Assumptions",
        "Rust",
        "wgpu",
        "determinism",
        "ADR-0004",
        "ADR-0008",
    ] {
        require(fa, &assumptions, lit);
    }
    let fl = "docs/limitations.md";
    let limits = read(fl);
    for lit in [
        "Wayland",
        "f32",
        "unmorphed",
        "macOS",
        "ECS",
        "haze",
        "complete-with-caveats",
        "Fifo",
        "plan 11",
        "plan 12",
        "ADR-0006",
        "ADR-0017",
        "ADR-0018",
    ] {
        require(fl, &limits, lit);
    }
    for (f, t) in [(fa, &assumptions), (fl, &limits)] {
        assert!(!t.contains("HLD"), "{f} must not cite HLD.md");
    }
}

#[test]
fn readme_links_documentation_set() {
    let readme = read("README.md");
    for lit in [
        "## Documentation",
        "docs/architecture-overview.md",
        "docs/assumptions.md",
        "docs/limitations.md",
        "docs/sdk-usage.md",
        "docs/architecture/adr/README.md",
    ] {
        require("README.md", &readme, lit);
    }
}
