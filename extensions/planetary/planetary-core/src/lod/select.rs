//! Screen-space-error selection with hysteresis and a bounded leaf count.

use std::collections::BTreeSet;

use crate::face::Face;
use crate::lod::balance::{balance, leaves_of};
use crate::lod::model::{LodModel, View};
use crate::patch_id::PatchId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodParams {
    pub tau_px: f64,
    pub hysteresis: f64,
    pub max_level: u8,
    pub max_leaves: usize,
    pub tau_growth: f64,
    pub max_tau_steps: u32,
    pub min_distance: f64,
}

impl Default for LodParams {
    fn default() -> Self {
        Self {
            tau_px: 2.0,
            hysteresis: 0.2,
            max_level: 19,
            max_leaves: 700,
            tau_growth: 1.5,
            max_tau_steps: 8,
            min_distance: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SelectStats {
    pub nodes_visited: u32,
    pub leaves: usize,
    pub forced_splits: u32,
    pub tau_eff: f64,
    pub tau_steps: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    pub split: BTreeSet<PatchId>,
    pub leaves: Vec<PatchId>,
    pub stats: SelectStats,
}

/// `eps(L) * proj_scale / max(|eye - c| - r, min_distance)`.
#[must_use]
pub fn screen_space_error(model: &LodModel, view: &View, id: PatchId, min_distance: f64) -> f64 {
    let b = model.node_bound(id);
    let d = (view.eye - b.center).length() - b.radius;
    let d = if d > min_distance { d } else { min_distance };
    model.geometric_error(id.level()) * view.proj_scale / d
}

/// Selects the split set for `view`. A node splits when `rho > tau`, or, if it was split in
/// `prev_split`, while `rho > tau * (1 - hysteresis)`. The result is balanced (2:1) and, if the
/// leaf count exceeds `max_leaves`, `tau` is escalated by `tau_growth` up to `max_tau_steps`.
#[must_use]
pub fn select(
    model: &LodModel,
    params: &LodParams,
    view: &View,
    prev_split: &BTreeSet<PatchId>,
) -> Selection {
    let mut tau = params.tau_px;
    let mut visited = 0u32;
    let mut forced_total = 0u32;
    let mut step = 0u32;
    loop {
        let mut split = BTreeSet::new();
        let mut stack: Vec<PatchId> = Face::ALL.iter().rev().map(|f| PatchId::root(*f)).collect();
        while let Some(p) = stack.pop() {
            visited = visited.saturating_add(1);
            if p.level() >= params.max_level {
                continue;
            }
            let rho = screen_space_error(model, view, p, params.min_distance);
            let thresh = if prev_split.contains(&p) {
                tau * (1.0 - params.hysteresis)
            } else {
                tau
            };
            if rho > thresh {
                split.insert(p);
                if let Some(ch) = p.children() {
                    stack.extend(ch.iter().rev());
                }
            }
        }
        forced_total = forced_total.saturating_add(balance(&mut split));
        let leaves = leaves_of(&split);
        if leaves.len() <= params.max_leaves || step >= params.max_tau_steps {
            let stats = SelectStats {
                nodes_visited: visited,
                leaves: leaves.len(),
                forced_splits: forced_total,
                tau_eff: tau,
                tau_steps: step,
            };
            return Selection {
                split,
                leaves,
                stats,
            };
        }
        tau *= params.tau_growth;
        step += 1;
    }
}
