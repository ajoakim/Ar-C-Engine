//! Streaming key for a terrain cell with a cached asset id, generic over the topology.

use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;

use engine_sdk::streaming::StreamKey;
use engine_sdk::{AssetId, SdkError};
use planetary_core::{CubeSphere, Topology};

/// Streaming key: planet number plus cell. The `AssetId` (SHA-1 based) is computed once.
///
/// The asset name is `planet/{planet:08x}/{raw:016x}`. Raw ids of different topologies may
/// collide numerically, which is fine because one planet number uses exactly one topology.
/// The key holds the topology so `parent()` can follow `Topology::primary_parent`.
pub struct CellKey<T: Topology> {
    pub planet: u32,
    pub cell: T::Cell,
    raw: u64,
    asset: AssetId,
    topo: Arc<T>,
}

/// The cube-sphere key of Phase 7.
pub type PatchKey = CellKey<CubeSphere>;

impl<T: Topology> CellKey<T> {
    /// Builds the key `planet/{planet:08x}/{raw:016x}`.
    ///
    /// # Errors
    /// `SdkError::App` if the asset id cannot be derived (never for the fixed ASCII form).
    pub fn new(topo: &Arc<T>, planet: u32, cell: T::Cell) -> Result<Self, SdkError> {
        let raw = topo.raw(cell);
        let name = format!("planet/{planet:08x}/{raw:016x}");
        let asset =
            AssetId::from_key(&name).map_err(|e| SdkError::App(format!("cell key {name}: {e}")))?;
        Ok(CellKey {
            planet,
            cell,
            raw,
            asset,
            topo: Arc::clone(topo),
        })
    }

    /// The cached asset id.
    #[must_use]
    pub fn asset(&self) -> AssetId {
        self.asset
    }

    /// The raw cell id.
    #[must_use]
    pub fn raw(&self) -> u64 {
        self.raw
    }
}

impl<T: Topology> Clone for CellKey<T> {
    fn clone(&self) -> Self {
        CellKey {
            planet: self.planet,
            cell: self.cell,
            raw: self.raw,
            asset: self.asset,
            topo: Arc::clone(&self.topo),
        }
    }
}

impl<T: Topology> PartialEq for CellKey<T> {
    fn eq(&self, other: &Self) -> bool {
        self.planet == other.planet && self.raw == other.raw
    }
}

impl<T: Topology> Eq for CellKey<T> {}

impl<T: Topology> PartialOrd for CellKey<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Topology> Ord for CellKey<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.planet, self.raw).cmp(&(other.planet, other.raw))
    }
}

impl<T: Topology> fmt::Debug for CellKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CellKey(planet={}, raw={:#x})", self.planet, self.raw)
    }
}

impl<T: Topology> StreamKey for CellKey<T> {
    fn asset_id(&self) -> AssetId {
        self.asset
    }

    fn parent(&self) -> Option<Self> {
        let p = self.topo.primary_parent(self.cell)?;
        CellKey::new(&self.topo, self.planet, p).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_sdk::DVec3;
    use planetary_core::{Face, PatchId, locate};

    fn topo() -> Arc<CubeSphere> {
        Arc::new(CubeSphere::new(32))
    }

    #[test]
    fn stable_and_distinct() {
        let t = topo();
        let a = PatchKey::new(&t, 1, PatchId::root(Face::PosX)).expect("root");
        let b = PatchKey::new(&t, 1, PatchId::root(Face::PosX)).expect("root");
        let c = PatchKey::new(&t, 1, PatchId::root(Face::NegX)).expect("root");
        let d = PatchKey::new(&t, 2, PatchId::root(Face::PosX)).expect("root");
        assert_eq!(a.asset_id(), b.asset_id());
        assert_ne!(a.asset_id(), c.asset_id());
        assert_ne!(a.asset_id(), d.asset_id());
        assert_eq!(a.asset(), a.asset_id());
    }

    #[test]
    fn asset_id_matches_phase7_name() {
        let t = topo();
        let id = locate(DVec3::new(0.3, 0.5, 0.8).normalize(), 5);
        let k = PatchKey::new(&t, 1, id).expect("key");
        let name = format!("planet/{:08x}/{:016x}", 1, id.raw());
        assert_eq!(k.asset_id(), AssetId::from_key(&name).expect("id"));
        assert_eq!(k.raw(), id.raw());
        assert!(format!("{k:?}").contains("planet=1"));
    }

    #[test]
    fn orders_by_planet_then_raw() {
        let t = topo();
        let a = PatchKey::new(&t, 1, PatchId::root(Face::PosX)).expect("a");
        let b = PatchKey::new(&t, 1, PatchId::root(Face::NegZ)).expect("b");
        let c = PatchKey::new(&t, 2, PatchId::root(Face::PosX)).expect("c");
        assert_eq!(a.cmp(&b), a.raw().cmp(&b.raw()));
        assert!(b < c && a < c);
    }

    #[test]
    fn parent_chain_reaches_root() {
        let t = topo();
        let id = locate(DVec3::new(0.3, 0.5, 0.8).normalize(), 24);
        let mut k = PatchKey::new(&t, 1, id).expect("deep");
        let mut steps = 0;
        while let Some(p) = k.parent() {
            assert_eq!(p.cell.level() + 1, k.cell.level());
            k = p;
            steps += 1;
        }
        assert_eq!(steps, 24);
        assert_eq!(k.cell.level(), 0);
    }
}
