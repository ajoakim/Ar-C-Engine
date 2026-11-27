//! APP-02 guard: the planetary app, its example and the planetary crates use the SDK only,
//! and no planetary source uses wall-clock time, sleeps or hash-ordered containers.

use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sibling(name: &str) -> PathBuf {
    root().join("..").join(name)
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

/// Keys declared in `[section]` (inline `key = ...` / `key.workspace = true` lines) and in
/// `[section.key]` table headers, sorted.
fn parse_keys(text: &str, section: &str) -> Vec<String> {
    let header = format!("[{section}]");
    let table_prefix = format!("[{section}.");
    let mut in_section = false;
    let mut keys = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_section = l == header;
            if let Some(rest) = l.strip_prefix(table_prefix.as_str()) {
                keys.push(
                    rest.trim_end_matches(']')
                        .trim()
                        .trim_matches('"')
                        .to_owned(),
                );
            }
            continue;
        }
        if in_section && !l.is_empty() && !l.starts_with('#') {
            let key: String = l
                .chars()
                .take_while(|c| *c != '.' && *c != '=' && !c.is_whitespace())
                .collect();
            keys.push(key);
        }
    }
    keys.sort();
    keys
}

fn manifest_keys(manifest: &Path) -> Vec<String> {
    parse_keys(&fs::read_to_string(manifest).unwrap(), "dependencies")
}

fn dev_manifest_keys(manifest: &Path) -> Vec<String> {
    parse_keys(&fs::read_to_string(manifest).unwrap(), "dev-dependencies")
}

#[test]
fn manifest_parser_handles_inline_and_table_forms() {
    let text = "[dependencies]
engine-sdk.workspace = true
# c
foo = { path = \"x\" }

                [dependencies.bar]
version = \"1\"

[dependencies.\"baz\"]
path = \"y\"

                [dev-dependencies]
qux = \"1\"

[dev-dependencies.quux]
version = \"1\"
";
    assert_eq!(
        parse_keys(text, "dependencies"),
        ["bar", "baz", "engine-sdk", "foo"]
    );
    assert_eq!(parse_keys(text, "dev-dependencies"), ["quux", "qux"]);
}

#[test]
fn dev_dependencies_are_pinned() {
    // Test-only deps may reach below the SDK (atmosphere_ring.rs renders the ring headless), but
    // only these; adding one needs a deliberate edit here. They must never move to [dependencies].
    assert_eq!(
        dev_manifest_keys(&root().join("Cargo.toml")),
        ["criterion", "engine-render", "engine-render-wgpu", "png"]
    );
    assert_eq!(
        dev_manifest_keys(&sibling("planetary-terrain").join("Cargo.toml")),
        Vec::<String>::new()
    );
    assert_eq!(
        dev_manifest_keys(&sibling("planetary-core").join("Cargo.toml")),
        ["criterion"]
    );
}

#[test]
fn manifest_dependencies_are_sdk_and_planetary_only() {
    assert_eq!(
        manifest_keys(&root().join("Cargo.toml")),
        ["engine-sdk", "planetary-core", "planetary-terrain"]
    );
    assert_eq!(
        manifest_keys(&sibling("planetary-terrain").join("Cargo.toml")),
        ["engine-sdk", "planetary-core"]
    );
    assert_eq!(
        manifest_keys(&sibling("planetary-core").join("Cargo.toml")),
        ["engine-sdk"]
    );
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
        "engine_streaming",
        "engine_procedural",
        "wgpu",
        "winit",
        "raw_window_handle",
        "conventional",
    ];
    let mut files = rs_files(&root().join("src"));
    files.extend(rs_files(&root().join("examples")));
    files.extend(rs_files(&sibling("planetary-core").join("src")));
    files.extend(rs_files(&sibling("planetary-terrain").join("src")));
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
fn examples_use_only_sdk_and_planetary_roots() {
    let files = rs_files(&root().join("examples"));
    assert!(!files.is_empty());
    for (f, r) in use_roots(&files) {
        assert!(
            [
                "engine_sdk",
                "planetary_app",
                "planetary_core",
                "planetary_terrain",
                "std"
            ]
            .contains(&r.as_str()),
            "{} uses root `{r}`",
            f.display()
        );
    }
}

#[test]
fn sources_use_only_allowed_roots() {
    let cases: [(PathBuf, &[&str]); 3] = [
        (
            root().join("src"),
            &[
                "engine_sdk",
                "planetary_core",
                "planetary_terrain",
                "crate",
                "self",
                "super",
                "std",
                "core",
            ],
        ),
        (
            sibling("planetary-terrain").join("src"),
            &[
                "engine_sdk",
                "planetary_core",
                "crate",
                "self",
                "super",
                "std",
                "core",
            ],
        ),
        (
            sibling("planetary-core").join("src"),
            &["engine_sdk", "crate", "self", "super", "std", "core"],
        ),
    ];
    for (dir, allowed) in cases {
        let files = rs_files(&dir);
        assert!(!files.is_empty());
        for (f, r) in use_roots(&files) {
            assert!(
                allowed.contains(&r.as_str()),
                "{} uses root `{r}`",
                f.display()
            );
        }
    }
}

#[test]
fn no_wall_clock_in_planetary_crates() {
    // Tokens are built at runtime so this file never matches itself.
    let banned = [
        ["Inst", "ant"].concat(),
        ["System", "Time"].concat(),
        ["thread::", "sleep"].concat(),
        ["Hash", "Map"].concat(),
        ["Hash", "Set"].concat(),
    ];
    let mut files = Vec::new();
    for base in [
        root(),
        sibling("planetary-core"),
        sibling("planetary-terrain"),
    ] {
        for sub in ["src", "tests", "examples"] {
            files.extend(rs_files(&base.join(sub)));
        }
    }
    let telemetry = root()
        .join("src")
        .join("telemetry.rs")
        .canonicalize()
        .unwrap();
    let instant = ["Inst", "ant"].concat();
    let mut scanned = 0usize;
    for f in files {
        let is_telemetry = f.canonicalize().is_ok_and(|c| c == telemetry);
        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        // banned_ops.rs is the float/hash scanner itself; sdk_only.rs is this file.
        if name == "banned_ops.rs" || name == "sdk_only.rs" {
            continue;
        }
        scanned += 1;
        for l in code_lines(&f) {
            for tok in &banned {
                if is_telemetry && *tok == instant {
                    continue;
                }
                assert!(
                    !l.contains(tok.as_str()),
                    "{} uses `{tok}`: {l}",
                    f.display()
                );
            }
        }
    }
    assert!(scanned >= 10, "only {scanned} files scanned");
}

#[test]
fn wall_clock_only_in_telemetry_module() {
    let instant = ["Inst", "ant"].concat();
    let telemetry = root().join("src").join("telemetry.rs");
    assert!(telemetry.exists());
    let telemetry = telemetry.canonicalize().unwrap();
    assert!(
        code_lines(&telemetry).iter().any(|l| l.contains(&instant)),
        "telemetry.rs must hold the wall clock"
    );
    let mut files = Vec::new();
    for base in [
        root(),
        sibling("planetary-core"),
        sibling("planetary-terrain"),
    ] {
        for sub in ["src", "tests", "examples"] {
            files.extend(rs_files(&base.join(sub)));
        }
    }
    for f in files {
        if f.canonicalize().is_ok_and(|c| c == telemetry) {
            continue;
        }
        for l in code_lines(&f) {
            assert!(!l.contains(&instant), "{} uses `{instant}`", f.display());
        }
    }
}
