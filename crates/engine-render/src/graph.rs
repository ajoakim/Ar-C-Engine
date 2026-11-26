//! Minimal render graph: named passes with declared reads/writes, topologically sorted.

use crate::backend::{DrawCommand, FramePacket, PassPacket, REVERSE_Z_CLEAR_DEPTH};
use crate::error::RenderError;
use crate::handles::{PipelineId, TextureId};
use crate::scene::RenderScene;

/// Imported surface color resource.
pub const BACKBUFFER: &str = "backbuffer";
/// Imported surface depth resource.
pub const DEPTH: &str = "depth";
/// Name of the P0 main pass.
pub const MAIN_PASS: &str = "main";

/// Render graph errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GraphError {
    /// Graph has no passes.
    #[error("render graph has no passes")]
    Empty,
    /// Pass name used twice.
    #[error("duplicate pass {0}")]
    DuplicatePass(String),
    /// Import name used twice.
    #[error("duplicate import {0}")]
    DuplicateImport(String),
    /// Pass reads a resource nobody provides.
    #[error("pass {pass} reads unknown resource {resource}")]
    UnknownResource {
        /// Pass name.
        pass: String,
        /// Resource name.
        resource: String,
    },
    /// Dependency cycle.
    #[error("cycle among passes {passes:?}")]
    Cycle {
        /// Passes that could not be ordered.
        passes: Vec<String>,
    },
}

/// A pass declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassDesc {
    /// Pass name.
    pub name: String,
    /// Resources read.
    pub reads: Vec<String>,
    /// Resources written.
    pub writes: Vec<String>,
}

/// Graph under construction.
#[derive(Debug, Clone, Default)]
pub struct RenderGraph {
    imports: Vec<String>,
    passes: Vec<PassDesc>,
}

impl RenderGraph {
    /// Empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare an externally provided resource.
    pub fn import(&mut self, name: &str) -> Result<(), GraphError> {
        if self.imports.iter().any(|i| i == name) {
            return Err(GraphError::DuplicateImport(name.to_owned()));
        }
        self.imports.push(name.to_owned());
        Ok(())
    }

    /// Add a pass.
    pub fn add_pass(
        &mut self,
        name: &str,
        reads: &[&str],
        writes: &[&str],
    ) -> Result<(), GraphError> {
        if self.passes.iter().any(|p| p.name == name) {
            return Err(GraphError::DuplicatePass(name.to_owned()));
        }
        self.passes.push(PassDesc {
            name: name.to_owned(),
            reads: reads.iter().map(|s| (*s).to_owned()).collect(),
            writes: writes.iter().map(|s| (*s).to_owned()).collect(),
        });
        Ok(())
    }

    /// Validate and topologically sort (Kahn, lowest insertion index first).
    pub fn compile(&self) -> Result<CompiledGraph, GraphError> {
        if self.passes.is_empty() {
            return Err(GraphError::Empty);
        }
        for p in &self.passes {
            for r in &p.reads {
                let known =
                    self.imports.contains(r) || self.passes.iter().any(|w| w.writes.contains(r));
                if !known {
                    return Err(GraphError::UnknownResource {
                        pass: p.name.clone(),
                        resource: r.clone(),
                    });
                }
            }
        }
        let n = self.passes.len();
        // deps[i] = passes that must run before i.
        let deps: Vec<Vec<usize>> = self
            .passes
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (0..n)
                    .filter(|&w| {
                        w != i
                            && self
                                .passes
                                .get(w)
                                .is_some_and(|wp| p.reads.iter().any(|r| wp.writes.contains(r)))
                    })
                    .collect()
            })
            .collect();
        let mut done = vec![false; n];
        let mut ordered = Vec::with_capacity(n);
        while ordered.len() < n {
            let next = (0..n).find(|&i| {
                !done.get(i).copied().unwrap_or(true)
                    && deps
                        .get(i)
                        .is_some_and(|d| d.iter().all(|&w| done.get(w).copied().unwrap_or(false)))
            });
            let Some(i) = next else {
                let passes = self
                    .passes
                    .iter()
                    .zip(&done)
                    .filter(|(_, d)| !**d)
                    .map(|(p, _)| p.name.clone())
                    .collect();
                return Err(GraphError::Cycle { passes });
            };
            if let (Some(d), Some(p)) = (done.get_mut(i), self.passes.get(i)) {
                *d = true;
                ordered.push(p.clone());
            }
        }
        Ok(CompiledGraph { ordered })
    }

    /// The P0 graph: one main pass writing backbuffer and depth.
    pub fn main_p0() -> Self {
        Self {
            imports: vec![BACKBUFFER.to_owned(), DEPTH.to_owned()],
            passes: vec![PassDesc {
                name: MAIN_PASS.to_owned(),
                reads: Vec::new(),
                writes: vec![BACKBUFFER.to_owned(), DEPTH.to_owned()],
            }],
        }
    }
}

/// A sorted, validated graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledGraph {
    ordered: Vec<PassDesc>,
}

impl CompiledGraph {
    /// Pass names in execution order.
    pub fn pass_names(&self) -> Vec<&str> {
        self.ordered.iter().map(|p| p.name.as_str()).collect()
    }

    /// Turn a scene into a frame packet.
    pub fn build_frame_packet(
        &self,
        scene: &RenderScene,
        pipeline: PipelineId,
        clear_color: [f32; 4],
    ) -> Result<FramePacket, RenderError> {
        let mut passes = Vec::with_capacity(self.ordered.len());
        for p in &self.ordered {
            if !p.writes.iter().any(|w| w == BACKBUFFER) {
                return Err(RenderError::UnsupportedPassTarget {
                    pass: p.name.clone(),
                });
            }
            let depth_target = p
                .writes
                .iter()
                .any(|w| w == DEPTH)
                .then_some(TextureId::DEPTH);
            passes.push(PassPacket {
                name: p.name.clone(),
                color_target: TextureId::BACKBUFFER,
                depth_target,
                clear_color,
                clear_depth: REVERSE_Z_CLEAR_DEPTH,
                draws: scene
                    .draws
                    .iter()
                    .map(|d| DrawCommand {
                        pipeline,
                        mesh: d.mesh,
                        mvp: scene.view_proj * d.model,
                    })
                    .collect(),
            });
        }
        Ok(FramePacket {
            frame_index: scene.frame_index,
            passes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{
        BufferDesc, BufferUsage, PipelineDesc, RenderBackend, ShaderId, VertexLayout,
        VertexPosColor, encode_vertices,
    };
    use crate::camera::Camera;
    use crate::handles::{BufferId, MeshHandle};
    use crate::null::NullBackend;
    use crate::scene::{DrawItem, RenderItem, extract};
    use engine_math::{FrameRegistry, WorldPos};
    use glam::Mat4;

    fn names(g: &RenderGraph) -> Vec<String> {
        g.compile()
            .unwrap()
            .pass_names()
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn graph_main_p0_compiles_to_single_main_pass() {
        assert_eq!(names(&RenderGraph::main_p0()), ["main"]);
    }

    #[test]
    fn graph_orders_writer_before_reader() {
        let mut g = RenderGraph::new();
        g.import(BACKBUFFER).unwrap();
        g.import(DEPTH).unwrap();
        g.add_pass("post", &["hdr"], &[BACKBUFFER]).unwrap();
        g.add_pass("main", &[], &["hdr", DEPTH]).unwrap();
        assert_eq!(names(&g), ["main", "post"]);
    }

    #[test]
    fn graph_tie_break_is_insertion_order() {
        let mut g = RenderGraph::new();
        g.add_pass("a", &[], &["x"]).unwrap();
        g.add_pass("b", &[], &["y"]).unwrap();
        for _ in 0..100 {
            assert_eq!(names(&g), ["a", "b"]);
        }
    }

    #[test]
    fn graph_rejects_cycle() {
        let mut g = RenderGraph::new();
        g.add_pass("a", &["x"], &["y"]).unwrap();
        g.add_pass("b", &["y"], &["x"]).unwrap();
        match g.compile() {
            Err(GraphError::Cycle { passes }) => {
                assert!(passes.contains(&"a".to_owned()) && passes.contains(&"b".to_owned()));
            }
            other => panic!("expected cycle, got {other:?}"),
        }
    }

    #[test]
    fn graph_rejects_unknown_resource() {
        let mut g = RenderGraph::new();
        g.add_pass("p", &["gbuffer"], &[]).unwrap();
        assert_eq!(
            g.compile().unwrap_err(),
            GraphError::UnknownResource {
                pass: "p".into(),
                resource: "gbuffer".into()
            }
        );
    }

    #[test]
    fn graph_rejects_duplicate_pass_and_import() {
        let mut g = RenderGraph::new();
        g.add_pass("main", &[], &[]).unwrap();
        assert_eq!(
            g.add_pass("main", &[], &[]),
            Err(GraphError::DuplicatePass("main".into()))
        );
        g.import("depth").unwrap();
        assert_eq!(
            g.import("depth"),
            Err(GraphError::DuplicateImport("depth".into()))
        );
    }

    #[test]
    fn graph_rejects_empty() {
        assert_eq!(RenderGraph::new().compile().unwrap_err(), GraphError::Empty);
    }

    fn handle() -> MeshHandle {
        MeshHandle::new(VertexLayout::PosColor, BufferId::from_raw(1), 3)
    }

    #[test]
    fn graph_build_packet_main_pass() {
        let v = Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0));
        let m1 = Mat4::from_scale(glam::Vec3::splat(2.0));
        let m2 = Mat4::from_translation(glam::Vec3::new(0.0, 5.0, 0.0));
        let scene = RenderScene {
            frame_index: 7,
            view_proj: v,
            draws: vec![
                DrawItem {
                    mesh: handle(),
                    model: m1,
                },
                DrawItem {
                    mesh: handle(),
                    model: m2,
                },
            ],
        };
        let pipe = PipelineId::from_raw(3);
        let pk = RenderGraph::main_p0()
            .compile()
            .unwrap()
            .build_frame_packet(&scene, pipe, [0.1, 0.2, 0.3, 1.0])
            .unwrap();
        assert_eq!(pk.frame_index, 7);
        assert_eq!(pk.passes.len(), 1);
        let p = &pk.passes[0];
        assert_eq!(p.name, "main");
        assert_eq!(p.color_target, TextureId::BACKBUFFER);
        assert_eq!(p.depth_target, Some(TextureId::DEPTH));
        assert_eq!(p.clear_depth, REVERSE_Z_CLEAR_DEPTH);
        assert_eq!(p.draws[0].mvp, v * m1);
        assert_eq!(p.draws[1].mvp, v * m2);
        assert_eq!(p.draws[0].pipeline, pipe);
    }

    #[test]
    fn graph_build_packet_rejects_pass_without_backbuffer() {
        let mut g = RenderGraph::new();
        g.add_pass("t", &[], &["tmp"]).unwrap();
        let r = g.compile().unwrap().build_frame_packet(
            &RenderScene::default(),
            PipelineId::from_raw(1),
            [0.0; 4],
        );
        assert_eq!(
            r,
            Err(RenderError::UnsupportedPassTarget { pass: "t".into() })
        );
    }

    #[test]
    fn graph_packet_accepted_by_null_backend() {
        let mut be = NullBackend::new();
        let data = encode_vertices(&[VertexPosColor::default(); 3]);
        let buf = be
            .create_buffer(
                &BufferDesc {
                    label: "tri",
                    usage: BufferUsage::Vertex,
                },
                &data,
            )
            .unwrap();
        let pipe = be
            .create_pipeline(&PipelineDesc {
                label: "p",
                shader: ShaderId::UnlitVertexColor,
                vertex_layout: VertexLayout::PosColor,
            })
            .unwrap();
        let f = FrameRegistry::new();
        let cam = Camera::new(WorldPos::from_xyz(f.root(), 0.0, 0.0, 0.0).unwrap());
        let item = RenderItem::new(
            MeshHandle::new(VertexLayout::PosColor, buf, 3),
            WorldPos::from_xyz(f.root(), 0.0, 0.0, -5.0).unwrap(),
        );
        let scene = extract(&f, &cam, 16.0 / 9.0, &[item], 0).unwrap();
        let pk = RenderGraph::main_p0()
            .compile()
            .unwrap()
            .build_frame_packet(&scene, pipe, [0.0, 0.0, 0.0, 1.0])
            .unwrap();
        be.submit(&pk).unwrap();
        assert_eq!(be.frames().len(), 1);
    }
}
