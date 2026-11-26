//! Screen-space-error quadtree LOD over the six cube faces: error model, selection with
//! hysteresis, cross-face 2:1 balancing and reusable validators.

mod balance;
mod check;
mod cull;
mod model;
mod select;

pub use balance::{balance, leaves_of};
pub use check::{is_balanced, is_partition, max_level_jump, reference_views};
pub use cull::{
    Footprint, HeightRange, cell_bound, cell_distance, eye_in_cone, frustum_visible,
    horizon_visible, patch_footprint,
};
pub use model::{Bound, DEFAULT_KAPPA, LodModel, RESIDUAL_LEVELS, View};
pub use select::{LodParams, SelectStats, screen_space_error};
