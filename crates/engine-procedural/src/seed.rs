//! Layer seeding (ADR-0008): a layer seed depends only on world seed, layer
//! name and layer version, never on the layer's position in a stack.

use crate::hash::{fnv1a64, splitmix64};

/// Stable identifier of a generated item (opaque to this crate).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableId(pub u64);

/// Seed of one layer; depends only on `(world_seed, name, version)`.
#[must_use]
pub fn layer_seed(world_seed: u64, layer_name: &str, layer_version: u32) -> u64 {
    splitmix64(splitmix64(world_seed ^ fnv1a64(layer_name.as_bytes())) ^ u64::from(layer_version))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn seed_layer_seed_known_answer() {
        assert_eq!(layer_seed(0, "base", 1), 0xF174_0912_3F30_3493);
    }

    #[test]
    fn seed_layer_seed_isolation() {
        let b = layer_seed(7, "a", 1);
        assert_eq!(b, layer_seed(7, "a", 1));
        assert_ne!(b, layer_seed(8, "a", 1));
        assert_ne!(b, layer_seed(7, "b", 1));
        assert_ne!(b, layer_seed(7, "a", 2));

        let collect = |names: &[&str]| -> BTreeMap<String, u64> {
            names
                .iter()
                .map(|n| ((*n).to_string(), layer_seed(99, n, 1)))
                .collect()
        };
        assert_eq!(collect(&["a", "b", "c"]), collect(&["c", "a", "b"]));
    }
}
