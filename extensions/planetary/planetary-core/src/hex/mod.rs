//! Hex grid topology: packed cell ids and the icosahedron/diamond tables (HEX-01).
//!
//! Integer math only; nothing here is platform dependent (ADR-0008, ADR-0011).

pub mod adjacency;
pub mod cell_id;
pub mod icosa;

pub use cell_id::{CellId, CellIdError, HEX_MAX_LEVEL};
pub use icosa::{DIAMONDS, Diamond, FACES};
