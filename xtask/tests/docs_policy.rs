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
