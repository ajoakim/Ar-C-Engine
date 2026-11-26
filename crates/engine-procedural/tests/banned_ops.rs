//! Scans engine-procedural sources for operations that are not bit-exact
//! across platforms or that have non-deterministic hashing (ADR-0008).

use std::fs;
use std::path::Path;

const BANNED: &[&str] = &[
    ".sin(",
    ".cos(",
    ".tan(",
    ".exp(",
    ".exp2(",
    ".ln(",
    ".log(",
    ".log2(",
    ".log10(",
    ".powf(",
    ".powi(",
    ".mul_add(",
    ".cbrt(",
    ".hypot(",
    ".atan2(",
    ".tanh(",
    "DefaultHasher",
    "RandomState",
    "HashMap",
    "HashSet",
    "as f32",
];

fn scan(dir: &Path, files: &mut usize, failures: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("read_dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            scan(&path, files, failures);
        } else if path.extension().is_some_and(|e| e == "rs") {
            *files += 1;
            let text = fs::read_to_string(&path).expect("read file");
            for (n, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                for token in BANNED {
                    if line.contains(token) {
                        failures.push(format!("{}:{}: {token}", path.display(), n + 1));
                    }
                }
            }
        }
    }
}

#[test]
fn banned_ops_absent_from_procedural_src() {
    let mut files = 0;
    let mut failures = Vec::new();
    scan(
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")),
        &mut files,
        &mut failures,
    );
    assert!(files >= 3, "only scanned {files} files");
    assert!(failures.is_empty(), "banned tokens found: {failures:#?}");
}
