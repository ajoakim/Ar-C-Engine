use conventional_3d::{ImportError, ImportLimits, ModelAssets, ModelLoader};
use engine_sdk::AssetId;
use std::path::PathBuf;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/textured_cube.glb");

fn missing() -> PathBuf {
    std::env::temp_dir().join("conventional_3d_definitely_missing.glb")
}

#[test]
fn loader_wait_imports_fixture_on_job() {
    let mut l = ModelLoader::new(1).unwrap();
    let t = l.load(FIXTURE, ImportLimits::default());
    assert_eq!(l.pending_len(), 1);
    let m = l.wait(t).unwrap();
    assert_eq!(m.meshes.len(), 1);
    assert_eq!(m.meshes[0].vertices.len(), 24);
    assert_eq!(l.pending_len(), 0);
}

#[test]
fn loader_wait_all_mixed_results() {
    let mut l = ModelLoader::new(2).unwrap();
    let t1 = l.load(FIXTURE, ImportLimits::default());
    let t2 = l.load(missing(), ImportLimits::default());
    let r = l.wait_all();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].ticket, t1);
    assert_eq!(r[1].ticket, t2);
    assert!(r[0].result.is_ok());
    assert!(matches!(r[1].result, Err(ImportError::Io(_))));
}

#[test]
fn loader_wait_buffers_other_results() {
    let mut l = ModelLoader::new(2).unwrap();
    let t1 = l.load(FIXTURE, ImportLimits::default());
    let t2 = l.load(FIXTURE, ImportLimits::default());
    assert!(l.wait(t2).is_ok());
    let r = l.wait_all();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].ticket, t1);
    assert!(r[0].result.is_ok());
}

#[test]
fn loader_zero_workers_clamped() {
    assert!(ModelLoader::new(0).is_ok());
}

#[test]
fn loader_unknown_ticket_errors() {
    let mut l = ModelLoader::new(1).unwrap();
    let t = l.load(FIXTURE, ImportLimits::default());
    assert!(l.wait(t).is_ok());
    assert!(matches!(l.wait(t), Err(ImportError::JobFailed(_))));
}

#[test]
fn assets_register_deterministic_ids() {
    let mut l = ModelLoader::new(1).unwrap();
    let t = l.load(FIXTURE, ImportLimits::default());
    let model = l.wait(t).unwrap();
    let mut a = ModelAssets::new();
    let reg = a.register("models/cube.glb", model.clone()).unwrap();
    assert_eq!(a.meshes.len(), 1);
    assert_eq!(a.materials.len(), 1);
    assert_eq!(a.textures.len(), 1);
    let id = a.meshes.id(reg.meshes[0]).unwrap();
    assert_eq!(id, AssetId::from_key("models/cube.glb#mesh/0").unwrap());
    let reg2 = a.register("models/cube.glb", model).unwrap();
    assert_eq!(a.meshes.id(reg2.meshes[0]).unwrap(), id);
    assert_eq!(a.meshes.len(), 1);
}
