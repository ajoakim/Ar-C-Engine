//! Geometric-error model `eps(L)`, node bounding spheres and the view description.

use core::f64::consts::FRAC_PI_2;

use engine_sdk::DVec3;

use crate::lod::cull::HeightRange;
use crate::terrain::{
    MOUNTAIN_SHAPE_SLOPE, PlanetTerrain, RANGE_HEIGHT_FREQS, RANGE_HEIGHT_GAIN, RANGE_HEIGHT_LO,
};

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
    ///
    /// Generator v3 (`generator_version >= 3`) keeps the v2 terms and adds
    /// `h * (1 - 0.6 g) + m * g`, `g = smoothstep(a, b, mask)`. Each added term is bounded as
    /// follows (`V_x` the fbm variation at the cell angle, `V_mask` the mask variation):
    /// - Gate: `0 <= g <= 1` and the smoothstep slope is at most `1.5 / (b - a)`, so
    ///   `|dg| <= min(1, 1.5 / (b - a) * V_mask)`.
    /// - Hills `h = A_h (2 f - 1)`: `|h| <= A_h`, `|dh| <= 2 A_h V_h`. The factor `1 - 0.6 g`
    ///   lies in `[0.4, 1]` and changes by `0.6 |dg|`. The product changes by at most
    ///   `A_h (2 V_h + 0.6 |dg|)`.
    /// - Mountains `m = A_m M r * r` (11-06D), `r = 1 - round_abs(2 f - 1)` in `[0, 1]`,
    ///   `M = range_height_at` in `[lo, 1]`. With `u = round_abs(x)`, `u' = x / sqrt(x^2 + e^2)`
    ///   in `[-1, 1]`: `d(r r)/df = -4 r u'`, so `r r` is `4 c`-Lipschitz in `f` with
    ///   `c = max (1 - u) u' = MOUNTAIN_SHAPE_SLOPE` (0.785 rounded up to 0.8; the generic bound
    ///   would be 4). `r r` is `C1` and flat where `r = 0`, so the mean value theorem applies.
    ///   `M` is a clamped affine map of the mean of three one-octave fields, so
    ///   `|dM| <= (1 - lo) G mean(V_i)` with `G` the range-height gain.
    ///   `d(M r r) = M d(r r) + r r dM`, `M <= 1`, `r r <= 1`, so `|dm| <= A_m (4 c V_m' +
    ///   (1 - lo) G mean(V_i))`; also `m` lies in `[0, A_m]` (since `M <= 1`), so the change is at
    ///   most `A_m min(4 c V_m' + (1 - lo) G mean(V_i), 1)`. With the gate factor in `[0, 1]`,
    ///   `|d(m g)| <= A_m (min(..., 1) + |dg|)`.
    /// - Span: both heights lie in `[sea, top]`, so no change exceeds `top - sea`; the v3
    ///   residual is capped there (v1/v2 are left as they were).
    #[must_use]
    pub fn from_terrain(t: &PlanetTerrain, kappa: f64) -> LodModel {
        Self::for_topology(t, kappa, FRAC_PI_2 / f64::from(t.cells_per_edge()))
    }

    /// Like [`LodModel::from_terrain`] for a topology whose level-0 sample spacing angle is
    /// `sample_angle0` (`Topology::sample_angle0`).
    #[must_use]
    pub fn for_topology(t: &PlanetTerrain, kappa: f64, sample_angle0: f64) -> LodModel {
        let p = t.params();
        let top = p.max_relief();
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
            // Relief exaggeration (v3 only) multiplies the ridge, hills and mountains terms, which
            // are linear in their amplitudes, so each bound scales by the same factor.
            let rs = if p.generator_version >= 3 {
                p.relief_scale
            } else {
                1.0
            };
            if p.generator_version >= 2 {
                // Generator v2 adds `ridge_amplitude * r * r * w` (`w = 0.25 + 0.75 * mask`) and
                // the detail term stays `detail * w`. `r = 1 - |2 f - 1|` is 2-Lipschitz in the
                // fbm value `f`, and `r <= 1` makes `r * r` 4-Lipschitz, so the ridge shape
                // varies by at most `4 V_r`. `w <= 1` and varies by `0.75 V_m`, with
                // `r * r <= 1`: the product varies by at most `A_r * (4 V_r + 0.75 V_m)`.
                let vr = fbm_variation(p.ridge_frequency, p.ridge_octaves, s);
                model.residual[level] += p.ridge_amplitude * rs * (4.0 * vr + 0.75 * vm);
            }
            if p.generator_version >= 3 {
                // See the "Generator v3" section of the `from_terrain` documentation.
                let lip = 1.5 / (p.range_gate_hi - p.range_gate_lo);
                let dg = (lip * vm).min(1.0);
                let vh = fbm_variation(p.hills_frequency, p.hills_octaves, s);
                let vmt = fbm_variation(p.mountains_frequency, p.mountains_octaves, s);
                let hills = p.hills_amplitude * rs * (2.0 * vh + 0.6 * dg);
                // Range height M in [LO, 1] is (1 - LO) * GAIN-Lipschitz in the mean of three
                // one-octave fields, so |dM| <= (1 - LO) * GAIN * mean(V_i).
                let vrange = RANGE_HEIGHT_FREQS
                    .iter()
                    .map(|f| fbm_variation(*f, 1, s))
                    .sum::<f64>()
                    / 3.0;
                let dm = (1.0 - RANGE_HEIGHT_LO) * RANGE_HEIGHT_GAIN * vrange;
                let mountains =
                    p.mountains_amplitude * rs * (4.0 * MOUNTAIN_SHAPE_SLOPE * vmt + dm).min(1.0)
                        + p.mountains_amplitude * rs * dg;
                model.residual[level] += hills + mountains;
                let span = model.height.max - model.height.min;
                if model.residual[level] > span {
                    model.residual[level] = span;
                }
            }
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

    /// Largest change `alpha` of a plane normal between the two views (`|n_new - n_old|`, a chord,
    /// the Lipschitz constant of the plane value in the distance from the eye), or `None` when
    /// exactly one view has a frustum. `Some(0.0)` for two views without one.
    #[must_use]
    pub fn orientation_delta(&self, other: &View) -> Option<f64> {
        match (self.frustum, other.frustum) {
            (None, None) => Some(0.0),
            (Some(a), Some(b)) => Some(plane_alpha(&a, &b)),
            _ => None,
        }
    }

    /// `None` when exactly one view has a frustum; `Some(None)` when neither has; otherwise
    /// `Some(Some((alpha, shift)))` where `alpha` is [`View::orientation_delta`] and `shift` the
    /// largest `|(n' - n) . e' + (d' - d)|` over the planes (`e'` the eye of `other`): the plane
    /// value at the eye of `other`, so `|p'(x) - p(x)| <= shift + alpha * |x - e'|`.
    #[must_use]
    pub fn plane_change(&self, other: &View) -> Option<Option<(f64, f64)>> {
        match (self.frustum, other.frustum) {
            (None, None) => Some(None),
            (Some(a), Some(b)) => {
                let e = other.eye;
                let mut shift = 0.0f64;
                for (p, q) in a.iter().zip(b.iter()) {
                    let s = (q[0] - p[0]) * e.x
                        + (q[1] - p[1]) * e.y
                        + (q[2] - p[2]) * e.z
                        + (q[3] - p[3]);
                    let s = if s < 0.0 { -s } else { s };
                    if s.is_nan() || s > shift {
                        shift = if s.is_nan() { f64::INFINITY } else { s };
                    }
                }
                Some(Some((plane_alpha(&a, &b), shift)))
            }
            _ => None,
        }
    }

    /// Whether the two views project alike (same `proj_scale`, bit for bit).
    #[must_use]
    pub fn same_projection(&self, other: &View) -> bool {
        self.proj_scale.to_bits() == other.proj_scale.to_bits()
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

/// Largest `|n_b - n_a|` over the six planes (infinite for NaN).
fn plane_alpha(a: &[[f64; 4]; 6], b: &[[f64; 4]; 6]) -> f64 {
    let mut alpha = 0.0f64;
    for (p, q) in a.iter().zip(b.iter()) {
        let dx = q[0] - p[0];
        let dy = q[1] - p[1];
        let dz = q[2] - p[2];
        let n = (dx * dx + dy * dy + dz * dz).sqrt();
        if n.is_nan() {
            return f64::INFINITY;
        }
        if n > alpha {
            alpha = n;
        }
    }
    alpha
}
