//! Negative tests: violating fixture workspaces MUST be rejected (WS-03).

use std::path::{Path, PathBuf};
use xtask::{Rule, check_deps};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
        .join("Cargo.toml")
}

#[test]
fn fixture_core_depends_on_extension_fails() {
    let v = check_deps(&fixture("core-depends-on-ext")).expect("metadata");
    assert!(
        v.iter().any(|x| x.rule == Rule::CoreDependsOnExtension
            && x.package == "engine-core"
            && x.dependency.as_deref() == Some("planetary-core")),
        "expected core-depends-on-extension, got {v:?}"
    );
}

#[test]
fn fixture_wgpu_winit_misplaced_fails() {
    let v = check_deps(&fixture("wgpu-winit-misplaced")).expect("metadata");
    assert!(
        v.iter()
            .any(|x| x.rule == Rule::WgpuOutsideAllowedCrate && x.package == "engine-core"),
        "expected wgpu violation for engine-core, got {v:?}"
    );
    assert!(
        v.iter()
            .any(|x| x.rule == Rule::WinitOutsideAllowedCrate && x.package == "engine-world"),
        "expected winit violation for engine-world, got {v:?}"
    );
    assert!(
        !v.iter()
            .any(|x| x.rule == Rule::WgpuOutsideAllowedCrate && x.package == "engine-render-wgpu"),
        "engine-render-wgpu must be allowed to use wgpu, got {v:?}"
    );
}

#[test]
fn fixture_egui_wgpu_misplaced_fails() {
    let v = check_deps(&fixture("egui-wgpu-misplaced")).expect("metadata");
    assert!(
        v.iter().any(|x| x.rule == Rule::WgpuOutsideAllowedCrate
            && x.package == "debug-ui"
            && x.dependency.as_deref() == Some("egui-wgpu")),
        "expected egui-wgpu violation for debug-ui, got {v:?}"
    );
    assert!(
        !v.iter().any(|x| x.package == "engine-render-wgpu"),
        "engine-render-wgpu must be allowed to use egui-wgpu, got {v:?}"
    );
}

#[test]
fn fixture_egui_wgpu_misplaced_cli_exits_one() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("check-deps")
        .arg("--manifest-path")
        .arg(fixture("egui-wgpu-misplaced"))
        .output()
        .expect("run xtask");
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn fixture_cli_exits_nonzero() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("check-deps")
        .arg("--manifest-path")
        .arg(fixture("core-depends-on-ext"))
        .output()
        .expect("run xtask");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("core-depends-on-extension"),
        "stderr: {stderr}"
    );
}
