use std::path::{Path, PathBuf};
use std::process::exit;

const USAGE: &str = "usage: cargo xtask check-deps [--manifest-path <PATH>]\n       cargo xtask check-canary [--manifest-path <PATH>] [--package <NAME>]";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check-deps") => run_check_deps(&args[1..]),
        Some("check-canary") => run_check_canary(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            exit(2);
        }
    }
}

/// Parses `--manifest-path` (and `--package` when `allow_package`). Exits 2 on bad input.
fn parse_opts(args: &[String], allow_package: bool) -> (PathBuf, Option<String>) {
    let mut manifest: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("Cargo.toml");
    let mut package = None;
    let mut i = 0;
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
            "--package" if allow_package => match args.get(i + 1) {
                Some(v) => {
                    package = Some(v.clone());
                    i += 2;
                }
                None => {
                    eprintln!("--package requires a value");
                    exit(2);
                }
            },
            other => {
                eprintln!("unknown argument: {other}");
                exit(2);
            }
        }
    }
    (manifest, package)
}

fn run_check_deps(args: &[String]) {
    let (manifest, _) = parse_opts(args, false);
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

fn run_check_canary(args: &[String]) {
    let (manifest, package) = parse_opts(args, true);
    let package = package.unwrap_or_else(|| xtask::canary::DEFAULT_CANARY_PACKAGE.to_string());
    let graph = match xtask::canary::canary_graph_from_manifest(&manifest) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("check-canary: {e}");
            exit(2);
        }
    };
    let reach = match xtask::canary::closure(&graph, &package) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("check-canary: {e}");
            exit(2);
        }
    };
    let violations = match xtask::canary::reachable_planetary(&graph, &package) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("check-canary: {e}");
            exit(2);
        }
    };
    if violations.is_empty() {
        println!(
            "check-canary: OK ({package} reaches {} packages, none planetary)",
            reach.len() - 1
        );
    } else {
        for v in &violations {
            eprintln!("{v}");
        }
        eprintln!("check-canary: {} violation(s)", violations.len());
        exit(1);
    }
}
