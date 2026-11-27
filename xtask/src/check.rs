//! Rule engine: reads the workspace graph and enforces dependency-direction rules R1-R6.

use crate::layers::{TOOLING, WGPU_ALLOWED, WINIT_ALLOWED, tier_of};
use std::fmt;
use std::path::{Component, Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    Core,
    Extension { family: String },
    Tooling,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepKind {
    Normal,
    Dev,
    Build,
}

#[derive(Debug, Clone)]
pub struct DepInfo {
    pub name: String,
    pub kind: DepKind,
}

#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub location: Location,
    pub deps: Vec<DepInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceGraph {
    pub packages: Vec<PackageInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    CoreDependsOnExtension,
    WgpuOutsideAllowedCrate,
    WinitOutsideAllowedCrate,
    ExtensionCrossDependency,
    TierViolation,
    UnlistedCrate,
}

impl Rule {
    pub fn id(self) -> &'static str {
        match self {
            Rule::CoreDependsOnExtension => "core-depends-on-extension",
            Rule::WgpuOutsideAllowedCrate => "wgpu-outside-allowed-crate",
            Rule::WinitOutsideAllowedCrate => "winit-outside-allowed-crate",
            Rule::ExtensionCrossDependency => "extension-cross-dependency",
            Rule::TierViolation => "tier-violation",
            Rule::UnlistedCrate => "unlisted-crate",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Violation {
    pub rule: Rule,
    pub package: String,
    pub dependency: Option<String>,
    pub message: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "error[{}]: {} -> {}: {}",
            self.rule.id(),
            self.package,
            self.dependency.as_deref().unwrap_or("-"),
            self.message
        )
    }
}

#[derive(Debug)]
pub enum CheckError {
    Metadata(cargo_metadata::Error),
    Path(String),
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckError::Metadata(e) => write!(f, "cargo metadata failed: {e}"),
            CheckError::Path(p) => write!(f, "path error: {p}"),
        }
    }
}

impl std::error::Error for CheckError {}

/// Classify a package by its manifest directory relative to the workspace root.
pub fn location_of(workspace_root: &Path, manifest_path: &Path) -> Location {
    let dir = manifest_path.parent().unwrap_or(manifest_path);
    let rel = dir.strip_prefix(workspace_root).unwrap_or(dir);
    let mut comps = rel.components().filter_map(|c| match c {
        Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
        _ => None,
    });
    match comps.next().as_deref() {
        Some("crates") => Location::Core,
        Some("extensions") => match comps.next() {
            Some(family) => Location::Extension { family },
            None => Location::Tooling,
        },
        _ => Location::Tooling,
    }
}

pub fn graph_from_manifest(manifest_path: &Path) -> Result<WorkspaceGraph, CheckError> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()
        .map_err(CheckError::Metadata)?;
    let root = metadata.workspace_root.as_std_path().to_path_buf();
    let packages = metadata
        .workspace_packages()
        .into_iter()
        .map(|p| PackageInfo {
            name: p.name.to_string(),
            location: location_of(&root, p.manifest_path.as_std_path()),
            deps: p
                .dependencies
                .iter()
                .map(|d| DepInfo {
                    name: d.name.clone(),
                    kind: match d.kind {
                        cargo_metadata::DependencyKind::Development => DepKind::Dev,
                        cargo_metadata::DependencyKind::Build => DepKind::Build,
                        _ => DepKind::Normal,
                    },
                })
                .collect(),
        })
        .collect();
    Ok(WorkspaceGraph { packages })
}

pub fn evaluate(graph: &WorkspaceGraph) -> Vec<Violation> {
    let mut out = Vec::new();
    let find = |n: &str| graph.packages.iter().find(|p| p.name == n);
    for pkg in &graph.packages {
        let mk = |rule, dep: Option<&str>, message: String| Violation {
            rule,
            package: pkg.name.clone(),
            dependency: dep.map(str::to_string),
            message,
        };
        if pkg.location != Location::Tooling && tier_of(&pkg.name).is_none() {
            out.push(mk(
                Rule::UnlistedCrate,
                None,
                "crate is not in the layer table (xtask/src/layers.rs)".into(),
            ));
        }
        let tooling = TOOLING.contains(&pkg.name.as_str());
        for dep in &pkg.deps {
            if dep.name == "wgpu" && !WGPU_ALLOWED.contains(&pkg.name.as_str()) && !tooling {
                out.push(mk(
                    Rule::WgpuOutsideAllowedCrate,
                    Some("wgpu"),
                    "wgpu may only be used by engine-render-wgpu".into(),
                ));
            }
            if dep.name == "egui-wgpu" && !WGPU_ALLOWED.contains(&pkg.name.as_str()) && !tooling {
                out.push(mk(
                    Rule::WgpuOutsideAllowedCrate,
                    Some("egui-wgpu"),
                    "egui-wgpu (which links wgpu) may only be used by engine-render-wgpu".into(),
                ));
            }
            if dep.name == "winit" && !WINIT_ALLOWED.contains(&pkg.name.as_str()) && !tooling {
                out.push(mk(
                    Rule::WinitOutsideAllowedCrate,
                    Some("winit"),
                    "winit may only be used by engine-platform".into(),
                ));
            }
            let Some(target) = find(&dep.name) else {
                continue;
            };
            match (&pkg.location, &target.location) {
                (Location::Core, Location::Extension { .. }) => out.push(mk(
                    Rule::CoreDependsOnExtension,
                    Some(&dep.name),
                    "core crates must not depend on extensions".into(),
                )),
                (Location::Extension { family: f1 }, Location::Extension { family: f2 })
                    if f1 != f2 =>
                {
                    out.push(mk(
                        Rule::ExtensionCrossDependency,
                        Some(&dep.name),
                        format!("extension family `{f1}` must not depend on family `{f2}`"),
                    ));
                }
                _ => {}
            }
            let same_family = matches!(
                (&pkg.location, &target.location),
                (Location::Extension { family: a }, Location::Extension { family: b }) if a == b
            );
            if let (Some(tp), Some(td)) = (tier_of(&pkg.name), tier_of(&dep.name))
                && td >= tp
                && !same_family
            {
                out.push(mk(
                    Rule::TierViolation,
                    Some(&dep.name),
                    format!("tier {tp} crate depends on tier {td} crate (dependencies must point to a lower tier)"),
                ));
            }
        }
    }
    out.sort_by(|a, b| {
        (&a.package, a.rule.id(), &a.dependency).cmp(&(&b.package, b.rule.id(), &b.dependency))
    });
    out
}

pub fn check_deps(manifest_path: &Path) -> Result<Vec<Violation>, CheckError> {
    Ok(evaluate(&graph_from_manifest(manifest_path)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, location: Location, deps: &[&str]) -> PackageInfo {
        PackageInfo {
            name: name.into(),
            location,
            deps: deps
                .iter()
                .map(|d| DepInfo {
                    name: (*d).into(),
                    kind: DepKind::Normal,
                })
                .collect(),
        }
    }
    fn ext(f: &str) -> Location {
        Location::Extension { family: f.into() }
    }
    fn g(packages: Vec<PackageInfo>) -> WorkspaceGraph {
        WorkspaceGraph { packages }
    }
    fn has(v: &[Violation], rule: Rule, package: &str, dep: Option<&str>) -> bool {
        v.iter()
            .any(|x| x.rule == rule && x.package == package && x.dependency.as_deref() == dep)
    }

    #[test]
    fn clean_graph_has_no_violations() {
        let v = evaluate(&g(vec![
            pkg("engine-math", Location::Core, &[]),
            pkg("engine-core", Location::Core, &["engine-math"]),
            pkg("engine-sdk", Location::Core, &["engine-core"]),
            pkg("planetary-core", ext("planetary"), &["engine-sdk"]),
        ]));
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn core_to_extension_flags_r1() {
        let v = evaluate(&g(vec![
            pkg("engine-core", Location::Core, &["planetary-core"]),
            pkg("planetary-core", ext("planetary"), &[]),
        ]));
        assert!(has(
            &v,
            Rule::CoreDependsOnExtension,
            "engine-core",
            Some("planetary-core")
        ));
    }

    #[test]
    fn wgpu_only_in_render_wgpu() {
        let v = evaluate(&g(vec![
            pkg("engine-core", Location::Core, &["wgpu"]),
            pkg("engine-render-wgpu", Location::Core, &["wgpu"]),
        ]));
        assert!(has(
            &v,
            Rule::WgpuOutsideAllowedCrate,
            "engine-core",
            Some("wgpu")
        ));
        assert!(!v.iter().any(|x| x.package == "engine-render-wgpu"));
    }

    #[test]
    fn egui_wgpu_only_in_render_wgpu() {
        let v = evaluate(&g(vec![
            pkg("debug-ui", Location::Core, &["egui-wgpu"]),
            pkg("engine-sdk", Location::Core, &["egui-wgpu"]),
            pkg("engine-render-wgpu", Location::Core, &["egui-wgpu"]),
        ]));
        assert!(has(
            &v,
            Rule::WgpuOutsideAllowedCrate,
            "debug-ui",
            Some("egui-wgpu")
        ));
        assert!(has(
            &v,
            Rule::WgpuOutsideAllowedCrate,
            "engine-sdk",
            Some("egui-wgpu")
        ));
        assert!(!v.iter().any(|x| x.package == "engine-render-wgpu"));
    }

    #[test]
    fn winit_only_in_platform() {
        let v = evaluate(&g(vec![
            pkg("engine-world", Location::Core, &["winit"]),
            pkg("engine-platform", Location::Core, &["winit"]),
        ]));
        assert!(has(
            &v,
            Rule::WinitOutsideAllowedCrate,
            "engine-world",
            Some("winit")
        ));
        assert!(!v.iter().any(|x| x.package == "engine-platform"));
    }

    #[test]
    fn extension_cross_family_flags_r4() {
        let v = evaluate(&g(vec![
            pkg(
                "conventional-3d",
                ext("conventional-3d"),
                &["planetary-core"],
            ),
            pkg("planetary-core", ext("planetary"), &[]),
        ]));
        assert!(has(
            &v,
            Rule::ExtensionCrossDependency,
            "conventional-3d",
            Some("planetary-core")
        ));
    }

    #[test]
    fn lower_tier_depending_upward_flags_r5() {
        let v = evaluate(&g(vec![
            pkg("engine-math", Location::Core, &["engine-core"]),
            pkg("engine-core", Location::Core, &["engine-jobs"]),
            pkg("engine-jobs", Location::Core, &[]),
        ]));
        assert!(has(
            &v,
            Rule::TierViolation,
            "engine-math",
            Some("engine-core")
        ));
        assert!(has(
            &v,
            Rule::TierViolation,
            "engine-core",
            Some("engine-jobs")
        ));
    }

    #[test]
    fn unknown_core_crate_flags_r6() {
        let v = evaluate(&g(vec![
            pkg("engine-foo", Location::Core, &[]),
            pkg("xtask", Location::Tooling, &[]),
        ]));
        assert!(has(&v, Rule::UnlistedCrate, "engine-foo", None));
        assert!(!v.iter().any(|x| x.package == "xtask"));
    }

    #[test]
    fn rule_ids_are_stable() {
        assert_eq!(
            Rule::CoreDependsOnExtension.id(),
            "core-depends-on-extension"
        );
        assert_eq!(
            Rule::WgpuOutsideAllowedCrate.id(),
            "wgpu-outside-allowed-crate"
        );
        assert_eq!(
            Rule::WinitOutsideAllowedCrate.id(),
            "winit-outside-allowed-crate"
        );
        assert_eq!(
            Rule::ExtensionCrossDependency.id(),
            "extension-cross-dependency"
        );
        assert_eq!(Rule::TierViolation.id(), "tier-violation");
        assert_eq!(Rule::UnlistedCrate.id(), "unlisted-crate");
    }
}
