//! Screen-space-error quadtree LOD over the six cube faces: error model, selection with
//! hysteresis, cross-face 2:1 balancing and reusable validators.

mod balance;
mod check;
mod model;
mod select;

pub use balance::{balance, leaves_of};
pub use check::{is_balanced, is_partition, max_level_jump, reference_views};
pub use model::{Bound, DEFAULT_KAPPA, LodModel, View};
pub use select::{LodParams, SelectStats, Selection, screen_space_error, select};
