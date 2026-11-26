//! APP-01 guard: the extension and its example use the SDK only.

use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn code_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.starts_with("//"))
        .collect()
}

fn use_roots(files: &[PathBuf]) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    for f in files {
        for l in code_lines(f) {
            if let Some(rest) = l.strip_prefix("use ") {
                let rest = rest.trim_start_matches('{').trim();
                let root: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                out.push((f.clone(), root));
            }
        }
    }
    out
}

#[test]
fn manifest_dependencies_are_sdk_only() {
    let text = fs::read_to_string(root().join("Cargo.toml")).unwrap();
    let mut in_deps = false;
    let mut keys = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_deps = l == "[dependencies]";
            continue;
        }
        if in_deps && !l.is_empty() && !l.starts_with('#') {
            let key: String = l
                .chars()
                .take_while(|c| *c != '.' && *c != '=' && !c.is_whitespace())
                .collect();
            keys.push(key);
        }
    }
    keys.sort();
    assert_eq!(keys, ["engine-sdk", "gltf", "image", "thiserror"]);
}

#[test]
fn no_forbidden_crate_names() {
    let forbidden = [
        "engine_core",
        "engine_render",
        "engine_platform",
        "engine_world",
        "engine_assets",
        "engine_jobs",
        "engine_math",
        "engine_spatial",
        "engine_diagnostics",
        "wgpu",
        "winit",
        "raw_window_handle",
        "planetary",
    ];
    let mut files = rs_files(&root().join("src"));
    files.extend(rs_files(&root().join("examples")));
    assert!(!files.is_empty());
    for f in files {
        for l in code_lines(&f) {
            for name in forbidden {
                assert!(!l.contains(name), "{} names `{name}`: {l}", f.display());
            }
        }
    }
}

#[test]
fn examples_use_only_sdk_roots() {
    let files = rs_files(&root().join("examples"));
    assert!(!files.is_empty());
    for (f, r) in use_roots(&files) {
        assert!(
            ["engine_sdk", "conventional_3d", "std"].contains(&r.as_str()),
            "{} uses root `{r}`",
            f.display()
        );
    }
}

#[test]
fn sources_use_only_allowed_roots() {
    let files = rs_files(&root().join("src"));
    assert!(!files.is_empty());
    for (f, r) in use_roots(&files) {
        assert!(
            [
                "engine_sdk",
                "crate",
                "self",
                "super",
                "std",
                "gltf",
                "image",
                "thiserror"
            ]
            .contains(&r.as_str()),
            "{} uses root `{r}`",
            f.display()
        );
    }
}
