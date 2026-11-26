//! Shared index buffers and index-range draws through the SDK on NullBackend.
#![allow(clippy::result_large_err)]

use engine_sdk::prelude::*;
use engine_sdk::{
    Engine, EngineConfig, IndexBufferHandle, MeshHandle, NullBackend, RenderItem, Runtime, WorldPos,
};

fn vtx() -> VertexPosNormalUv {
    VertexPosNormalUv {
        position: [0.0; 3],
        normal: [0.0, 1.0, 0.0],
        uv: [0.0; 2],
    }
}

#[derive(Default)]
struct RangeApp {
    ib: Option<IndexBufferHandle>,
    meshes: Vec<MeshHandle>,
}

impl App for RangeApp {
    fn setup(&mut self, r: &mut Resources<'_>, s: &mut SceneState) -> Result<(), SdkError> {
        let ib = r.create_index_buffer("ib", &[0, 1, 2, 2, 1, 3])?;
        for (first, label) in [(0, "a"), (3, "b")] {
            let m = r
                .create_lit_vertices(label, &[vtx(); 4])?
                .with_index_range(&ib, first, 3)?;
            let pos = WorldPos::from_xyz(s.frames.root(), 0.0, 0.0, -5.0).unwrap();
            s.items.push(RenderItem::new(m, pos));
            self.meshes.push(m);
        }
        self.ib = Some(ib);
        Ok(())
    }

    fn update(&mut self, _dt: f64, _s: &mut SceneState) -> AppControl {
        AppControl::Continue
    }
}

fn runtime() -> Runtime<NullBackend> {
    Runtime::new(Engine::new(EngineConfig::default()), NullBackend::new()).expect("runtime")
}

#[test]
fn two_ranges_of_one_buffer_render_two_draws() {
    let mut rt = runtime();
    let mut app = RangeApp::default();
    rt.setup(&mut app).unwrap();
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    // two vertex buffers + one shared index buffer
    assert_eq!(rt.backend().buffer_count(), 3);
    let draws: usize = rt
        .backend()
        .last_frame()
        .unwrap()
        .passes
        .iter()
        .map(|p| p.draws.len())
        .sum();
    assert_eq!(draws, 2);
}

#[derive(Default)]
struct Lifecycle {
    calls: u32,
    ib: Option<IndexBufferHandle>,
    mesh: Option<MeshHandle>,
    results: Vec<bool>,
}

impl App for Lifecycle {
    fn setup(&mut self, _r: &mut Resources<'_>, _s: &mut SceneState) -> Result<(), SdkError> {
        Ok(())
    }

    fn update(&mut self, _dt: f64, _s: &mut SceneState) -> AppControl {
        AppControl::Continue
    }

    fn gpu_frame(&mut self, r: &mut Resources<'_>, _s: &mut SceneState) -> Result<(), SdkError> {
        let n = self.calls;
        self.calls += 1;
        match n {
            0 => {
                let ib = r.create_index_buffer("ib", &[0, 1, 2])?;
                self.mesh = Some(
                    r.create_lit_vertices("v", &[vtx(); 3])?
                        .with_index_range(&ib, 0, 3)?,
                );
                self.ib = Some(ib);
                // invalid index lists are rejected
                self.results.push(r.create_index_buffer("e", &[]).is_err());
                self.results
                    .push(r.create_index_buffer("e", &[0, 1]).is_err());
                self.results.push(r.create_lit_vertices("e", &[]).is_err());
            }
            1 => {
                r.destroy_mesh(self.mesh.take().unwrap())?;
            }
            2 => {
                let ib = self.ib.unwrap();
                r.destroy_index_buffer(ib)?;
                self.results.push(r.destroy_index_buffer(ib).is_err());
            }
            _ => {}
        }
        Ok(())
    }
}

#[test]
fn destroy_mesh_keeps_shared_buffer_until_destroyed() {
    let mut rt = runtime();
    let mut app = Lifecycle::default();
    rt.setup(&mut app).unwrap();
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    assert_eq!(rt.backend().buffer_count(), 2);
    assert_eq!(app.results, vec![true, true, true]);
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    assert_eq!(rt.backend().buffer_count(), 1);
    rt.frame(&mut app, 1.0 / 60.0).unwrap();
    assert_eq!(rt.backend().buffer_count(), 0);
    assert!(app.results[3]);
}

#[test]
fn destroyed_shared_buffer_invalidates_draw() {
    let mut rt = runtime();
    let mut app = RangeApp::default();
    rt.setup(&mut app).unwrap();
    // Destroy the shared buffer behind the scene's back, via a second app hook.
    struct Killer(IndexBufferHandle);
    impl App for Killer {
        fn setup(&mut self, _: &mut Resources<'_>, _: &mut SceneState) -> Result<(), SdkError> {
            Ok(())
        }
        fn update(&mut self, _: f64, _: &mut SceneState) -> AppControl {
            AppControl::Continue
        }
        fn gpu_frame(&mut self, r: &mut Resources<'_>, _: &mut SceneState) -> Result<(), SdkError> {
            r.destroy_index_buffer(self.0)
        }
    }
    let mut killer = Killer(app.ib.unwrap());
    // The scene still draws ranges of the destroyed buffer: the submit is rejected.
    assert!(rt.frame(&mut killer, 1.0 / 60.0).is_err());
}

struct Once<F: FnMut(&mut Resources<'_>) -> Result<(), SdkError>>(F);

impl<F: FnMut(&mut Resources<'_>) -> Result<(), SdkError>> App for Once<F> {
    fn setup(&mut self, r: &mut Resources<'_>, _: &mut SceneState) -> Result<(), SdkError> {
        (self.0)(r)
    }
    fn update(&mut self, _: f64, _: &mut SceneState) -> AppControl {
        AppControl::Continue
    }
}

#[test]
fn range_validation_through_sdk() {
    let mut rt = runtime();
    let mut app = Once(|r: &mut Resources<'_>| {
        let ib = r.create_index_buffer("ib", &[0, 1, 2, 2, 1, 3])?;
        assert_eq!(ib.count, 6);
        assert_eq!(ib.max_index, 3);
        let m = r.create_lit_vertices("v", &[vtx(); 4])?;
        assert!(m.index.is_none());
        assert!(m.with_index_range(&ib, 3, 6).is_err());
        assert!(m.with_index_range(&ib, 0, 2).is_err());
        let small = r.create_lit_vertices("s", &[vtx(); 3])?;
        assert!(small.with_index_range(&ib, 0, 3).is_err());
        Ok(())
    });
    rt.setup(&mut app).unwrap();
}

#[test]
fn create_lit_mesh_still_owns_its_index_buffer() {
    let mut rt = runtime();
    let mut app = Once(|r: &mut Resources<'_>| {
        let m = r.create_lit_mesh("m", &[vtx(); 3], &[0, 1, 2])?;
        assert!(!m.index.unwrap().shared);
        r.destroy_mesh(m)
    });
    rt.setup(&mut app).unwrap();
    assert_eq!(rt.backend().buffer_count(), 0);
}
