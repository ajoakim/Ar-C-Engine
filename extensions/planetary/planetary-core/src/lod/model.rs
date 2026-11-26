//! Geometric-error model `eps(L)`, node bounding spheres and the view description.

use core::f64::consts::FRAC_PI_2;

use engine_sdk::DVec3;

use crate::lod::cull::HeightRange;
use crate::terrain::PlanetTerrain;

/// Default fraction of the cell size used as the geometric error.
pub const DEFAULT_KAPPA: f64 = 0.25;

/// Number of levels covered by [`LodModel::residual`] (`0..=24`).
pub const RESIDUAL_LEVELS: usize = 25;

/// Parameters of the geometric-error model (meters).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodModel {
    pub radius: f64,
    /// Conservative global height range above `radius`.
    pub height: HeightRange,
    /// `residual[L]`: bound on the height change between any point of a level-`L` patch and the
    /// nearest of its grid samples (see [`LodModel::from_terrain`]).
    pub residual: [f64; RESIDUAL_LEVELS],
    pub cells_per_edge: u32,
    /// Sample spacing angle (radians) at level 0; level `L` uses `sample_angle0 / 2^L`.
    pub sample_angle0: f64,
    pub kappa: f64,
}

/// Bounding sphere of a patch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bound {
    pub center: DVec3,
    pub radius: f64,
}

/// Max gradient of the value noise per unit coordinate: the fade `t*t*(3 - 2t)` has slope at most
/// `1.5`, corner values lie in `[0, 1)`, so each partial derivative is at most `1.5` and the
/// gradient norm at most `1.5 * sqrt(3)`.
fn noise_gradient_bound() -> f64 {
    1.5 * 3.0f64.sqrt()
}

/// Upper bound of the change of a normalised fbm (values in `[0, 1)`) between two unit
/// directions at angular distance `s`: `sum_k w_k * min(1, G * f * 2^k * s)` with
/// `w_k = 0.5^k / sum_j 0.5^j`. Powers are formed by repeated multiplication.
fn fbm_variation(freq: f64, octaves: u32, s: f64) -> f64 {
    let g = noise_gradient_bound();
    let mut norm = 0.0;
    let mut w = 1.0;
    for _ in 0..octaves {
        norm += w;
        w *= 0.5;
    }
    if norm <= 0.0 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut w = 1.0;
    let mut scale = freq;
    for _ in 0..octaves {
        let step = g * scale * s;
        sum += w * if step < 1.0 { step } else { 1.0 };
        w *= 0.5;
        scale *= 2.0;
    }
    sum / norm
}

impl LodModel {
    /// Builds the model from the terrain.
    ///
    /// `height` is `[sea_level, max(base + detail amplitudes, sea_level)]`: `compose` floors at
    /// sea level and the layers are bounded by their amplitudes.
    ///
    /// `residual[L]` bounds `|h(p) - h(q)|` for directions at angular distance
    /// `theta(L)` (the cell angle; every point of a level-`L` patch is within that of a grid
    /// sample). With `h = max(sea, base + detail * (0.25 + 0.75 * mask))`, base and detail being
    /// `A * (2 * fbm - 1)`: `2 A_b V_b + 2 A_d V_d + 0.75 A_d V_m`, where `V_x` is the fbm
    /// variation. The `max` with sea level is 1-Lipschitz, so the bound carries over.
    #[must_use]
    pub fn from_terrain(t: &PlanetTerrain, kappa: f64) -> LodModel {
        Self::for_topology(t, kappa, FRAC_PI_2 / f64::from(t.cells_per_edge()))
    }

    /// Like [`LodModel::from_terrain`] for a topology whose level-0 sample spacing angle is
    /// `sample_angle0` (`Topology::sample_angle0`).
    #[must_use]
    pub fn for_topology(t: &PlanetTerrain, kappa: f64, sample_angle0: f64) -> LodModel {
        let p = t.params();
        let top = p.base_amplitude + p.detail_amplitude;
        let height = HeightRange {
            min: p.sea_level,
            max: if top > p.sea_level { top } else { p.sea_level },
        };
        let mut model = LodModel {
            radius: p.radius,
            height,
            residual: [0.0; RESIDUAL_LEVELS],
            cells_per_edge: t.cells_per_edge(),
            sample_angle0,
            kappa,
        };
        for level in 0..RESIDUAL_LEVELS {
            let s = model.theta(level as u8);
            let vb = fbm_variation(p.base_frequency, p.base_octaves, s);
            let vd = fbm_variation(p.detail_frequency, p.detail_octaves, s);
            let vm = fbm_variation(p.mask_frequency, p.mask_octaves, s);
            model.residual[level] = 2.0 * p.base_amplitude * vb
                + 2.0 * p.detail_amplitude * vd
                + 0.75 * p.detail_amplitude * vm;
        }
        model
    }

    fn theta(&self, level: u8) -> f64 {
        self.sample_angle0 / f64::from(1u32 << level)
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

    /// Height-variation bound within a level-`level` patch beyond its sample min/max.
    #[must_use]
    pub fn residual(&self, level: u8) -> f64 {
        let i = usize::from(level);
        if i < RESIDUAL_LEVELS {
            self.residual[i]
        } else {
            self.residual[RESIDUAL_LEVELS - 1]
        }
    }

    /// Height range valid for every descendant of (and the surface of) a patch at
    /// `ancestor_level` whose sample heights span `ancestor`: widened by the residual, limited to
    /// the global range.
    #[must_use]
    pub fn derived_range(&self, ancestor: HeightRange, ancestor_level: u8) -> HeightRange {
        ancestor
            .widen(self.residual(ancestor_level))
            .clamp_to(self.height)
    }
}

/// Camera data needed for selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub eye: DVec3,
    pub proj_scale: f64,
    /// Optional world-space frustum planes `[a, b, c, d]` with inward normals.
    pub frustum: Option<[[f64; 4]; 6]>,
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
            frustum: None,
        })
    }

    /// Attaches frustum planes; non-finite planes are rejected (returns `self` unchanged).
    #[must_use]
    pub fn with_frustum(self, planes: [[f64; 4]; 6]) -> View {
        if planes.iter().any(|p| p.iter().any(|v| !v.is_finite())) {
            return self;
        }
        View {
            frustum: Some(planes),
            ..self
        }
    }
}
