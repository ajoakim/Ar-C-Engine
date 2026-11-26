//! Geometric-error model `eps(L)`, node bounding spheres and the view description.

use core::f64::consts::FRAC_PI_2;

use engine_sdk::DVec3;

use crate::mapping::{patch_center_dir, patch_corner_dirs};
use crate::patch_id::PatchId;
use crate::terrain::PlanetTerrain;

/// Default fraction of the cell size used as the geometric error.
pub const DEFAULT_KAPPA: f64 = 0.25;

/// Parameters of the geometric-error model (meters).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodModel {
    pub radius: f64,
    pub max_relief: f64,
    pub cells_per_edge: u32,
    pub kappa: f64,
}

/// Bounding sphere of a patch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bound {
    pub center: DVec3,
    pub radius: f64,
}

impl LodModel {
    #[must_use]
    pub fn from_terrain(t: &PlanetTerrain, kappa: f64) -> LodModel {
        LodModel {
            radius: t.params().radius,
            max_relief: t.params().max_relief(),
            cells_per_edge: t.cells_per_edge(),
            kappa,
        }
    }

    fn theta(&self, level: u8) -> f64 {
        FRAC_PI_2 / (f64::from(self.cells_per_edge) * f64::from(1u32 << level))
    }

    /// Cell size in meters at `level` (arc length at the reference radius).
    #[must_use]
    pub fn cell_size(&self, level: u8) -> f64 {
        self.radius * self.theta(level)
    }

    /// Chord sagitta of one cell at `level`.
    #[must_use]
    pub fn sagitta(&self, level: u8) -> f64 {
        let th = self.theta(level);
        self.radius * th * th / 8.0
    }

    /// `max(kappa * cell_size, sagitta)`.
    #[must_use]
    pub fn geometric_error(&self, level: u8) -> f64 {
        let a = self.kappa * self.cell_size(level);
        let s = self.sagitta(level);
        if a > s { a } else { s }
    }

    /// Bounding sphere inflated by the maximum relief and the sagitta.
    #[must_use]
    pub fn node_bound(&self, id: PatchId) -> Bound {
        let c = patch_center_dir(id);
        let center = DVec3::new(c.x * self.radius, c.y * self.radius, c.z * self.radius);
        let mut max_d = 0.0f64;
        for d in patch_corner_dirs(id) {
            let p = DVec3::new(d.x * self.radius, d.y * self.radius, d.z * self.radius);
            let l = (p - center).length();
            if l > max_d {
                max_d = l;
            }
        }
        Bound {
            center,
            radius: max_d + self.max_relief + self.sagitta(id.level()),
        }
    }
}

/// Camera data needed for selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub eye: DVec3,
    pub proj_scale: f64,
}

impl View {
    /// `proj_scale = viewport_height_px / (2 * tan_half_fov_y)`; `None` for non-finite or
    /// non-positive inputs.
    #[must_use]
    pub fn new(eye: DVec3, viewport_height_px: u32, tan_half_fov_y: f64) -> Option<View> {
        if !(eye.x.is_finite() && eye.y.is_finite() && eye.z.is_finite()) {
            return None;
        }
        if viewport_height_px == 0 || !tan_half_fov_y.is_finite() || tan_half_fov_y <= 0.0 {
            return None;
        }
        Some(View {
            eye,
            proj_scale: f64::from(viewport_height_px) / (2.0 * tan_half_fov_y),
        })
    }
}
