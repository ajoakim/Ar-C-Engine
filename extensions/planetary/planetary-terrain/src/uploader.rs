//! Frame-thread GPU uploader: validated v4 decode then a vertex-only upload.

use std::collections::BTreeMap;
use std::fmt;

use engine_sdk::streaming::Uploader;
use engine_sdk::{DVec3, MeshHandle, Resources, VertexPosNormalUvMorph};
use planetary_core::lod::HeightRange;
use planetary_core::{Topology, decode_cell_payload};

use crate::key::CellKey;

/// A GPU-resident cell owned by the terrain system: one vertex buffer, no index buffer (drawn
/// as an index range of the shared buffer of its vertex class).
pub struct HeldCell<T: Topology> {
    pub key: CellKey<T>,
    pub mesh: MeshHandle,
    pub origin: DVec3,
    /// Measured height range of the cell samples.
    pub range: HeightRange,
    pub vertex_class: u8,
}

impl<T: Topology> Clone for HeldCell<T> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            mesh: self.mesh,
            origin: self.origin,
            range: self.range,
            vertex_class: self.vertex_class,
        }
    }
}

impl<T: Topology> fmt::Debug for HeldCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeldCell")
            .field("key", &self.key)
            .field("origin", &self.origin)
            .field("range", &self.range)
            .field("vertex_class", &self.vertex_class)
            .finish_non_exhaustive()
    }
}

/// Uploader borrowing the frame's GPU resources and the held-mesh table.
pub struct GpuUploader<'r, 'a, T: Topology> {
    pub resources: &'r mut Resources<'a>,
    pub held: &'r mut BTreeMap<T::Cell, HeldCell<T>>,
    /// Exact vertex count per vertex class (`None` for an unknown class).
    pub vertex_counts: &'r [Option<u32>],
    pub rejected: Vec<CellKey<T>>,
}

impl<T: Topology> Uploader<CellKey<T>> for GpuUploader<'_, '_, T> {
    fn upload(&mut self, key: &CellKey<T>, data: &[u8]) -> bool {
        let counts = self.vertex_counts;
        let Ok(mesh) = decode_cell_payload(data, key.raw(), &|class| {
            counts.get(usize::from(class)).copied().flatten()
        }) else {
            // A corrupt payload must not retry forever; the system evicts it afterwards.
            self.rejected.push(key.clone());
            return true;
        };
        // decode guarantees `morph.len() == vertices.len()`.
        let verts: Vec<VertexPosNormalUvMorph> = mesh
            .vertices
            .iter()
            .zip(&mesh.morph)
            .map(|(v, t)| VertexPosNormalUvMorph {
                position: v.position,
                normal: v.normal,
                uv: v.uv,
                morph_position: t.position,
                morph_uv: t.uv,
            })
            .collect();
        match self
            .resources
            .create_lit_morph_vertices("terrain-cell", &verts)
        {
            Err(_) => false,
            Ok(h) => {
                let held = HeldCell {
                    key: key.clone(),
                    mesh: h,
                    origin: mesh.origin,
                    range: HeightRange {
                        min: mesh.height_min,
                        max: mesh.height_max,
                    },
                    vertex_class: mesh.vertex_class,
                };
                if let Some(old) = self.held.insert(key.cell, held) {
                    let _ = self.resources.destroy_mesh(old.mesh);
                }
                true
            }
        }
    }
}
