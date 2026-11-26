//! Developer-facing generator API: pure, independently seeded layers (ADR-0008).

use std::collections::BTreeMap;

use crate::error::ProcError;
use crate::hash::{Fnv1a64, canonical_bits};
use crate::seed::{StableId, layer_seed};

/// Named f64 parameters; ordered (`BTreeMap`) so iteration and equality are deterministic.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Params {
    values: BTreeMap<String, f64>,
}

impl Params {
    /// Empty parameter set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set `name` to `value` (builder style).
    #[must_use]
    pub fn with(mut self, name: &str, value: f64) -> Self {
        self.values.insert(name.to_string(), value);
        self
    }

    /// Look up a parameter.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<f64> {
        self.values.get(name).copied()
    }

    /// Look up a parameter, falling back to `default`.
    #[must_use]
    pub fn get_or(&self, name: &str, default: f64) -> f64 {
        self.get(name).unwrap_or(default)
    }
}

/// A deterministic generator layer: output = f(id, seed, params). Must be pure:
/// no globals, clocks, thread-locals, RNG state or I/O; only ADR-0008 float ops.
pub trait Generator: Send + Sync {
    /// Seed input; unique within a stack.
    fn name(&self) -> &'static str;
    /// Bump to deliberately change this layer's output (re-pin goldens).
    fn version(&self) -> u32;
    /// Number of f64 values written per id (>= 1).
    fn output_len(&self) -> usize;
    /// Write exactly `out.len() == output_len()` values.
    fn generate(&self, id: StableId, seed: u64, params: &Params, out: &mut [f64]);
}

/// An ordered composition of uniquely named layers sharing a world seed.
pub struct LayerStack {
    world_seed: u64,
    layers: Vec<Box<dyn Generator>>,
}

impl LayerStack {
    /// Empty stack for `world_seed`.
    #[must_use]
    pub fn new(world_seed: u64) -> Self {
        Self {
            world_seed,
            layers: Vec::new(),
        }
    }

    /// Append a layer.
    ///
    /// # Errors
    /// `DuplicateLayer` if the name is taken, `EmptyLayer` if `output_len() == 0`.
    pub fn push(&mut self, layer: Box<dyn Generator>) -> Result<(), ProcError> {
        let name = layer.name();
        if layer.output_len() == 0 {
            return Err(ProcError::EmptyLayer(name));
        }
        if self.layers.iter().any(|l| l.name() == name) {
            return Err(ProcError::DuplicateLayer(name));
        }
        self.layers.push(layer);
        Ok(())
    }

    /// Builder form of [`LayerStack::push`].
    ///
    /// # Errors
    /// Same as [`LayerStack::push`].
    pub fn with_layer(mut self, layer: Box<dyn Generator>) -> Result<Self, ProcError> {
        self.push(layer)?;
        Ok(self)
    }

    /// World seed.
    #[must_use]
    pub fn world_seed(&self) -> u64 {
        self.world_seed
    }

    /// Layer names in declared order.
    #[must_use]
    pub fn layer_names(&self) -> Vec<&'static str> {
        self.layers.iter().map(|l| l.name()).collect()
    }

    /// Sum of all layers' `output_len`.
    #[must_use]
    pub fn output_len(&self) -> usize {
        self.layers.iter().map(|l| l.output_len()).sum()
    }

    /// Run every layer for `id`. Each layer's seed depends only on
    /// `(world_seed, name, version)`.
    #[must_use]
    pub fn generate(&self, id: StableId, params: &Params) -> LayerOutput {
        let mut layers = Vec::with_capacity(self.layers.len());
        for layer in &self.layers {
            let seed = layer_seed(self.world_seed, layer.name(), layer.version());
            let mut out = vec![0.0; layer.output_len()];
            layer.generate(id, seed, params, &mut out);
            layers.push((layer.name(), out));
        }
        LayerOutput { layers }
    }
}

/// Per-layer outputs of one `generate` call, in declared layer order.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerOutput {
    layers: Vec<(&'static str, Vec<f64>)>,
}

impl LayerOutput {
    /// Values of the named layer.
    #[must_use]
    pub fn layer(&self, name: &str) -> Option<&[f64]> {
        self.layers
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_slice())
    }

    /// All layers in declared order.
    #[must_use]
    pub fn layers(&self) -> &[(&'static str, Vec<f64>)] {
        &self.layers
    }

    /// FNV-1a-64 over, per layer in declared order: name length (u64 LE), name
    /// bytes, value count (u64 LE), canonical bits (u64 LE) of each value.
    ///
    /// # Errors
    /// `NonFinite` if any value is NaN or infinite.
    pub fn content_hash(&self) -> Result<u64, ProcError> {
        let mut h = Fnv1a64::new();
        for (name, values) in &self.layers {
            h.write_u64(name.len() as u64);
            h.write(name.as_bytes());
            h.write_u64(values.len() as u64);
            for (i, v) in values.iter().enumerate() {
                h.write_u64(bits_or_err(name, i, *v)?);
            }
        }
        Ok(h.finish())
    }

    /// Canonical little-endian bytes of all values in declared layer order.
    ///
    /// # Errors
    /// `NonFinite` if any value is NaN or infinite.
    pub fn to_le_bytes(&self) -> Result<Vec<u8>, ProcError> {
        let total: usize = self.layers.iter().map(|(_, v)| v.len()).sum();
        let mut bytes = Vec::with_capacity(total * 8);
        for (name, values) in &self.layers {
            for (i, v) in values.iter().enumerate() {
                bytes.extend_from_slice(&bits_or_err(name, i, *v)?.to_le_bytes());
            }
        }
        Ok(bytes)
    }
}

fn bits_or_err(layer: &'static str, index: usize, v: f64) -> Result<u64, ProcError> {
    canonical_bits(v).ok_or(ProcError::NonFinite {
        layer,
        index: index as u64,
    })
}

/// FNV-1a-64 over count (u64 LE) + canonical bits of `values`; `layer` labels errors.
///
/// # Errors
/// `NonFinite` if any value is NaN or infinite.
pub fn hash_values(layer: &'static str, values: &[f64]) -> Result<u64, ProcError> {
    let mut h = Fnv1a64::new();
    h.write_u64(values.len() as u64);
    for (i, v) in values.iter().enumerate() {
        h.write_u64(bits_or_err(layer, i, *v)?);
    }
    Ok(h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::unit_f64;

    struct Const {
        name: &'static str,
        len: usize,
        val: f64,
    }

    impl Generator for Const {
        fn name(&self) -> &'static str {
            self.name
        }
        fn version(&self) -> u32 {
            1
        }
        fn output_len(&self) -> usize {
            self.len
        }
        fn generate(&self, _id: StableId, _seed: u64, _p: &Params, out: &mut [f64]) {
            for (i, o) in out.iter_mut().enumerate() {
                *o = self.val + i as f64;
            }
        }
    }

    struct Probe;
    impl Generator for Probe {
        fn name(&self) -> &'static str {
            "probe"
        }
        fn version(&self) -> u32 {
            3
        }
        fn output_len(&self) -> usize {
            1
        }
        fn generate(&self, _id: StableId, seed: u64, _p: &Params, out: &mut [f64]) {
            out[0] = unit_f64(seed);
        }
    }

    fn c(name: &'static str, len: usize, val: f64) -> Box<dyn Generator> {
        Box::new(Const { name, len, val })
    }

    #[test]
    fn generator_duplicate_layer_rejected() {
        let mut s = LayerStack::new(1);
        s.push(c("base", 1, 0.0)).unwrap();
        assert_eq!(
            s.push(c("base", 1, 0.0)),
            Err(ProcError::DuplicateLayer("base"))
        );
    }

    #[test]
    fn generator_output_slices() {
        let s = LayerStack::new(1)
            .with_layer(c("a", 1, 5.0))
            .unwrap()
            .with_layer(c("b", 2, 7.0))
            .unwrap();
        assert_eq!(s.output_len(), 3);
        assert_eq!(s.layer_names(), vec!["a", "b"]);
        let o = s.generate(StableId(0), &Params::new());
        assert_eq!(o.layer("a").unwrap(), &[5.0]);
        assert_eq!(o.layer("b").unwrap(), &[7.0, 8.0]);
        assert!(o.layer("zzz").is_none());
    }

    #[test]
    fn generator_receives_layer_seed() {
        let s = LayerStack::new(42).with_layer(Box::new(Probe)).unwrap();
        let o = s.generate(StableId(9), &Params::new());
        assert_eq!(
            o.layer("probe").unwrap()[0],
            unit_f64(layer_seed(42, "probe", 3))
        );
    }

    #[test]
    fn generator_wrong_len_rejected() {
        let mut s = LayerStack::new(1);
        assert_eq!(s.push(c("e", 0, 0.0)), Err(ProcError::EmptyLayer("e")));
        s.push(c("two", 2, 0.0)).unwrap();
        let o = s.generate(StableId(0), &Params::new());
        assert_eq!(o.layer("two").unwrap().len(), 2);
    }

    #[test]
    fn generator_non_finite_rejected() {
        let s = LayerStack::new(1)
            .with_layer(c("nan", 1, f64::NAN))
            .unwrap();
        let o = s.generate(StableId(0), &Params::new());
        let e = ProcError::NonFinite {
            layer: "nan",
            index: 0,
        };
        assert_eq!(o.content_hash(), Err(e.clone()));
        assert_eq!(o.to_le_bytes(), Err(e));
    }

    #[test]
    fn generator_negative_zero_canonical() {
        let a = LayerStack::new(1).with_layer(c("z", 1, -0.0)).unwrap();
        let b = LayerStack::new(1).with_layer(c("z", 1, 0.0)).unwrap();
        let p = Params::new();
        assert_eq!(
            a.generate(StableId(0), &p).content_hash(),
            b.generate(StableId(0), &p).content_hash()
        );
    }

    #[test]
    fn generator_params_ordered() {
        let x = Params::new().with("b", 1.0).with("a", 2.0);
        let y = Params::new().with("a", 2.0).with("b", 1.0);
        assert_eq!(x, y);
        assert_eq!(x.get("a"), Some(2.0));
        assert_eq!(x.get_or("missing", 3.0), 3.0);
    }

    #[test]
    fn generator_to_le_bytes_layout() {
        let s = LayerStack::new(1)
            .with_layer(c("a", 1, 1.5))
            .unwrap()
            .with_layer(c("b", 2, 2.5))
            .unwrap();
        let o = s.generate(StableId(0), &Params::new());
        let bytes = o.to_le_bytes().unwrap();
        assert_eq!(bytes.len(), 24);
        assert_eq!(&bytes[0..8], &canonical_bits(1.5).unwrap().to_le_bytes());
    }
}
