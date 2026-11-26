//! Scans planetary-core sources for operations that are not bit-exact across platforms,
//! non-deterministic hashing/clocks (ADR-0008), and platform-width integers in
//! border-critical code. Tokens are built with `concat!` so this file never contains them.

use std::fs;
use std::path::Path;

const BANNED: &[&str] = &[
    concat!(".si", "n("),
    concat!(".co", "s("),
    concat!(".ta", "n("),
    concat!(".ex", "p("),
    concat!(".ex", "p2("),
    concat!(".l", "n("),
    concat!(".lo", "g("),
    concat!(".lo", "g2("),
    concat!(".lo", "g10("),
    concat!(".po", "wf("),
    concat!(".po", "wi("),
    concat!(".mul_", "add("),
    concat!(".cb", "rt("),
    concat!(".hyp", "ot("),
    concat!(".at", "an("),
    concat!(".at", "an2("),
    concat!(".as", "in("),
    concat!(".ac", "os("),
    concat!(".ta", "nh("),
    concat!("Default", "Hasher"),
    concat!("Random", "State"),
    concat!("Hash", "Map"),
    concat!("Hash", "Set"),
    concat!("Inst", "ant"),
    concat!("System", "Time"),
    concat!("thread::", "sleep"),
];

const WIDTH_TOKEN: &str = concat!("us", "ize");
const BEGIN: &str = "// BORDER-CRITICAL BEGIN";
const END: &str = "// BORDER-CRITICAL END";

fn src_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
}

fn scan(dir: &Path, files: &mut u32, failures: &mut Vec<String>) {
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
fn banned_ops_absent_from_planetary_core_src() {
    let mut files = 0;
    let mut failures = Vec::new();
    scan(src_dir(), &mut files, &mut failures);
    assert!(files >= 3, "only scanned {files} files");
    assert!(failures.is_empty(), "banned tokens found: {failures:#?}");
}

/// Returns (BEGIN count, END count, offending lines); `whole` checks every line.
fn width_scan(name: &str, text: &str, whole: bool) -> (u32, u32, Vec<String>) {
    let (mut begins, mut ends) = (0u32, 0u32);
    let mut inside = whole;
    let mut bad = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let t = line.trim();
        if t == BEGIN {
            begins += 1;
            inside = true;
            continue;
        }
        if t == END {
            ends += 1;
            inside = whole;
            continue;
        }
        if inside && !t.starts_with("//") && line.contains(WIDTH_TOKEN) {
            bad.push(format!("{name}:{}: {t}", n + 1));
        }
    }
    (begins, ends, bad)
}

#[test]
fn no_usize_in_border_critical_code() {
    let mut failures = Vec::new();

    let mapping = fs::read_to_string(src_dir().join("mapping.rs")).expect("mapping.rs");
    let (_, _, bad) = width_scan("mapping.rs", &mapping, true);
    failures.extend(bad);

    let hex_mapping =
        fs::read_to_string(src_dir().join("hex").join("mapping.rs")).expect("hex/mapping.rs");
    let (_, _, bad) = width_scan("hex/mapping.rs", &hex_mapping, true);
    failures.extend(bad);

    let terrain = fs::read_to_string(src_dir().join("terrain.rs")).expect("terrain.rs");
    let (b, e, bad) = width_scan("terrain.rs", &terrain, false);
    assert!(b >= 1, "terrain.rs has no BORDER-CRITICAL marker pair");
    assert_eq!(b, e, "terrain.rs unbalanced BORDER-CRITICAL markers");
    failures.extend(bad);

    let mesh_path = src_dir().join("mesh.rs");
    if mesh_path.exists() {
        let mesh = fs::read_to_string(&mesh_path).expect("mesh.rs");
        let (b, e, bad) = width_scan("mesh.rs", &mesh, false);
        assert!(b >= 1, "mesh.rs has no BORDER-CRITICAL marker pair");
        assert_eq!(b, e, "mesh.rs unbalanced BORDER-CRITICAL markers");
        failures.extend(bad);
    }

    assert!(
        failures.is_empty(),
        "platform-width ints found: {failures:#?}"
    );
}
