//! Planet-space terrain: three independently seeded layers (base shape, detail, mask) built on
//! `engine_sdk::procedural`, composed by one pure `height_at_dir` of the unit direction.
//!
//! Heights depend only on the direction (never on a patch-local coordinate or on the LOD level),
//! so vertices shared by two patches agree bit-for-bit.

use std::fmt;

use engine_sdk::DVec3;
use engine_sdk::procedural::{
    Generator, LayerStack, Params, ProcError, StableId, fbm_3d, layer_seed,
};

use crate::mapping::{is_valid_cells_per_edge, vertex_dir};
use crate::patch_id::PatchId;

pub const BASE_LAYER: &str = "planet.base";
pub const DETAIL_LAYER: &str = "planet.detail";
pub const MASK_LAYER: &str = "planet.mask";
pub const LAYER_VERSION: u32 = 1;
/// Ridged mid-scale mountain-range layer (generator version 2 only).
pub const RIDGE_LAYER: &str = "planet.ridge";
pub const RIDGE_LAYER_VERSION: u32 = 2;
/// Small-scale rolling-hills layer (generator version 3 only).
pub const HILLS_LAYER: &str = "planet.hills";
/// Large-scale mountain layer gated by the mask (generator version 3 only).
pub const MOUNTAINS_LAYER: &str = "planet.mountains";
pub const V3_LAYER_VERSION: u32 = 3;
/// Smallest accepted [`TerrainParams::relief_scale`].
pub const RELIEF_SCALE_MIN: f64 = 0.1;
/// Largest accepted [`TerrainParams::relief_scale`].
pub const RELIEF_SCALE_MAX: f64 = 8.0;

/// Terrain configuration (meters). See [`TerrainParams::validate`] for the accepted ranges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainParams {
    pub radius: f64,
    pub sea_level: f64,
    pub base_amplitude: f64,
    pub base_frequency: f64,
    pub base_octaves: u32,
    pub detail_amplitude: f64,
    pub detail_frequency: f64,
    pub detail_octaves: u32,
    pub mask_frequency: f64,
    pub mask_octaves: u32,
    /// Generator version: 1 (base + detail + mask) or 2 (adds the ridge layer and surface v2).
    pub generator_version: u32,
    /// Ridge layer (generator version 2 only; ignored for version 1).
    pub ridge_amplitude: f64,
    pub ridge_frequency: f64,
    pub ridge_octaves: u32,
    /// Hills layer (generator version 3 only; ignored below 3). Starting estimates.
    pub hills_amplitude: f64,
    pub hills_frequency: f64,
    pub hills_octaves: u32,
    /// Mountains layer (generator version 3 only; ignored below 3). Starting estimates.
    pub mountains_amplitude: f64,
    pub mountains_frequency: f64,
    pub mountains_octaves: u32,
    /// Range gate: `smoothstep(range_gate_lo, range_gate_hi, mask)` (generator version 3 only).
    pub range_gate_lo: f64,
    pub range_gate_hi: f64,
    /// Relief exaggeration (generator version 3 only; ignored below 3): multiplies the ridge,
    /// hills and mountains amplitudes and nothing else. 1.0 is the calibrated Earth-like relief
    /// and is bit-identical to the output before this field existed. Accepted range
    /// [`RELIEF_SCALE_MIN`]..=[`RELIEF_SCALE_MAX`] (user decision 2026-11-27, ADR-0020).
    pub relief_scale: f64,
}

impl Default for TerrainParams {
    fn default() -> Self {
        Self {
            radius: 6_371_000.0,
            sea_level: -200.0,
            base_amplitude: 3000.0,
            base_frequency: 1.5,
            base_octaves: 5,
            detail_amplitude: 800.0,
            detail_frequency: 24.0,
            detail_octaves: 14,
            mask_frequency: 3.0,
            mask_octaves: 3,
            generator_version: 1,
            ridge_amplitude: 2500.0,
            ridge_frequency: 12.0,
            ridge_octaves: 5,
            hills_amplitude: 600.0,
            hills_frequency: 200.0,
            hills_octaves: 4,
            mountains_amplitude: 3900.0,
            mountains_frequency: 470.0,
            mountains_octaves: 5,
            range_gate_lo: 0.44,
            range_gate_hi: 0.74,
            relief_scale: 1.0,
        }
    }
}

impl TerrainParams {
    /// Generator version 2: the default parameters plus the ridge mountain-range layer
    /// (amplitude 2500 m, frequency 12, 5 octaves: features of about 530 km down to 33 km).
    #[must_use]
    pub fn demo_v2() -> Self {
        Self {
            generator_version: 2,
            ..Self::default()
        }
    }

    /// Generator version 3: `demo_v2` plus hills and mountains layers. All v3 constants
    /// (ridge 1800 m / 3 octaves, hills 600 m f200 x4, mountains 3900 m f470 x5, gate 0.44..0.74;
    /// the v3 ridge and mountains shapes use the rounded crease, 11-06C)
    /// are calibrated by `tests/relief_v3.rs` at `relief_scale` 1.0 (the optional exaggeration).
    #[must_use]
    pub fn demo_v3() -> Self {
        Self {
            generator_version: 3,
            ridge_amplitude: 1800.0,
            ridge_octaves: 3,
            ..Self::demo_v2()
        }
    }

    /// Upper bound of any height above the reference radius.
    #[must_use]
    pub fn max_relief(&self) -> f64 {
        if self.generator_version >= 3 {
            let s = self.relief_scale;
            return self.base_amplitude
                + self.detail_amplitude
                + self.ridge_amplitude * s
                + self.hills_amplitude * s
                + self.mountains_amplitude * s;
        }
        if self.generator_version >= 2 {
            self.base_amplitude + self.detail_amplitude + self.ridge_amplitude
        } else {
            self.base_amplitude + self.detail_amplitude
        }
    }

    /// Rejects non-finite values, non-positive radius/frequencies/amplitudes, octave counts
    /// outside `1..=24` and `frequency * 2^octaves >= 2^40` (keeps noise coordinates far below
    /// the `2^52` precondition of the value noise).
    pub fn validate(&self) -> Result<(), TerrainError> {
        let finite = [
            self.radius,
            self.sea_level,
            self.base_amplitude,
            self.base_frequency,
            self.detail_amplitude,
            self.detail_frequency,
            self.mask_frequency,
        ];
        if finite.iter().any(|v| !v.is_finite()) {
            return Err(TerrainError::InvalidParams("non-finite parameter"));
        }
        if self.radius <= 0.0 {
            return Err(TerrainError::InvalidParams("radius must be positive"));
        }
        if self.base_amplitude < 0.0 || self.detail_amplitude < 0.0 {
            return Err(TerrainError::InvalidParams("amplitude must be >= 0"));
        }
        if !(1..=3).contains(&self.generator_version) {
            return Err(TerrainError::InvalidParams(
                "generator_version must be 1, 2 or 3",
            ));
        }
        let mut layers = vec![
            (self.base_frequency, self.base_octaves),
            (self.detail_frequency, self.detail_octaves),
            (self.mask_frequency, self.mask_octaves),
        ];
        if self.generator_version >= 2 {
            if !self.ridge_amplitude.is_finite() || !self.ridge_frequency.is_finite() {
                return Err(TerrainError::InvalidParams("non-finite parameter"));
            }
            if self.ridge_amplitude <= 0.0 {
                return Err(TerrainError::InvalidParams(
                    "ridge amplitude must be positive",
                ));
            }
            layers.push((self.ridge_frequency, self.ridge_octaves));
        }
        if self.generator_version >= 3 {
            let v3 = [
                self.hills_amplitude,
                self.hills_frequency,
                self.mountains_amplitude,
                self.mountains_frequency,
                self.range_gate_lo,
                self.range_gate_hi,
                self.relief_scale,
            ];
            if v3.iter().any(|v| !v.is_finite()) {
                return Err(TerrainError::InvalidParams("non-finite parameter"));
            }
            if self.hills_amplitude < 0.0 || self.mountains_amplitude < 0.0 {
                return Err(TerrainError::InvalidParams("amplitude must be >= 0"));
            }
            if !(RELIEF_SCALE_MIN..=RELIEF_SCALE_MAX).contains(&self.relief_scale) {
                return Err(TerrainError::InvalidParams(
                    "relief_scale must be in 0.1..=8.0",
                ));
            }
            if !(0.0 <= self.range_gate_lo
                && self.range_gate_lo < self.range_gate_hi
                && self.range_gate_hi <= 1.0)
            {
                return Err(TerrainError::InvalidParams(
                    "range gate must satisfy 0 <= lo < hi <= 1",
                ));
            }
            layers.push((self.hills_frequency, self.hills_octaves));
            layers.push((self.mountains_frequency, self.mountains_octaves));
        }
        for (freq, octaves) in layers {
            if freq <= 0.0 {
                return Err(TerrainError::InvalidParams("frequency must be positive"));
            }
            if !(1..=24).contains(&octaves) {
                return Err(TerrainError::InvalidParams("octaves must be in 1..=24"));
            }
            if freq * ((1u64 << octaves) as f64) >= 1_099_511_627_776.0 {
                return Err(TerrainError::InvalidParams(
                    "frequency * 2^octaves must be < 2^40",
                ));
            }
        }
        Ok(())
    }
}

/// Per-layer seeds derived from the world seed (`layer_seed(world, name, version)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerSeeds {
    pub base: u64,
    pub detail: u64,
    pub mask: u64,
    /// Ridge layer seed (0 for generator version 1).
    pub ridge: u64,
    /// Hills layer seed (0 below generator version 3).
    pub hills: u64,
    /// Mountains layer seed (0 below generator version 3).
    pub mountains: u64,
}

/// Height (meters above the reference radius) and mask at one direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainSample {
    pub height: f64,
    pub mask: f64,
}

#[derive(Debug)]
pub enum TerrainError {
    InvalidParams(&'static str),
    CellsPerEdge(u32),
    Proc(ProcError),
}

impl fmt::Display for TerrainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerrainError::InvalidParams(why) => write!(f, "invalid terrain parameters: {why}"),
            TerrainError::CellsPerEdge(n) => {
                write!(
                    f,
                    "cells per edge must be a power of two in 2..=64, got {n}"
                )
            }
            TerrainError::Proc(e) => write!(f, "procedural stack error: {e}"),
        }
    }
}

impl std::error::Error for TerrainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TerrainError::Proc(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ProcError> for TerrainError {
    fn from(e: ProcError) -> Self {
        TerrainError::Proc(e)
    }
}

// BORDER-CRITICAL BEGIN
/// `base_amplitude * (2 * fbm - 1)`.
#[must_use]
pub fn base_at(seed: u64, p: &TerrainParams, d: DVec3) -> f64 {
    let f = p.base_frequency;
    let v = fbm_3d(seed, d.x * f, d.y * f, d.z * f, p.base_octaves);
    p.base_amplitude * (2.0 * v - 1.0)
}

/// `detail_amplitude * (2 * fbm - 1)`.
#[must_use]
pub fn detail_at(seed: u64, p: &TerrainParams, d: DVec3) -> f64 {
    let f = p.detail_frequency;
    let v = fbm_3d(seed, d.x * f, d.y * f, d.z * f, p.detail_octaves);
    p.detail_amplitude * (2.0 * v - 1.0)
}

/// Mask in `[0, 1)`.
#[must_use]
pub fn mask_at(seed: u64, p: &TerrainParams, d: DVec3) -> f64 {
    let f = p.mask_frequency;
    fbm_3d(seed, d.x * f, d.y * f, d.z * f, p.mask_octaves)
}

/// Ridge term before the amplitude: `r * r` with `r = 1 - |2 * fbm - 1|`, in `[0, 1]`.
#[must_use]
pub fn ridge_shape(seed: u64, p: &TerrainParams, d: DVec3) -> f64 {
    let f = p.ridge_frequency;
    let v = fbm_3d(seed, d.x * f, d.y * f, d.z * f, p.ridge_octaves);
    let r = 1.0 - (2.0 * v - 1.0).abs();
    r * r
}

/// `ridge_amplitude * r * r`; the mask weighting is applied in [`compose_v2`].
#[must_use]
pub fn ridge_at(seed: u64, p: &TerrainParams, d: DVec3) -> f64 {
    p.ridge_amplitude * ridge_shape(seed, p, d)
}

/// Generator v2 composition: `(base + detail * w + ridge * w)` with `w = 0.25 + 0.75 * mask`,
/// floored at sea level. `ridge` is the value returned by [`ridge_at`].
#[must_use]
pub fn compose_v2(p: &TerrainParams, base: f64, detail: f64, mask: f64, ridge: f64) -> f64 {
    let w = 0.25 + 0.75 * mask;
    (base + detail * w + ridge * w).max(p.sea_level)
}

/// `hills_amplitude * (2 * fbm - 1)`.
#[must_use]
pub fn hills_at(seed: u64, p: &TerrainParams, d: DVec3) -> f64 {
    let f = p.hills_frequency;
    let v = fbm_3d(seed, d.x * f, d.y * f, d.z * f, p.hills_octaves);
    p.hills_amplitude * (2.0 * v - 1.0)
}

/// `mountains_amplitude * r * r` with `r = 1 - round_abs(2 * fbm - 1)`; gated in [`compose_v3`].
#[must_use]
pub fn mountains_at(seed: u64, p: &TerrainParams, d: DVec3) -> f64 {
    let f = p.mountains_frequency;
    let v = fbm_3d(seed, d.x * f, d.y * f, d.z * f, p.mountains_octaves);
    let r = 1.0 - round_abs(2.0 * v - 1.0);
    p.mountains_amplitude * (r * r)
}

/// Crease rounding width of the v3 ridge and mountains shapes (ADR-0020, 11-06C).
pub const CREASE_EPS: f64 = 0.1;

/// Smooth `|x|`: `sqrt(x * x + eps^2) - eps`. It is 1-Lipschitz like `abs`, zero at zero and
/// at most `|x|`, so `r = 1 - round_abs(x)` stays in `[0, 1]` for `|x| <= 1`. Only exact IEEE
/// operations (`*`, `+`, `-`, `sqrt`), so it is bit-identical across platforms.
#[must_use]
pub fn round_abs(x: f64) -> f64 {
    (x * x + CREASE_EPS * CREASE_EPS).sqrt() - CREASE_EPS
}

/// Generator v3 ridge shape: [`ridge_shape`] with the `|x|` crease rounded by [`round_abs`].
/// [`ridge_shape`] itself stays untouched for generator v2.
#[must_use]
pub fn ridge_shape_v3(seed: u64, p: &TerrainParams, d: DVec3) -> f64 {
    let f = p.ridge_frequency;
    let v = fbm_3d(seed, d.x * f, d.y * f, d.z * f, p.ridge_octaves);
    let r = 1.0 - round_abs(2.0 * v - 1.0);
    r * r
}

/// `ridge_amplitude * ridge_shape_v3`; the v3 counterpart of [`ridge_at`].
#[must_use]
pub fn ridge_at_v3(seed: u64, p: &TerrainParams, d: DVec3) -> f64 {
    p.ridge_amplitude * ridge_shape_v3(seed, p, d)
}

/// Polynomial smoothstep (comparisons and arithmetic only); requires `b > a`.
#[must_use]
pub fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = (x - a) / (b - a);
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Generator v3 composition: the v2 sum plus `hills * (1 - 0.6 g) + mountains * g`, with
/// `g = smoothstep(range_gate_lo, range_gate_hi, mask)`, floored at sea level. `ridge`, `hills`
/// and `mtn` are multiplied by `relief_scale` first (exact at 1.0).
#[must_use]
pub fn compose_v3(
    p: &TerrainParams,
    base: f64,
    detail: f64,
    mask: f64,
    ridge: f64,
    hills: f64,
    mtn: f64,
) -> f64 {
    let w = 0.25 + 0.75 * mask;
    let g = smoothstep(p.range_gate_lo, p.range_gate_hi, mask);
    let s = p.relief_scale;
    let (ridge, hills, mtn) = (ridge * s, hills * s, mtn * s);
    (base + detail * w + ridge * w + hills * (1.0 - 0.6 * g) + mtn * g).max(p.sea_level)
}

/// Combines the layers: `(base + detail * (0.25 + 0.75 * mask))`, floored at sea level.
#[must_use]
pub fn compose(p: &TerrainParams, base: f64, detail: f64, mask: f64) -> f64 {
    (base + detail * (0.25 + 0.75 * mask)).max(p.sea_level)
}

impl PlanetTerrain {
    /// Height and mask at unit direction `d`.
    #[must_use]
    pub fn sample(&self, d: DVec3) -> TerrainSample {
        let p = &self.params;
        let base = base_at(self.seeds.base, p, d);
        let detail = detail_at(self.seeds.detail, p, d);
        let mask = mask_at(self.seeds.mask, p, d);
        let height = if p.generator_version >= 3 {
            let ridge = ridge_at_v3(self.seeds.ridge, p, d);
            let hills = hills_at(self.seeds.hills, p, d);
            let mtn = mountains_at(self.seeds.mountains, p, d);
            compose_v3(p, base, detail, mask, ridge, hills, mtn)
        } else if p.generator_version >= 2 {
            let ridge = ridge_at(self.seeds.ridge, p, d);
            compose_v2(p, base, detail, mask, ridge)
        } else {
            compose(p, base, detail, mask)
        };
        TerrainSample { height, mask }
    }

    /// Pure terrain height at unit direction `d`.
    #[must_use]
    pub fn height_at_dir(&self, d: DVec3) -> f64 {
        self.sample(d).height
    }

    /// Surface position: `d * (radius + height)`, component-wise.
    #[must_use]
    pub fn surface_point(&self, d: DVec3) -> DVec3 {
        let r = self.params.radius + self.height_at_dir(d);
        DVec3::new(d.x * r, d.y * r, d.z * r)
    }
}
// BORDER-CRITICAL END

/// Fills `out` (row-major `(n + 1)^2`) with `f(direction)` for patch `raw`; zeros when the id
/// is invalid (`Generator::generate` cannot report errors).
fn fill_patch(raw: u64, n: u32, out: &mut [f64], f: impl Fn(DVec3) -> f64) {
    let Ok(id) = PatchId::from_raw(raw) else {
        out.fill(0.0);
        return;
    };
    let w = n + 1;
    for (k, slot) in out.iter_mut().enumerate() {
        let Ok(k) = u32::try_from(k) else {
            *slot = 0.0;
            continue;
        };
        *slot = f(vertex_dir(id, n, k % w, k / w));
    }
}

fn layer_len(n: u32) -> usize {
    ((u64::from(n) + 1) * (u64::from(n) + 1)) as usize
}

/// Large-scale shape layer, one value per patch vertex.
///
/// Configured by its immutable fields; the runtime `Params` argument is ignored.
#[derive(Clone, Copy, Debug)]
pub struct BaseLayer {
    pub params: TerrainParams,
    pub cells_per_edge: u32,
}

impl Generator for BaseLayer {
    fn name(&self) -> &'static str {
        BASE_LAYER
    }
    fn version(&self) -> u32 {
        LAYER_VERSION
    }
    fn output_len(&self) -> usize {
        layer_len(self.cells_per_edge)
    }
    fn generate(&self, id: StableId, seed: u64, _params: &Params, out: &mut [f64]) {
        let p = self.params;
        fill_patch(id.0, self.cells_per_edge, out, |d| base_at(seed, &p, d));
    }
}

/// High-frequency detail layer, one value per patch vertex.
///
/// Configured by its immutable fields; the runtime `Params` argument is ignored.
#[derive(Clone, Copy, Debug)]
pub struct DetailLayer {
    pub params: TerrainParams,
    pub cells_per_edge: u32,
}

impl Generator for DetailLayer {
    fn name(&self) -> &'static str {
        DETAIL_LAYER
    }
    fn version(&self) -> u32 {
        LAYER_VERSION
    }
    fn output_len(&self) -> usize {
        layer_len(self.cells_per_edge)
    }
    fn generate(&self, id: StableId, seed: u64, _params: &Params, out: &mut [f64]) {
        let p = self.params;
        fill_patch(id.0, self.cells_per_edge, out, |d| detail_at(seed, &p, d));
    }
}

/// Detail-strength mask layer in [0, 1), one value per patch vertex.
///
/// Configured by its immutable fields; the runtime `Params` argument is ignored.
#[derive(Clone, Copy, Debug)]
pub struct MaskLayer {
    pub params: TerrainParams,
    pub cells_per_edge: u32,
}

impl Generator for MaskLayer {
    fn name(&self) -> &'static str {
        MASK_LAYER
    }
    fn version(&self) -> u32 {
        LAYER_VERSION
    }
    fn output_len(&self) -> usize {
        layer_len(self.cells_per_edge)
    }
    fn generate(&self, id: StableId, seed: u64, _params: &Params, out: &mut [f64]) {
        let p = self.params;
        fill_patch(id.0, self.cells_per_edge, out, |d| mask_at(seed, &p, d));
    }
}

/// Ridged mountain-range layer (generator version 2), one value per patch vertex.
///
/// Configured by its immutable fields; the runtime `Params` argument is ignored.
#[derive(Clone, Copy, Debug)]
pub struct RidgeLayer {
    pub params: TerrainParams,
    pub cells_per_edge: u32,
}

impl Generator for RidgeLayer {
    fn name(&self) -> &'static str {
        RIDGE_LAYER
    }
    fn version(&self) -> u32 {
        RIDGE_LAYER_VERSION
    }
    fn output_len(&self) -> usize {
        layer_len(self.cells_per_edge)
    }
    fn generate(&self, id: StableId, seed: u64, _params: &Params, out: &mut [f64]) {
        let p = self.params;
        if p.generator_version >= 3 {
            fill_patch(id.0, self.cells_per_edge, out, |d| ridge_at_v3(seed, &p, d));
        } else {
            fill_patch(id.0, self.cells_per_edge, out, |d| ridge_at(seed, &p, d));
        }
    }
}

/// Small-scale hills layer (generator version 3), one value per patch vertex.
///
/// Configured by its immutable fields; the runtime `Params` argument is ignored.
#[derive(Clone, Copy, Debug)]
pub struct HillsLayer {
    pub params: TerrainParams,
    pub cells_per_edge: u32,
}

impl Generator for HillsLayer {
    fn name(&self) -> &'static str {
        HILLS_LAYER
    }
    fn version(&self) -> u32 {
        V3_LAYER_VERSION
    }
    fn output_len(&self) -> usize {
        layer_len(self.cells_per_edge)
    }
    fn generate(&self, id: StableId, seed: u64, _params: &Params, out: &mut [f64]) {
        let p = self.params;
        fill_patch(id.0, self.cells_per_edge, out, |d| hills_at(seed, &p, d));
    }
}

/// Large-scale mountains layer (generator version 3), one value per patch vertex.
///
/// Configured by its immutable fields; the runtime `Params` argument is ignored.
#[derive(Clone, Copy, Debug)]
pub struct MountainsLayer {
    pub params: TerrainParams,
    pub cells_per_edge: u32,
}

impl Generator for MountainsLayer {
    fn name(&self) -> &'static str {
        MOUNTAINS_LAYER
    }
    fn version(&self) -> u32 {
        V3_LAYER_VERSION
    }
    fn output_len(&self) -> usize {
        layer_len(self.cells_per_edge)
    }
    fn generate(&self, id: StableId, seed: u64, _params: &Params, out: &mut [f64]) {
        let p = self.params;
        fill_patch(id.0, self.cells_per_edge, out, |d| {
            mountains_at(seed, &p, d)
        });
    }
}

/// A configured planet: parameters, derived layer seeds and the generator stack.
pub struct PlanetTerrain {
    params: TerrainParams,
    seeds: LayerSeeds,
    cells_per_edge: u32,
    stack: LayerStack,
}

impl PlanetTerrain {
    pub fn new(
        world_seed: u64,
        params: TerrainParams,
        cells_per_edge: u32,
    ) -> Result<Self, TerrainError> {
        params.validate()?;
        if !is_valid_cells_per_edge(cells_per_edge) {
            return Err(TerrainError::CellsPerEdge(cells_per_edge));
        }
        let mut stack = LayerStack::new(world_seed)
            .with_layer(Box::new(BaseLayer {
                params,
                cells_per_edge,
            }))?
            .with_layer(Box::new(DetailLayer {
                params,
                cells_per_edge,
            }))?
            .with_layer(Box::new(MaskLayer {
                params,
                cells_per_edge,
            }))?;
        let v2 = params.generator_version >= 2;
        if v2 {
            stack = stack.with_layer(Box::new(RidgeLayer {
                params,
                cells_per_edge,
            }))?;
        }
        let v3 = params.generator_version >= 3;
        if v3 {
            stack = stack
                .with_layer(Box::new(HillsLayer {
                    params,
                    cells_per_edge,
                }))?
                .with_layer(Box::new(MountainsLayer {
                    params,
                    cells_per_edge,
                }))?;
        }
        let seeds = LayerSeeds {
            base: layer_seed(world_seed, BASE_LAYER, LAYER_VERSION),
            detail: layer_seed(world_seed, DETAIL_LAYER, LAYER_VERSION),
            mask: layer_seed(world_seed, MASK_LAYER, LAYER_VERSION),
            ridge: if v2 {
                layer_seed(world_seed, RIDGE_LAYER, RIDGE_LAYER_VERSION)
            } else {
                0
            },
            hills: if v3 {
                layer_seed(world_seed, HILLS_LAYER, V3_LAYER_VERSION)
            } else {
                0
            },
            mountains: if v3 {
                layer_seed(world_seed, MOUNTAINS_LAYER, V3_LAYER_VERSION)
            } else {
                0
            },
        };
        Ok(Self {
            params,
            seeds,
            cells_per_edge,
            stack,
        })
    }

    #[must_use]
    pub fn params(&self) -> &TerrainParams {
        &self.params
    }

    #[must_use]
    pub fn seeds(&self) -> LayerSeeds {
        self.seeds
    }

    #[must_use]
    pub fn cells_per_edge(&self) -> u32 {
        self.cells_per_edge
    }

    #[must_use]
    pub fn stack(&self) -> &LayerStack {
        &self.stack
    }

    /// `(n + 1)^2` samples, row-major index `j * (n + 1) + i`, produced through the layer stack.
    #[must_use]
    pub fn patch_samples(&self, id: PatchId) -> Vec<TerrainSample> {
        let out = self.stack.generate(StableId(id.raw()), &Params::new());
        let (Some(b), Some(d), Some(m)) = (
            out.layer(BASE_LAYER),
            out.layer(DETAIL_LAYER),
            out.layer(MASK_LAYER),
        ) else {
            return Vec::new();
        };
        if self.params.generator_version >= 3 {
            let (Some(r), Some(h), Some(mt)) = (
                out.layer(RIDGE_LAYER),
                out.layer(HILLS_LAYER),
                out.layer(MOUNTAINS_LAYER),
            ) else {
                return Vec::new();
            };
            return b
                .iter()
                .zip(d)
                .zip(m)
                .zip(r)
                .zip(h)
                .zip(mt)
                .map(
                    |(((((&base, &detail), &mask), &ridge), &hills), &mtn)| TerrainSample {
                        height: compose_v3(&self.params, base, detail, mask, ridge, hills, mtn),
                        mask,
                    },
                )
                .collect();
        }
        if self.params.generator_version >= 2 {
            let Some(r) = out.layer(RIDGE_LAYER) else {
                return Vec::new();
            };
            return b
                .iter()
                .zip(d)
                .zip(m)
                .zip(r)
                .map(|(((&base, &detail), &mask), &ridge)| TerrainSample {
                    height: compose_v2(&self.params, base, detail, mask, ridge),
                    mask,
                })
                .collect();
        }
        b.iter()
            .zip(d)
            .zip(m)
            .map(|((&base, &detail), &mask)| TerrainSample {
                height: compose(&self.params, base, detail, mask),
                mask,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn planet_terrain_is_send_sync() {
        assert_send_sync::<PlanetTerrain>();
    }
}
