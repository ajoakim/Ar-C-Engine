//! Builds the planet-agnostic [`DiagnosticsSnapshot`] from planetary telemetry.
//!
//! Level and patch vocabulary stays here; the overlay UI only sees gauges, budgets and
//! histograms.

use engine_sdk::DiagnosticsSnapshot;
use planetary_terrain::StreamBudget;

use crate::telemetry::FrameSample;

/// Counts per level: bin `i` holds the number of cells at level `i`.
#[must_use]
pub fn level_histogram(levels: &[u8]) -> Vec<u32> {
    let Some(&max) = levels.iter().max() else {
        return Vec::new();
    };
    let mut bins = vec![0u32; usize::from(max) + 1];
    for &l in levels {
        bins[usize::from(l)] = bins[usize::from(l)].saturating_add(1);
    }
    bins
}

/// Build the overlay snapshot from one frame's data. Pure and deterministic.
#[must_use]
pub fn build(
    sample: &FrameSample,
    drawn_levels: &[u8],
    wanted_levels: &[u8],
    budget: StreamBudget,
    detail_budget_patches: u64,
) -> DiagnosticsSnapshot {
    let mut s = DiagnosticsSnapshot::default()
        .histogram("LOD drawn", level_histogram(drawn_levels))
        .histogram("LOD wanted", level_histogram(wanted_levels))
        .gauge("selected", sample.wanted_leaves as f64)
        .gauge("drawn", sample.drawn_leaves as f64)
        .gauge("queue pending", sample.pending as f64)
        .gauge("queue in flight", sample.in_flight as f64)
        .gauge("queue awaiting upload", sample.awaiting_upload as f64)
        .budget(
            "detail patches",
            sample.held_meshes as f64,
            detail_budget_patches as f64,
        )
        .budget(
            "uploads/frame",
            f64::from(sample.uploads),
            f64::from(budget.uploads),
        )
        .budget(
            "starts/frame",
            f64::from(sample.started),
            f64::from(budget.starts),
        );
    s.frame_ms = sample.frame_ms;
    s.prepare_ms = sample.prepare_ms;
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> (FrameSample, StreamBudget) {
        let sample = FrameSample {
            frame_ms: 8.0,
            prepare_ms: 1.5,
            wanted_leaves: 40,
            drawn_leaves: 30,
            held_meshes: 750,
            pending: 5,
            in_flight: 3,
            awaiting_upload: 2,
            uploads: 4,
            started: 6,
            ..FrameSample::default()
        };
        let budget = StreamBudget {
            uploads: 8,
            starts: 12,
            requests: 16,
            scale: 1.0,
        };
        (sample, budget)
    }

    #[test]
    fn maps_fields() {
        let (sample, budget) = inputs();
        let s = build(&sample, &[1, 1, 3], &[2, 3], budget, 1500);
        assert_eq!(s.histograms[0].label, "LOD drawn");
        assert_eq!(s.histograms[0].bins, vec![0, 2, 0, 1]);
        assert_eq!(s.histograms[1].label, "LOD wanted");
        assert_eq!(s.histograms[1].bins, vec![0, 0, 1, 1]);
        let g = |name: &str| s.gauges.iter().find(|(l, _)| l == name).map(|(_, v)| *v);
        assert_eq!(g("selected"), Some(40.0));
        assert_eq!(g("drawn"), Some(30.0));
        assert_eq!(g("queue pending"), Some(5.0));
        assert_eq!(g("queue in flight"), Some(3.0));
        assert_eq!(g("queue awaiting upload"), Some(2.0));
        let b = |name: &str| s.budgets.iter().find(|b| b.label == name).cloned().unwrap();
        assert_eq!(
            (b("detail patches").used, b("detail patches").cap),
            (750.0, 1500.0)
        );
        assert_eq!(
            (b("uploads/frame").used, b("uploads/frame").cap),
            (4.0, 8.0)
        );
        assert_eq!((b("starts/frame").used, b("starts/frame").cap), (6.0, 12.0));
        assert_eq!((s.frame_ms, s.prepare_ms), (8.0, 1.5));
    }

    #[test]
    fn deterministic_and_empty_levels_ok() {
        let (sample, budget) = inputs();
        assert_eq!(
            build(&sample, &[2], &[], budget, 10),
            build(&sample, &[2], &[], budget, 10)
        );
        assert!(level_histogram(&[]).is_empty());
    }
}
