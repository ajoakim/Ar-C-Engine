//! Screen-space-error selection with hysteresis and a bounded leaf count.

use std::collections::BTreeSet;

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
