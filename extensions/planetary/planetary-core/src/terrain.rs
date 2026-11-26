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
        }
    }
}

impl TerrainParams {
    /// Upper bound of any height above the reference radius.
    #[must_use]
    pub fn max_relief(&self) -> f64 {
        self.base_amplitude + self.detail_amplitude
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
        let layers = [
            (self.base_frequency, self.base_octaves),
            (self.detail_frequency, self.detail_octaves),
            (self.mask_frequency, self.mask_octaves),
        ];
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
        TerrainSample {
            height: compose(p, base, detail, mask),
            mask,
        }
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
        let stack = LayerStack::new(world_seed)
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
        let seeds = LayerSeeds {
            base: layer_seed(world_seed, BASE_LAYER, LAYER_VERSION),
            detail: layer_seed(world_seed, DETAIL_LAYER, LAYER_VERSION),
            mask: layer_seed(world_seed, MASK_LAYER, LAYER_VERSION),
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
