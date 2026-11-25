use std::path::{Path, PathBuf};
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("check-deps") {
        eprintln!("usage: cargo xtask check-deps [--manifest-path <PATH>]");
        exit(2);
    }
    let mut manifest: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("Cargo.toml");
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--manifest-path" => match args.get(i + 1) {
                Some(v) => {
                    manifest = PathBuf::from(v);
                    i += 2;
                }
                None => {
                    eprintln!("--manifest-path requires a value");
                    exit(2);
                }
            },
            other => {
                eprintln!("unknown argument: {other}");
                exit(2);
            }
        }
    }
    let graph = match xtask::graph_from_manifest(&manifest) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("check-deps: {e}");
            exit(2);
        }
    };
    let violations = xtask::evaluate(&graph);
    if violations.is_empty() {
        println!(
            "check-deps: OK ({} workspace packages checked)",
            graph.packages.len()
        );
    } else {
        for v in &violations {
            eprintln!("{v}");
        }
        eprintln!("check-deps: {} violation(s)", violations.len());
        exit(1);
    }
}
