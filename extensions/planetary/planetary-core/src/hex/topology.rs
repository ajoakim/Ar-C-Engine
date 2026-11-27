//! The hex grid as a [`Topology`]: twelve level-0 pentagons, refined with aperture 4 (a centre
//! child plus one child per edge).
//!
//! Every method delegates to the hex modules; the variant index of a [`Piece`] is the position in
//! [`hex_index_variants`].

use core::f64::consts::PI;
use std::collections::BTreeSet;

use engine_sdk::DVec3;

use crate::hex::cell_id::CellId;
use crate::hex::mapping::{self, MAX_SECTOR_N};
use crate::hex::mesh::{build_hex_cell, hex_index_variants};
use crate::hex::{adjacency, pieces};
use crate::lod::{Footprint, LodModel};
use crate::terrain::PlanetTerrain;
use crate::topology::{CellMesh, IndexVariant, Piece, Topology};

/// Slots of the per-thread footprint memo (a power of two).
const FOOTPRINT_SLOTS: u64 = 1 << 14;

thread_local! {
    /// Direct-mapped memo of [`mapping::footprint`]: `(raw + 1, footprint)` per slot, `0` marks an
    /// empty slot. Allocated on first use by the calling thread. `mapping::footprint` is a pure
    /// function of the cell, so a hit returns exactly the value a miss computes: the memo changes
    /// the cost of the call, never its result (07.1-27).
    static FOOTPRINT_MEMO: core::cell::RefCell<Vec<(u64, Footprint)>> =
        const { core::cell::RefCell::new(Vec::new()) };
}

/// Index of `raw` in the footprint memo (multiplicative hash, top bits).
/// Sample spacing angle at level 0 of a hex sphere with sector side `n` (`Topology::sample_angle0`).
pub(crate) fn hex_sample_angle0(n: u32) -> f64 {
    (8.0 * PI / (10.0 * 3f64.sqrt())).sqrt() / (3f64.sqrt() * f64::from(n))
}

fn footprint_slot(raw: u64) -> usize {
    let h = raw.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - FOOTPRINT_SLOTS.trailing_zeros());
    usize::try_from(h).unwrap_or(0)
}

/// [`mapping::footprint`] through the per-thread memo; bit-identical to the direct call.
#[must_use]
pub fn cached_footprint(c: CellId) -> Footprint {
    let key = c.raw().wrapping_add(1);
    let slot = footprint_slot(c.raw());
    FOOTPRINT_MEMO.with(|m| {
        let mut m = m.borrow_mut();
        if m.is_empty() {
            let empty = Footprint {
                center: DVec3::ZERO,
                cos_delta: 0.0,
                sin_delta: 0.0,
            };
            m.resize(usize::try_from(FOOTPRINT_SLOTS).unwrap_or(0), (0, empty));
        }
        if let Some(entry) = m.get(slot)
            && entry.0 == key
        {
            return entry.1;
        }
        let fp = mapping::footprint(c);
        if let Some(entry) = m.get_mut(slot) {
            *entry = (key, fp);
        }
        fp
    })
}

/// Twelve pentagon roots; each cell is drawn as sector grids of side `n`.
#[derive(Clone, Debug)]
pub struct HexSphere {
    n: u32,
    roots: Vec<CellId>,
}

impl HexSphere {
    /// `n` is the sector side: even, `2..=64`. Anything else returns `None`.
    #[must_use]
    pub fn new(n: u32) -> Option<HexSphere> {
        if !(2..=MAX_SECTOR_N).contains(&n) || !n.is_multiple_of(2) {
            return None;
        }
        Some(HexSphere {
            n,
            roots: CellId::level_iter(0).collect(),
        })
    }

    #[must_use]
    pub fn sector_side(&self) -> u32 {
        self.n
    }
}

impl Topology for HexSphere {
    type Cell = CellId;

    fn name(&self) -> &'static str {
        "hex"
    }

    fn roots(&self) -> &[CellId] {
        &self.roots
    }

    fn level(&self, c: CellId) -> u8 {
        c.level()
    }

    fn raw(&self, c: CellId) -> u64 {
        c.raw()
    }

    fn from_raw(&self, raw: u64) -> Option<CellId> {
        CellId::from_raw(raw).ok()
    }

    fn children(&self, c: CellId, out: &mut Vec<CellId>) {
        adjacency::children(c, out);
    }

    fn parents(&self, c: CellId, out: &mut Vec<CellId>) {
        adjacency::parents(c, out);
    }

    fn primary_parent(&self, c: CellId) -> Option<CellId> {
        adjacency::primary_parent(c)
    }

    fn neighbors(&self, c: CellId, out: &mut Vec<CellId>) {
        adjacency::neighbors(c, out);
    }

    fn required_splits(&self, c: CellId, out: &mut Vec<CellId>) {
        pieces::required_splits(c, out);
    }

    fn eager_closure(&self) -> bool {
        // The hex closure forces about 4.6 cells per split (07.1-06); charging it up front keeps
        // the budget for the near-camera chain instead of trimming it afterwards.
        true
    }

    fn footprint(&self, c: CellId) -> Footprint {
        cached_footprint(c)
    }

    fn locate(&self, dir: DVec3, level: u8) -> CellId {
        mapping::locate(dir, level)
    }

    fn sample_angle0(&self) -> f64 {
        hex_sample_angle0(self.n)
    }

    fn pieces(&self, split: &BTreeSet<CellId>, out: &mut Vec<Piece<CellId>>) {
        pieces::pieces(split, out);
    }

    fn index_variants(&self) -> Vec<IndexVariant> {
        hex_index_variants(self.n)
    }

    fn build_mesh(
        &self,
        c: CellId,
        terrain: &PlanetTerrain,
        model: &LodModel,
        debug_cells: bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<CellMesh> {
        build_hex_cell(c, terrain, model, debug_cells, cancelled)
    }
}
