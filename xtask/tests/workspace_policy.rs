//! Workspace metadata policy: edition, MSRV, license, publish, toolchain pin (WS-01).

use cargo_metadata::{Edition, MetadataCommand, semver::Version};
use std::path::{Path, PathBuf};

fn root_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("Cargo.toml")
}

#[test]
fn real_workspace_ok() {
    let v = xtask::check_deps(&root_manifest()).expect("metadata");
    assert!(v.is_empty(), "violations: {v:?}");
}

#[test]
fn workspace_metadata_policy() {
    let md = MetadataCommand::new()
        .manifest_path(root_manifest())
        .no_deps()
        .exec()
        .expect("metadata");
    let min = Version::new(1, 95, 0);
    for p in md.workspace_packages() {
        assert_eq!(
            p.edition,
            Edition::E2024,
            "{}: edition must be 2024",
            p.name
        );
        let rv = p
            .rust_version
            .as_ref()
            .unwrap_or_else(|| panic!("{}: rust-version missing", p.name));
        assert!(*rv >= min, "{}: rust-version {rv} < 1.95.0", p.name);
        assert_eq!(
            p.license.as_deref(),
            Some("MIT OR Apache-2.0"),
            "{}: license",
            p.name
        );
        assert_eq!(
            p.publish,
            Some(vec![]),
            "{}: must be publish = false",
            p.name
        );
    }
    let tc = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("rust-toolchain.toml"),
    )
    .expect("rust-toolchain.toml");
    assert!(
        tc.contains("channel = \"1.99.0\""),
        "toolchain must pin 1.99.0"
    );
}
