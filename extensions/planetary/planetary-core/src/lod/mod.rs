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
    Footprint, HeightRange, cell_bound, cell_distance, cone_margin_m, eye_in_cone, frustum_margin,
    frustum_visible, horizon_eye_cap, horizon_margin_m, horizon_visible, patch_footprint,
    sse_margin_m,
};
pub use model::{Bound, DEFAULT_KAPPA, LodModel, RESIDUAL_LEVELS, View};
pub use select::{
    LodParams, SelectSlack, SelectStats, morph_factor, refine_distance, screen_space_error,
};
