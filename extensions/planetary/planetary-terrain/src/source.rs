//! Worker-side patch producer.

use std::sync::Arc;

use engine_sdk::CancelToken;
use engine_sdk::streaming::StreamSource;
use planetary_core::lod::LodModel;
use planetary_core::{PlanetTerrain, build_patch_cancellable, encode_payload, payload_len};

use crate::key::PatchKey;

/// Builds patch meshes on worker threads and encodes them as validated payloads.
pub struct PatchSource {
    terrain: Arc<PlanetTerrain>,
    model: LodModel,
}

impl PatchSource {
    #[must_use]
    pub fn new(terrain: Arc<PlanetTerrain>, model: LodModel) -> Self {
        Self { terrain, model }
    }
}

impl StreamSource<PatchKey> for PatchSource {
    fn estimate_bytes(&self, _key: &PatchKey) -> u64 {
        payload_len(self.terrain.cells_per_edge()) as u64
    }

    fn produce(&self, key: &PatchKey, cancel: &CancelToken) -> Option<Vec<u8>> {
        if cancel.is_cancelled() {
            return None;
        }
        build_patch_cancellable(&self.terrain, &self.model, key.cell, &|| {
            cancel.is_cancelled()
        })
        .map(|m| encode_payload(&m))
    }
}
