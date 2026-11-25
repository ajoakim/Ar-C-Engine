//! CORE-03 guard: the SDK facade must not expose or depend on GPU/window crates.
#![allow(clippy::result_large_err)]

use std::fs;

fn read(rel: &str) -> String {
    fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn facade_manifest_has_no_gpu_or_window_deps() {
    for line in read("Cargo.toml").lines() {
        let l = line.trim_start();
        for bad in ["wgpu", "winit", "raw-window-handle"] {
            assert!(!l.starts_with(bad), "manifest names {bad}: {line}");
        }
    }
}

#[test]
fn facade_public_api_has_no_backend_types() {
    let lib = read("src/lib.rs");
    for bad in [
        "wgpu::",
        "winit::",
        "raw_window_handle",
        "PlatformWindow",
        "RenderTarget",
        "engine_render_wgpu::",
    ] {
        assert!(!lib.contains(bad), "lib.rs names {bad}");
    }
}

#[test]
fn facade_example_uses_only_sdk() {
    let ex = read("examples/triangle.rs");
    assert!(ex.contains("use engine_sdk::"));
    for bad in [
        "wgpu",
        "winit",
        "engine_render",
        "engine_core",
        "engine_platform",
        "engine_math",
    ] {
        assert!(!ex.contains(bad), "example names {bad}");
    }
}
