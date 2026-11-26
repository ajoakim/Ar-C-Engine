//! Worker-side cell producer (v3 cell payloads), generic over the topology.

use std::sync::Arc;

use engine_sdk::CancelToken;
use engine_sdk::streaming::StreamSource;
use planetary_core::lod::LodModel;
use planetary_core::{CELL_HEADER_LEN, PlanetTerrain, Topology, encode_cell_payload};

use crate::key::CellKey;

/// Bytes of one vertex in a cell payload (position, normal, uv as f32).
const VERTEX_BYTES: usize = 32;

/// Estimated payload size for a cell with `max_vertex_count` vertices.
#[must_use]
pub fn cell_payload_estimate(max_vertex_count: u32) -> u64 {
    (CELL_HEADER_LEN + max_vertex_count as usize * VERTEX_BYTES) as u64
}

/// Builds cell meshes on worker threads and encodes them as validated v3 payloads.
pub struct CellSource<T: Topology> {
    topo: Arc<T>,
    terrain: Arc<PlanetTerrain>,
    model: LodModel,
    debug_cells: bool,
    estimate: u64,
}

impl<T: Topology> CellSource<T> {
    /// `max_vertex_count` is the largest vertex count over the topology's vertex classes.
    #[must_use]
    pub fn new(
        topo: Arc<T>,
        terrain: Arc<PlanetTerrain>,
        model: LodModel,
        debug_cells: bool,
        max_vertex_count: u32,
    ) -> Self {
        Self {
            topo,
            terrain,
            model,
            debug_cells,
            estimate: cell_payload_estimate(max_vertex_count),
        }
    }
}

impl<T: Topology> StreamSource<CellKey<T>> for CellSource<T> {
    fn estimate_bytes(&self, _key: &CellKey<T>) -> u64 {
        self.estimate
    }

    fn produce(&self, key: &CellKey<T>, cancel: &CancelToken) -> Option<Vec<u8>> {
        if cancel.is_cancelled() {
            return None;
        }
        self.topo
            .build_mesh(
                key.cell,
                &self.terrain,
                &self.model,
                self.debug_cells,
                &|| cancel.is_cancelled(),
            )
            .map(|m| encode_cell_payload(&m))
    }
}
