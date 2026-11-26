//! PLAN-01 canary tests: pure graph logic, real workspace, leak fixture, CLI exit codes.

use std::path::{Path, PathBuf};
use std::process::Command;
use xtask::CheckError;
use xtask::canary::{
    CanaryNode, DepGraph, check_canary, closure, is_planetary, reachable_planetary,
};

fn graph(spec: &[(&str, bool, &[&str])]) -> DepGraph {
    let mut g = DepGraph::default();
    for (n, p, deps) in spec {
        g.nodes.insert(
            (*n).to_string(),
            CanaryNode {
                planetary: *p,
                deps: deps.iter().map(|d| (*d).to_string()).collect(),
            },
        );
    }
    g
}

fn repo_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("Cargo.toml")
}

fn fixture_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("canary-leak")
        .join("Cargo.toml")
}

fn xtask(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .output()
        .expect("run xtask")
}

#[test]
fn canary_unit_transitive_path() {
    let g = graph(&[("a", false, &["b"]), ("b", false, &["c"]), ("c", true, &[])]);
    let v = reachable_planetary(&g, "a").expect("ok");
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].reached, "c");
    assert_eq!(v[0].path, ["a", "b", "c"]);
}

#[test]
fn canary_unit_clean_graph_ok() {
    let g = graph(&[
        ("a", false, &["b"]),
        ("b", false, &["c"]),
        ("c", false, &[]),
    ]);
    assert!(reachable_planetary(&g, "a").expect("ok").is_empty());
}

#[test]
fn canary_unit_unknown_root_errors() {
    let g = graph(&[("a", false, &[])]);
    assert!(matches!(
        reachable_planetary(&g, "missing"),
        Err(CheckError::Path(_))
    ));
    assert!(matches!(closure(&g, "missing"), Err(CheckError::Path(_))));
}

#[test]
fn canary_unit_is_planetary() {
    assert!(is_planetary(
        "planetary-core",
        Path::new("extensions/planetary/planetary-core")
    ));
    assert!(is_planetary(
        "terrain-x",
        Path::new("extensions/planetary/terrain-x")
    ));
    assert!(is_planetary("planetary-tools", Path::new("tools/x")));
    assert!(!is_planetary("engine-sdk", Path::new("crates/engine-sdk")));
    assert!(!is_planetary(
        "conventional-3d",
        Path::new("extensions/conventional-3d")
    ));
}

#[test]
fn canary_unit_cycle_terminates() {
    let g = graph(&[("a", false, &["b"]), ("b", false, &["a"])]);
    assert!(reachable_planetary(&g, "a").expect("ok").is_empty());
    assert_eq!(closure(&g, "a").expect("ok").len(), 2);
}

#[test]
fn canary_real_workspace_ok() {
    let v = check_canary(&repo_manifest(), "conventional-3d").expect("metadata");
    assert!(v.is_empty(), "unexpected violations: {v:?}");
}

#[test]
fn canary_fixture_leak_detected() {
    let v = check_canary(&fixture_manifest(), "conventional-3d").expect("metadata");
    assert_eq!(v.len(), 1, "{v:?}");
    assert_eq!(v[0].reached, "planetary-core");
    assert_eq!(
        v[0].path,
        ["conventional-3d", "engine-sdk", "planetary-core"]
    );
}

#[test]
fn canary_cli_real_workspace_exit_0() {
    let out = xtask(&["check-canary"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("check-canary: OK"));
}

#[test]
fn canary_cli_fixture_exit_1() {
    let m = fixture_manifest();
    let out = xtask(&["check-canary", "--manifest-path", m.to_str().expect("utf8")]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("canary-planetary-reachable"), "{err}");
    assert!(
        err.contains("conventional-3d -> engine-sdk -> planetary-core"),
        "{err}"
    );
}

#[test]
fn canary_cli_unknown_package_exit_2() {
    let out = xtask(&["check-canary", "--package", "does-not-exist"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn canary_cli_check_deps_unchanged() {
    let out = xtask(&["check-deps"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("check-deps: OK ("));
}
