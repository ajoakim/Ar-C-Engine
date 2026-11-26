//! Integration tests for glTF import (fixture, malformed input, limits).

mod support;

use conventional_3d::procedural::cube_mesh;
use conventional_3d::{
    ImportError, ImportLimits, import_gltf_path, import_gltf_path_with, import_gltf_slice,
    import_gltf_slice_with,
};
use std::path::PathBuf;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/textured_cube.glb")
}

fn tri_json(uri: &str) -> String {
    format!(
        "{{\"asset\":{{\"version\":\"2.0\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\
         \"nodes\":[{{\"mesh\":0}}],\
         \"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":0}}}}]}}],\
         \"accessors\":[{{\"bufferView\":0,\"componentType\":5126,\"count\":3,\"type\":\"VEC3\",\
         \"min\":[0,0,0],\"max\":[1,1,0]}}],\
         \"bufferViews\":[{{\"buffer\":0,\"byteOffset\":0,\"byteLength\":36}}],\
         \"buffers\":[{{\"byteLength\":36,\"uri\":\"{uri}\"}}]}}"
    )
}

#[test]
fn fixture_is_up_to_date() {
    let on_disk = std::fs::read(fixture_path()).expect("fixture present");
    assert!(on_disk.len() < 10_240, "fixture size {}", on_disk.len());
    assert_eq!(support::textured_cube_glb(), on_disk);
}

#[test]
fn import_fixture_counts() {
    let m = import_gltf_path(&fixture_path()).expect("import");
    assert_eq!(m.meshes.len(), 1);
    assert_eq!(m.meshes[0].vertices.len(), 24);
    assert_eq!(m.meshes[0].indices.len(), 36);
    assert_eq!(m.meshes[0].material, Some(0));
    assert_eq!(m.materials.len(), 1);
    assert_eq!(m.materials[0].base_color, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(m.materials[0].texture, Some(0));
    assert_eq!(m.textures.len(), 1);
    assert_eq!((m.textures[0].width, m.textures[0].height), (8, 8));
    assert_eq!(m.textures[0].rgba8.len(), 256);
    assert_eq!(m.nodes.len(), 2);
    assert_eq!(m.nodes[0].name.as_deref(), Some("root"));
    assert_eq!(m.nodes[0].parent, None);
    assert_eq!(m.nodes[1].name.as_deref(), Some("cube"));
    assert_eq!(m.nodes[1].parent, Some(0));
    assert_eq!(m.nodes[1].meshes, vec![0]);
    let t = m.nodes[1].local;
    assert!((t.translation.y - 0.5).abs() < 1e-6 && t.translation.x == 0.0);
    let expected = engine_sdk::Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
    assert!(t.rotation.angle_between(expected) < 1e-4);
}

#[test]
fn import_fixture_matches_procedural() {
    let m = import_gltf_path(&fixture_path()).expect("import");
    let c = cube_mesh(1.0);
    assert_eq!(m.meshes[0].vertices, c.vertices);
    assert_eq!(m.meshes[0].indices, c.indices);
}

#[test]
fn import_path_and_slice_agree() {
    let bytes = std::fs::read(fixture_path()).expect("read");
    assert_eq!(
        import_gltf_path(&fixture_path()).expect("path"),
        import_gltf_slice(&bytes).expect("slice")
    );
}

#[test]
fn rejects_truncated() {
    let bytes = support::textured_cube_glb();
    for len in [0, 12, 20, bytes.len() / 2, bytes.len() - 1] {
        assert!(import_gltf_slice(&bytes[..len]).is_err(), "prefix {len}");
    }
}

#[test]
fn rejects_out_of_range_index() {
    let glb = support::triangle_glb(Some(&[0, 1, 99]), 4, false);
    assert!(matches!(
        import_gltf_slice(&glb),
        Err(ImportError::InvalidIndex { index: 99, .. })
    ));
}

#[test]
fn rejects_external_uri_in_slice() {
    let json = tri_json("cube.bin");
    assert!(matches!(
        import_gltf_slice(json.as_bytes()),
        Err(ImportError::ExternalReference(_))
    ));
}

#[test]
fn rejects_escaping_uris() {
    let dir = std::env::temp_dir().join(format!("c3d-uri-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    for uri in ["../secret.bin", "%FF.bin", "file:///etc/passwd", "/abs.bin"] {
        let json = tri_json(uri);
        let r = import_gltf_slice_with(json.as_bytes(), Some(&dir), &ImportLimits::default());
        assert!(
            matches!(r, Err(ImportError::UnsupportedUri(_))),
            "{uri}: {r:?}"
        );
    }
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn accepts_data_uri_buffer() {
    let data: Vec<u8> = [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let uri = format!(
        "data:application/octet-stream;base64,{}",
        support::base64(&data)
    );
    let m = import_gltf_slice(tri_json(&uri).as_bytes()).expect("import");
    assert_eq!(m.meshes.len(), 1);
    assert_eq!(m.meshes[0].vertices.len(), 3);
}

#[test]
fn rejects_oversized_texture() {
    let limits = ImportLimits {
        max_texture_dimension: 4,
        ..ImportLimits::default()
    };
    assert!(matches!(
        import_gltf_path_with(&fixture_path(), &limits),
        Err(ImportError::TextureTooLarge { .. })
    ));
}

#[test]
fn rejects_file_too_large() {
    let limits = ImportLimits {
        max_file_bytes: 16,
        ..ImportLimits::default()
    };
    assert!(matches!(
        import_gltf_path_with(&fixture_path(), &limits),
        Err(ImportError::FileTooLarge { .. })
    ));
}

#[test]
fn generates_missing_attributes() {
    let m = import_gltf_slice(&support::triangle_glb(None, 4, false)).expect("import");
    let mesh = &m.meshes[0];
    assert_eq!(mesh.indices, vec![0, 1, 2]);
    for v in &mesh.vertices {
        assert_eq!(v.uv, [0.0, 0.0]);
        let len = v.normal.iter().map(|c| c * c).sum::<f32>().sqrt();
        assert!((len - 1.0).abs() < 1e-5);
        assert!((v.normal[2].abs() - 1.0).abs() < 1e-5);
    }
}

#[test]
fn skips_non_triangle_primitives() {
    let m = import_gltf_slice(&support::triangle_glb(Some(&[0, 1, 2]), 4, true)).expect("import");
    assert_eq!(m.meshes.len(), 1);
}

#[test]
fn rejects_deep_hierarchy() {
    let r = import_gltf_slice(&support::chain_glb(70));
    assert!(
        matches!(r, Err(ImportError::HierarchyTooDeep { max: 64 })),
        "{r:?}"
    );
    assert!(import_gltf_slice(&support::chain_glb(10)).is_ok());
}

#[test]
#[ignore = "regenerates assets/textured_cube.glb; run once and commit"]
fn regenerate_textured_cube_fixture() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/textured_cube.glb");
    std::fs::write(path, support::textured_cube_glb()).expect("write fixture");
}
