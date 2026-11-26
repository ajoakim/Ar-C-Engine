//! PLAN-01 canary: no planetary crate may be reachable from the conventional app.

use crate::check::CheckError;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fmt;
use std::path::{Component, Path};

pub const CANARY_RULE_ID: &str = "canary-planetary-reachable";
pub const DEFAULT_CANARY_PACKAGE: &str = "conventional-3d";

#[derive(Debug, Clone, Default)]
pub struct CanaryNode {
    pub planetary: bool,
    pub deps: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DepGraph {
    pub nodes: BTreeMap<String, CanaryNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanaryViolation {
    pub root: String,
    pub reached: String,
    pub path: Vec<String>,
}

impl fmt::Display for CanaryViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "error[{CANARY_RULE_ID}]: {} -> {}: path {}",
            self.root,
            self.reached,
            self.path.join(" -> ")
        )
    }
}

/// A package is planetary if named `planetary-*` or located under `extensions/planetary/`.
pub fn is_planetary(name: &str, rel_manifest_dir: &Path) -> bool {
    if name.starts_with("planetary-") {
        return true;
    }
    let mut comps = rel_manifest_dir
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .map(|c| c.as_os_str().to_string_lossy().into_owned());
    comps.next().as_deref() == Some("extensions") && comps.next().as_deref() == Some("planetary")
}

/// BFS closure over deps, including the root.
pub fn closure(graph: &DepGraph, root: &str) -> Result<BTreeSet<String>, CheckError> {
    if !graph.nodes.contains_key(root) {
        return Err(CheckError::Path(format!(
            "package `{root}` not found in dependency graph"
        )));
    }
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();
    seen.insert(root.to_string());
    queue.push_back(root.to_string());
    while let Some(cur) = queue.pop_front() {
        if let Some(node) = graph.nodes.get(&cur) {
            for d in &node.deps {
                if seen.insert(d.clone()) {
                    queue.push_back(d.clone());
                }
            }
        }
    }
    Ok(seen)
}

/// Every planetary node reachable from `root` (including root), with its shortest path.
pub fn reachable_planetary(
    graph: &DepGraph,
    root: &str,
) -> Result<Vec<CanaryViolation>, CheckError> {
    if !graph.nodes.contains_key(root) {
        return Err(CheckError::Path(format!(
            "package `{root}` not found in dependency graph"
        )));
    }
    let mut parent: HashMap<String, String> = HashMap::new();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();
    seen.insert(root.to_string());
    queue.push_back(root.to_string());
    let mut out = Vec::new();
    while let Some(cur) = queue.pop_front() {
        let Some(node) = graph.nodes.get(&cur) else {
            continue;
        };
        if node.planetary {
            let mut path = vec![cur.clone()];
            let mut at = &cur;
            while let Some(p) = parent.get(at) {
                path.push(p.clone());
                at = p;
            }
            path.reverse();
            out.push(CanaryViolation {
                root: root.to_string(),
                reached: cur.clone(),
                path,
            });
        }
        for d in &node.deps {
            if seen.insert(d.clone()) {
                parent.insert(d.clone(), cur.clone());
                queue.push_back(d.clone());
            }
        }
    }
    out.sort_by(|a, b| a.reached.cmp(&b.reached));
    Ok(out)
}

/// Builds the RESOLVED transitive graph. All dependency kinds (normal, dev, build) are
/// followed: a conservative over-approximation. Packages sharing a name (several versions
/// of a third-party crate) merge into one node: deps are unioned, planetary flag OR-ed.
pub fn canary_graph_from_manifest(manifest_path: &Path) -> Result<DepGraph, CheckError> {
    // Deliberately NO `.no_deps()`: the resolve graph is the whole point of the canary.
    let md = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .map_err(CheckError::Metadata)?;
    let resolve = md
        .resolve
        .as_ref()
        .ok_or_else(|| CheckError::Path("cargo metadata returned no resolve graph".into()))?;
    let packages: HashMap<&cargo_metadata::PackageId, &cargo_metadata::Package> =
        md.packages.iter().map(|p| (&p.id, p)).collect();
    let mut graph = DepGraph::default();
    for node in &resolve.nodes {
        let Some(pkg) = packages.get(&node.id) else {
            continue;
        };
        let name = pkg.name.to_string();
        let dir = pkg
            .manifest_path
            .parent()
            .map(|p| p.as_std_path().to_path_buf())
            .unwrap_or_default();
        let rel = dir
            .strip_prefix(md.workspace_root.as_std_path())
            .map(Path::to_path_buf)
            .unwrap_or(dir);
        let planetary = is_planetary(&name, &rel);
        let deps: Vec<String> = node
            .deps
            .iter()
            .filter_map(|d| packages.get(&d.pkg).map(|p| p.name.to_string()))
            .collect();
        let entry = graph.nodes.entry(name).or_default();
        entry.planetary |= planetary;
        entry.deps.extend(deps);
    }
    Ok(graph)
}

pub fn check_canary(
    manifest_path: &Path,
    package: &str,
) -> Result<Vec<CanaryViolation>, CheckError> {
    let graph = canary_graph_from_manifest(manifest_path)?;
    reachable_planetary(&graph, package)
}
