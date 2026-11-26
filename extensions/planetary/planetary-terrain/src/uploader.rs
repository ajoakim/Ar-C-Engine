//! Frame-thread GPU uploader: validated decode then `create_lit_mesh`.

use std::collections::BTreeMap;

use engine_sdk::streaming::Uploader;
use engine_sdk::{DVec3, MeshHandle, Resources};
use planetary_core::{PatchId, decode_payload};

use crate::key::PatchKey;

/// A GPU-resident patch owned by the terrain system.
#[derive(Clone, Debug)]
pub struct HeldPatch {
    pub key: PatchKey,
    pub mesh: MeshHandle,
    pub origin: DVec3,
}

/// Uploader borrowing the frame's GPU resources and the held-mesh table.
pub struct GpuUploader<'r, 'a> {
    pub resources: &'r mut Resources<'a>,
    pub held: &'r mut BTreeMap<PatchId, HeldPatch>,
    pub cells_per_edge: u32,
    pub rejected: Vec<PatchKey>,
}

impl Uploader<PatchKey> for GpuUploader<'_, '_> {
    fn upload(&mut self, key: &PatchKey, data: &[u8]) -> bool {
        let Ok(mesh) = decode_payload(data, key.id, self.cells_per_edge) else {
            // A corrupt payload must not retry forever; the system evicts it afterwards.
            self.rejected.push(key.clone());
            return true;
        };
        match self
            .resources
            .create_lit_mesh("planet-patch", &mesh.vertices, &mesh.indices)
        {
            Err(_) => false,
            Ok(h) => {
                let held = HeldPatch {
                    key: key.clone(),
                    mesh: h,
                    origin: mesh.origin,
                };
                if let Some(old) = self.held.insert(key.id, held) {
                    let _ = self.resources.destroy_mesh(old.mesh);
                }
                true
            }
        }
    }
}
