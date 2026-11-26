//! Stable asset identifiers.
//!
//! An [`AssetId`] is a UUID v5 derived from a normalized key. The
//! normalization performed by [`normalize_key`] is part of the stability
//! contract: changing it changes every ID. `AssetId` is a naming ID, not a
//! security hash (v5 uses SHA-1 internally).

use std::fmt;

use uuid::Uuid;

use crate::error::AssetError;

/// Fixed namespace for asset IDs. Never change.
pub const ASSET_NAMESPACE: Uuid = Uuid::from_u128(0x6f3c_1a52_8d4e_4b07_9c21_5e7a_b3d0_41f8);

/// Stable identifier of an asset, derived from its normalized key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(Uuid);

impl AssetId {
    /// Builds an ID from a key, normalizing it first.
    ///
    /// # Errors
    /// Returns [`AssetError::EmptyKey`] if the normalized key is empty.
    pub fn from_key(key: &str) -> Result<Self, AssetError> {
        let normalized = normalize_key(key);
        if normalized.is_empty() {
            return Err(AssetError::EmptyKey);
        }
        Ok(Self(Uuid::new_v5(&ASSET_NAMESPACE, normalized.as_bytes())))
    }

    /// Wraps an existing UUID.
    #[must_use]
    pub const fn from_uuid(u: Uuid) -> Self {
        Self(u)
    }

    /// Returns the underlying UUID.
    #[must_use]
    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

/// Normalizes an asset key: trim, backslash to slash, collapse slashes,
/// strip leading `./` and `/`, trim trailing `/`, lowercase.
#[must_use]
pub fn normalize_key(key: &str) -> String {
    let replaced = key.trim().replace('\\', "/");
    let mut collapsed = String::with_capacity(replaced.len());
    let mut prev_slash = false;
    for c in replaced.chars() {
        if c == '/' {
            if !prev_slash {
                collapsed.push(c);
            }
            prev_slash = true;
        } else {
            collapsed.push(c);
            prev_slash = false;
        }
    }
    let mut s = collapsed.as_str();
    while let Some(rest) = s.strip_prefix("./") {
        s = rest;
    }
    if s == "." {
        s = "";
    }
    let s = s.strip_prefix('/').unwrap_or(s);
    s.trim_end_matches('/').to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE: &str = "edd6e2d5-1c0e-598d-98d6-760ebf1d0798";

    #[test]
    fn id_golden_values() {
        assert_eq!(AssetId::from_key("meshes/cube.gltf").unwrap().to_string(), CUBE);
        assert_eq!(
            AssetId::from_key("textures/grass.png").unwrap().to_string(),
            "d2b52f2b-6b78-5164-a03d-8654200399c2"
        );
    }

    #[test]
    fn id_same_key_same_id() {
        assert_eq!(AssetId::from_key("x/y").unwrap(), AssetId::from_key("x/y").unwrap());
    }

    #[test]
    fn id_normalization_equivalence() {
        for k in ["  .\\Meshes\\\\Cube.GLTF ", "./meshes//cube.gltf", "meshes/cube.gltf"] {
            assert_eq!(AssetId::from_key(k).unwrap().to_string(), CUBE);
        }
    }

    #[test]
    fn id_normalize_key_cases() {
        for k in ["  .\\Meshes\\\\Cube.GLTF ", "./meshes//cube.gltf", "meshes/cube.gltf"] {
            assert_eq!(normalize_key(k), "meshes/cube.gltf");
        }
    }

    #[test]
    fn id_empty_key_rejected() {
        assert_eq!(AssetId::from_key(""), Err(AssetError::EmptyKey));
        assert_eq!(AssetId::from_key("  ./ "), Err(AssetError::EmptyKey));
    }

    #[test]
    fn id_distinct_keys_distinct_ids() {
        assert_ne!(AssetId::from_key("a").unwrap(), AssetId::from_key("b").unwrap());
    }
}
