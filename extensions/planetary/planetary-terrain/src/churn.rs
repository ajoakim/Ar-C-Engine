//! Per-frame churn counters of the drawn cut (07.1-20 flicker diagnosis).
//!
//! Opt-in (`TerrainConfig::diagnostics`). Everything is a set or map keyed by cell (BTree only),
//! history is pruned to [`HISTORY_FRAMES`] frames, and no clock is read.

use std::collections::{BTreeMap, BTreeSet};

use planetary_core::{Piece, Topology};

/// Frames of history kept for flip-flop detection.
pub const HISTORY_FRAMES: u64 = 8;
/// Ancestor levels walked when matching a refine or coarsen event.
const ANCESTOR_LEVELS: usize = 4;

/// Churn of one frame versus the previous one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChurnCounts {
    /// Refine events (an appeared cell with an ancestor that disappeared) plus coarsen events
    /// (a disappeared cell with an ancestor that appeared).
    pub level_changes: u32,
    /// Cells in the drawn set now and not last frame.
    pub drawn_appeared: u32,
    /// Cells in the drawn set last frame and not now.
    pub drawn_disappeared: u32,
    /// Cells drawn in both frames whose culled flag flipped.
    pub cull_toggles: u32,
    /// Drawn pieces that overlap another drawn piece (see [`count_overlaps`]).
    pub overlap_pairs: u32,
    /// Appeared cells that disappeared within the last [`HISTORY_FRAMES`] frames.
    pub flipflops: u32,
    /// Flip-flops with no explanation (trim, budget hit, coarsen stage, eviction of a relative).
    pub flipflops_unexplained: u32,
}

/// Frame-level facts that can explain a flip-flop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChurnFlags {
    /// Selection trimmed splits, hit the node or leaf budget, rejected retained cells for the
    /// budget, or the retention age bound expired a cell; or the effective cut coarsened.
    pub budget_or_trim: bool,
}

/// Number of overlapping drawn pieces: pieces drawn twice for the same cell, drawn cells that are
/// also split, and whole pieces that have a drawn ancestor. A half piece (a piece whose cell has
/// two parents of which only one is split) legitimately coexists with its other, unsplit parent,
/// which is drawn whole, so a drawn direct parent only counts against a cell with a single parent
/// or with all parents drawn-or-split (it is then drawn whole). Ancestors are walked through
/// split cells only.
#[must_use]
pub fn count_overlaps<T: Topology>(
    topo: &T,
    pieces: &[Piece<T::Cell>],
    split: &BTreeSet<T::Cell>,
) -> u32 {
    let drawn: BTreeSet<T::Cell> = pieces.iter().map(|p| p.cell).collect();
    let mut n = 0u32;
    // Same cell drawn twice.
    if drawn.len() < pieces.len() {
        n += u32::try_from(pieces.len() - drawn.len()).unwrap_or(u32::MAX);
    }
    let mut ps = Vec::new();
    for &c in &drawn {
        if split.contains(&c) {
            n += 1;
            continue;
        }
        ps.clear();
        topo.parents(c, &mut ps);
        // Whole when it has one parent, or every parent is split (a drawn parent means that
        // parent is unsplit, i.e. this cell is a half piece of the other parent).
        let direct_drawn = ps.iter().filter(|p| drawn.contains(p)).count();
        if ps.len() == 1 && direct_drawn == 1 {
            n += 1;
        }
        // Climb through split ancestors only.
        let mut frontier: Vec<T::Cell> = ps.iter().copied().filter(|p| split.contains(p)).collect();
        let mut seen: BTreeSet<T::Cell> = BTreeSet::new();
        let mut buf = Vec::new();
        while let Some(a) = frontier.pop() {
            if !seen.insert(a) {
                continue;
            }
            if drawn.contains(&a) {
                n += 1;
                continue;
            }
            buf.clear();
            topo.parents(a, &mut buf);
            for &q in &buf {
                if split.contains(&q) {
                    frontier.push(q);
                } else if drawn.contains(&q) && buf.len() == 1 {
                    n += 1;
                }
            }
        }
    }
    n
}

/// All parents up to [`ANCESTOR_LEVELS`] levels above `c`.
fn ancestors<T: Topology>(topo: &T, c: T::Cell) -> BTreeSet<T::Cell> {
    let mut out = BTreeSet::new();
    let mut layer = vec![c];
    let mut buf = Vec::new();
    for _ in 0..ANCESTOR_LEVELS {
        let mut next = Vec::new();
        for &x in &layer {
            buf.clear();
            topo.parents(x, &mut buf);
            for &p in &buf {
                if out.insert(p) {
                    next.push(p);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        layer = next;
    }
    out
}

/// Frame-to-frame state of the churn counters.
pub struct ChurnTracker<C: Ord + Copy> {
    prev_drawn: BTreeSet<C>,
    prev_culled: BTreeSet<C>,
    gone_at: BTreeMap<C, u64>,
    destroyed_at: BTreeMap<C, u64>,
    last_flag_frame: Option<u64>,
}

impl<C: Ord + Copy> Default for ChurnTracker<C> {
    fn default() -> Self {
        Self {
            prev_drawn: BTreeSet::new(),
            prev_culled: BTreeSet::new(),
            gone_at: BTreeMap::new(),
            destroyed_at: BTreeMap::new(),
            last_flag_frame: None,
        }
    }
}

impl<C: Ord + Copy> ChurnTracker<C> {
    /// Observes the drawn set of `frame` and returns the churn versus the previous call.
    ///
    /// `pieces` are the resident drawn pieces, `drawn` their cells (culled ones included),
    /// `culled` the drawn cells skipped by draw culling, `split` the split set of the drawn cut,
    /// `destroyed` the cells whose meshes were destroyed this frame.
    #[allow(clippy::too_many_arguments)]
    pub fn observe<T: Topology<Cell = C>>(
        &mut self,
        topo: &T,
        frame: u64,
        pieces: &[Piece<C>],
        drawn: &BTreeSet<C>,
        culled: BTreeSet<C>,
        split: &BTreeSet<C>,
        flags: ChurnFlags,
        destroyed: &[C],
    ) -> ChurnCounts {
        let mut out = ChurnCounts::default();
        let appeared: Vec<C> = drawn.difference(&self.prev_drawn).copied().collect();
        let disappeared: Vec<C> = self.prev_drawn.difference(drawn).copied().collect();
        out.drawn_appeared = u32::try_from(appeared.len()).unwrap_or(u32::MAX);
        out.drawn_disappeared = u32::try_from(disappeared.len()).unwrap_or(u32::MAX);

        let appeared_set: BTreeSet<C> = appeared.iter().copied().collect();
        let gone_set: BTreeSet<C> = disappeared.iter().copied().collect();
        for &a in &appeared {
            if ancestors(topo, a).iter().any(|p| gone_set.contains(p)) {
                out.level_changes += 1;
            }
        }
        for &d in &disappeared {
            if ancestors(topo, d).iter().any(|p| appeared_set.contains(p)) {
                out.level_changes += 1;
            }
        }
        out.cull_toggles = u32::try_from(
            drawn
                .intersection(&self.prev_drawn)
                .filter(|c| culled.contains(c) != self.prev_culled.contains(c))
                .count(),
        )
        .unwrap_or(u32::MAX);
        out.overlap_pairs = count_overlaps(topo, pieces, split);

        for &d in destroyed {
            self.destroyed_at.insert(d, frame);
        }
        if flags.budget_or_trim {
            self.last_flag_frame = Some(frame);
        }
        let flag_recent = self
            .last_flag_frame
            .is_some_and(|f| frame.saturating_sub(f) <= HISTORY_FRAMES);
        let recent_destroyed = |x: &C, at: &BTreeMap<C, u64>| {
            at.get(x)
                .is_some_and(|f| frame.saturating_sub(*f) <= HISTORY_FRAMES)
        };
        let mut kin = Vec::new();
        for &a in &appeared {
            let Some(&gone) = self.gone_at.get(&a) else {
                continue;
            };
            if frame.saturating_sub(gone) > HISTORY_FRAMES {
                continue;
            }
            out.flipflops += 1;
            let mut explained = flag_recent || recent_destroyed(&a, &self.destroyed_at);
            if !explained {
                kin.clear();
                topo.parents(a, &mut kin);
                topo.children(a, &mut kin);
                explained = kin.iter().any(|k| recent_destroyed(k, &self.destroyed_at));
            }
            if !explained {
                out.flipflops_unexplained += 1;
            }
        }
        for &d in &disappeared {
            self.gone_at.insert(d, frame);
        }
        self.gone_at
            .retain(|_, f| frame.saturating_sub(*f) <= HISTORY_FRAMES);
        self.destroyed_at
            .retain(|_, f| frame.saturating_sub(*f) <= HISTORY_FRAMES);
        self.prev_drawn.clone_from(drawn);
        self.prev_culled = culled;
        out
    }
}
