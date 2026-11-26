//! Streaming key for a terrain patch with a cached asset id.

use engine_sdk::streaming::StreamKey;
use engine_sdk::{AssetId, SdkError};
use planetary_core::PatchId;

/// Streaming key: planet number plus patch id. The `AssetId` (SHA-1 based) is computed once.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PatchKey {
    pub planet: u32,
    pub id: PatchId,
    asset: AssetId,
}

impl PatchKey {
    /// Builds the key `planet/{planet:08x}/{raw:016x}`.
    ///
    /// # Errors
    /// `SdkError::App` if the asset id cannot be derived (never for the fixed ASCII form).
    pub fn new(planet: u32, id: PatchId) -> Result<PatchKey, SdkError> {
        let name = format!("planet/{planet:08x}/{:016x}", id.raw());
        let asset = AssetId::from_key(&name)
            .map_err(|e| SdkError::App(format!("patch key {name}: {e}")))?;
        Ok(PatchKey { planet, id, asset })
    }

    /// The cached asset id.
    #[must_use]
    pub fn asset(&self) -> AssetId {
        self.asset
    }
}

impl StreamKey for PatchKey {
    fn asset_id(&self) -> AssetId {
        self.asset
    }

    fn parent(&self) -> Option<Self> {
        PatchKey::new(self.planet, self.id.parent()?).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_sdk::DVec3;
    use planetary_core::{Face, locate};

    #[test]
    fn stable_and_distinct() {
        let a = PatchKey::new(1, PatchId::root(Face::PosX)).expect("root");
        let b = PatchKey::new(1, PatchId::root(Face::PosX)).expect("root");
        let c = PatchKey::new(1, PatchId::root(Face::NegX)).expect("root");
        let d = PatchKey::new(2, PatchId::root(Face::PosX)).expect("root");
        assert_eq!(a.asset_id(), b.asset_id());
        assert_ne!(a.asset_id(), c.asset_id());
        assert_ne!(a.asset_id(), d.asset_id());
        assert_eq!(a.asset(), a.asset_id());
    }

    #[test]
    fn parent_chain_reaches_root() {
        let id = locate(DVec3::new(0.3, 0.5, 0.8).normalize(), 24);
        let mut k = PatchKey::new(1, id).expect("deep");
        let mut steps = 0;
        while let Some(p) = k.parent() {
            assert_eq!(p.id.level() + 1, k.id.level());
            k = p;
            steps += 1;
        }
        assert_eq!(steps, 24);
        assert_eq!(k.id.level(), 0);
    }
}
