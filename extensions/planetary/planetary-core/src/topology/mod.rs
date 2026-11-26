//! The cell-hierarchy contract that selection, balancing and meshing are generic over.
//!
//! A topology is a hierarchy of cells covering the sphere: a fixed set of root cells, each cell
//! refined into children. The cube-sphere quadtree ([`cube::CubeSphere`]) is the first
//! implementation. Everything here is kept minimal; a method exists only when selection,
//! balance or meshing needs it.

pub mod cube;
pub mod select;

use std::collections::BTreeSet;

use engine_sdk::{DVec3, VertexPosNormalUv};

use crate::lod::{Footprint, LodModel};
use crate::morph::MorphVertex;
use crate::terrain::PlanetTerrain;

/// One drawable piece of a selection: a cell drawn with one index variant.
///
/// For the cube-sphere every piece is a whole leaf cell with variant 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Piece<C> {
    pub cell: C,
    pub variant: u16,
}

/// One shared index buffer. `vertex_class` selects the vertex layout the indices refer to
/// (cells of one class have the same vertex count).
#[derive(Clone, Debug, PartialEq)]
pub struct IndexVariant {
    pub name: &'static str,
    pub vertex_class: u8,
    pub indices: Vec<u32>,
}

/// A built cell: f64 origin plus f32 offsets relative to it (no indices, see [`IndexVariant`]).
#[derive(Clone, Debug, PartialEq)]
pub struct CellMesh {
    pub raw: u64,
    pub vertex_class: u8,
    pub origin: DVec3,
    pub vertices: Vec<VertexPosNormalUv>,
    /// Geomorph target per vertex (same length and order as `vertices`): the parent-level surface
    /// at the vertex's direction (see [`crate::morph`]). Identity (position and uv copied) for
    /// roots and for meshes decoded from a v3 payload.
    pub morph: Vec<MorphVertex>,
    pub bound_radius: f32,
    pub skirt_depth: f64,
    /// Measured min/max of the sample heights (skirts excluded).
    pub height_min: f64,
    pub height_max: f64,
}

/// A hierarchy of cells over the sphere.
///
/// Contract (every implementation must satisfy it; the selection code relies on it):
/// - `roots` is non-empty and fixed; every other cell reaches a root through `primary_parent`.
/// - `level(root) == 0`; every child has level `level(parent) + 1`.
/// - `raw`/`from_raw` round-trip; `from_raw` rejects anything that is not a valid cell.
/// - `required_splits(c)` lists cells that must be split whenever `c` is split (parents and the
///   cells that keep neighbouring levels within one of each other). All are strictly shallower
///   than `c`, which is what makes the balance closure and the budget trim terminate.
pub trait Topology: Send + Sync + 'static {
    type Cell: Copy + Ord + Eq + core::fmt::Debug + Send + Sync + 'static;

    /// Short stable name for logs and telemetry.
    fn name(&self) -> &'static str;
    /// The level-0 cells.
    fn roots(&self) -> &[Self::Cell];
    fn level(&self, c: Self::Cell) -> u8;
    /// Stable 64-bit identifier, unique per cell.
    fn raw(&self, c: Self::Cell) -> u64;
    #[allow(clippy::wrong_self_convention)] // the topology is the decoding context
    fn from_raw(&self, raw: u64) -> Option<Self::Cell>;
    /// Appends the children of `c` (empty at the deepest level): 4 for a quad.
    fn children(&self, c: Self::Cell, out: &mut Vec<Self::Cell>);
    /// Appends the parents of `c`, ascending raw (0 for roots, 1 for a tree, up to 2 for a hex).
    fn parents(&self, c: Self::Cell, out: &mut Vec<Self::Cell>);
    /// The parent with the lowest raw id.
    fn primary_parent(&self, c: Self::Cell) -> Option<Self::Cell>;
    /// Appends the same-level cells sharing an edge with `c`.
    fn neighbors(&self, c: Self::Cell, out: &mut Vec<Self::Cell>);
    /// Appends the cells that must be split if `c` is split; all strictly shallower than `c`.
    fn required_splits(&self, c: Self::Cell, out: &mut Vec<Self::Cell>);
    /// Whether selection charges the piece budget for the cells the closure forces at the moment
    /// a cell is split, instead of only repairing the set afterwards. Topologies whose closure is
    /// large (several forced splits per split) need this to spend the budget on the cells that
    /// matter; the cube-sphere keeps the original accounting.
    fn eager_closure(&self) -> bool {
        false
    }
    /// Cone containing the cell on the unit sphere.
    fn footprint(&self, c: Self::Cell) -> Footprint;
    /// The cell of `level` containing the unit direction `dir`.
    fn locate(&self, dir: DVec3, level: u8) -> Self::Cell;
    /// Sample spacing angle (radians) at level 0 for this topology's grid size.
    fn sample_angle0(&self) -> f64;
    /// Appends the pieces drawn for the closed split set `split`, ascending.
    fn pieces(&self, split: &BTreeSet<Self::Cell>, out: &mut Vec<Piece<Self::Cell>>);
    /// The shared index buffers, indexed by [`Piece::variant`].
    fn index_variants(&self) -> Vec<IndexVariant>;
    /// Builds the mesh of `c`, or `None` when `cancelled` returned true while building.
    ///
    /// With `debug_cells`, `uv[1]` becomes a per-cell, per-level palette row selector
    /// `(((level * 7 + (raw >> 7)) % 16) + 0.5) / 16`; otherwise uv is the terrain attribute.
    fn build_mesh(
        &self,
        c: Self::Cell,
        terrain: &PlanetTerrain,
        model: &LodModel,
        debug_cells: bool,
        cancelled: &dyn Fn() -> bool,
    ) -> Option<CellMesh>;
}
