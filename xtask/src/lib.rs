//! Workspace tooling: dependency-direction guardrails (WS-03).

pub mod check;
pub mod layers;

pub use check::{
    CheckError, Rule, Violation, WorkspaceGraph, check_deps, evaluate, graph_from_manifest,
};
