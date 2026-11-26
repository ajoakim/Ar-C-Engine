//! Workspace tooling: dependency-direction guardrails (WS-03).

pub mod canary;
pub mod check;
pub mod layers;

pub use canary::{CANARY_RULE_ID, CanaryViolation, DepGraph, check_canary};
pub use check::{
    CheckError, Rule, Violation, WorkspaceGraph, check_deps, evaluate, graph_from_manifest,
};
